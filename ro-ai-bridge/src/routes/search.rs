//! Search Route — Parallel multi-source RAG retrieval
//!
//! - POST /api/search — Fetch raw chunks from Vector, Graph, Tree in parallel
//!
//! ISO 29110 — Task 2.1: Parallel 3-Source Search API

use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{info, warn};

use crate::retrieval::graph::{graph_to_retrieval_results, GraphRetriever, SqlGraphRetriever};
use crate::retrieval::qdrant::{QdrantRetriever, RetrievalResult, VectorRetriever};
use crate::retrieval::tree::{tree_to_retrieval_results, NativeTreeRetriever, TreeRetriever};
use crate::retrieval::{determine_mode_used, rerank_results, rerank_results_rrf, source_distribution, EnsembleWeights};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::iam::IamService;
use mimir_core_ai::services::qdrant::QdrantService;

// ── Request / Response Types ──────────────────────────

/// Request body for POST /api/search
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// The search query string.
    pub query: String,
    /// Tenant ID override (falls back to header/context).
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Ensemble weights for reranking. Defaults to {0.5, 0.3, 0.2}.
    #[serde(default)]
    pub weights: Option<EnsembleWeights>,
    /// Maximum number of results to return. Default: 10, Max: 50.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Filter which sources to query. Default: all three.
    /// Accepted values: "vector", "graph", "tree"
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    /// Qdrant collections to search. Default: ["golden_qa", "source_chunks"].
    #[serde(default)]
    pub collections: Option<Vec<String>>,
    /// Source-level filters to narrow search scope.
    #[serde(default)]
    pub filters: Option<SearchFilters>,
    /// Optional cross-encoder rerank configuration.
    #[serde(default)]
    pub rerank: Option<crate::routes::rag_eval::RerankConfig>,
}

/// Source-level filters for narrowing search scope.
/// Per user requirement: filtering at source level (not chunk level).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SearchFilters {
    /// Only search within these data source IDs.
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// Only search within these source types (e.g., "pdf", "url", "manual").
    #[serde(default)]
    pub source_types: Option<Vec<String>>,
}

/// Response body for POST /api/search
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// Reranked retrieval results from all sources.
    pub results: Vec<RetrievalResult>,
    /// Count of results per source type.
    pub distribution: Value,
    /// The actual weights used for reranking.
    pub weights_used: EnsembleWeights,
    /// Which retrieval mode contributed to results.
    pub mode_used: String,
    /// Total latency in milliseconds.
    pub latency_ms: u64,
    /// The original query string.
    pub query: String,
}

// ── Constants ─────────────────────────────────────────

/// Maximum results allowed per request.
const MAX_LIMIT: usize = 50;

/// Default limit if not specified.
const DEFAULT_LIMIT: usize = 10;

/// Per-source timeout in seconds to prevent one slow source from blocking all.
const SOURCE_TIMEOUT_SECS: u64 = 10;

/// Default Qdrant collections to search.
const DEFAULT_COLLECTIONS: &[&str] = &["golden_qa", "source_chunks"];

// ── Route Registration ───────────────────────────────

pub fn search_routes() -> Router<DbPool> {
    Router::new().route("/api/search", post(search_handler))
}

// ── Handler ──────────────────────────────────────────

/// POST /api/search — Parallel multi-source retrieval
///
/// Fires Vector, Graph, and Tree retrievers concurrently via `tokio::join!`,
/// applies ensemble reranking with configurable weights, and returns raw
/// results without LLM synthesis.
async fn search_handler(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<SearchRequest>,
) -> impl IntoResponse {
    let start = Instant::now();

    // Validate query
    let query = payload.query.trim().to_string();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Query must not be empty"
            })),
        )
            .into_response();
    }

    // Resolve tenant
    let tenant_id = tenant_ctx
        .as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or(payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    // Resolve weights
    let mut weights = payload.weights.unwrap_or_default();
    weights.normalize();

    // Resolve limit (clamped)
    let limit = payload.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    // Determine which sources to query
    let active_sources = resolve_active_sources(&payload.sources);

    // Resolve embedding model from tenant config
    let iam = IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    // Resolve collections for vector search
    let collections: Vec<String> = payload
        .collections
        .unwrap_or_else(|| DEFAULT_COLLECTIONS.iter().map(|s| s.to_string()).collect());

    info!(
        event = "search",
        query = %query,
        tenant = %tenant_id,
        sources = ?active_sources,
        limit = limit,
        filters = ?payload.filters,
        "🔍 /api/search parallel query"
    );

    let filters = payload.filters.unwrap_or_default();

    // ── Stage 1: Vector + Graph in parallel via tokio::join! ──

    let (vector_results, graph_results) = tokio::join!(
        // Vector search (with timeout)
        async {
            if !active_sources.vector {
                return vec![];
            }
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_vector(&query, &tenant_id, &embed_model, &collections, limit, &filters),
            )
            .await
            {
                Ok(results) => results,
                Err(_) => {
                    warn!(
                        source = "vector",
                        "⏰ Vector search timed out after {}s", SOURCE_TIMEOUT_SECS
                    );
                    vec![]
                }
            }
        },
        // Graph search (with timeout)
        async {
            if !active_sources.graph {
                return vec![];
            }
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_graph(&query, &tenant_id, &pool, limit, &filters),
            )
            .await
            {
                Ok(results) => results,
                Err(_) => {
                    warn!(
                        source = "graph",
                        "⏰ Graph search timed out after {}s", SOURCE_TIMEOUT_SECS
                    );
                    vec![]
                }
            }
        },
    );

    // ── Stage 2: Tree search using Vector candidates as pre-filter ──
    let tree_results = if active_sources.tree {
        let vector_candidate_titles: Vec<String> = vector_results.iter()
            .map(|r| r.title.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        match tokio::time::timeout(
            std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS * 2),
            fetch_tree(&query, &tenant_id, &pool, &filters, &vector_candidate_titles),
        )
        .await
        {
            Ok(results) => results,
            Err(_) => {
                warn!(
                    source = "tree",
                    "⏰ Tree search timed out after {}s", SOURCE_TIMEOUT_SECS * 2
                );
                vec![]
            }
        }
    } else {
        vec![]
    };

    // Merge all results
    let mut all_results = Vec::new();
    all_results.extend(vector_results);
    all_results.extend(graph_results);
    all_results.extend(tree_results);

    info!(
        event = "search_raw",
        total_raw = all_results.len(),
        "📦 Raw results before reranking"
    );

    // Apply reranking strategy
    let ranked = if let Some(ref rc) = payload.rerank {
        if rc.enabled && rc.strategy == "cross-encoder" {
            // Pre-filter with RRF to limit the number of documents passed to Cross-Encoder
            let pre_filtered = rerank_results(&all_results, &weights, (limit * 2).max(20));
            // Try to resolve cross-encoder model
            if let Ok(router) = mimir_core_ai::services::llm_router::LlmRouter::new(pool.clone(), &tenant_id).await {
                if let Ok((reranker, model)) = router.resolve_reranker(rc.model.as_deref()) {
                    crate::retrieval::ensemble::cross_encoder_rerank(&reranker, &model, &query, pre_filtered, limit)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Cross-encoder failed: {}. Falling back to RRF.", e);
                            rerank_results_rrf(&all_results, &weights, limit)
                        })
                } else {
                    rerank_results_rrf(&all_results, &weights, limit)
                }
            } else {
                rerank_results_rrf(&all_results, &weights, limit)
            }
        } else if rc.enabled && rc.strategy == "rrf" {
            // True Reciprocal Rank Fusion
            rerank_results_rrf(&all_results, &weights, limit)
        } else {
            rerank_results(&all_results, &weights, limit)
        }
    } else {
        // Default: weighted score reranking
        rerank_results(&all_results, &weights, limit)
    };

    // Compute distribution and mode
    let distribution = source_distribution(&ranked);
    let mode_used = determine_mode_used(&ranked).to_string();

    let latency_ms = start.elapsed().as_millis() as u64;

    info!(
        event = "search_complete",
        results = ranked.len(),
        mode = %mode_used,
        latency_ms = latency_ms,
        "✅ /api/search completed"
    );

    (
        StatusCode::OK,
        Json(SearchResponse {
            results: ranked,
            distribution,
            weights_used: weights,
            mode_used,
            latency_ms,
            query,
        }),
    )
        .into_response()
}

// ── Source Fetch Functions ────────────────────────────

/// Active source configuration parsed from the request.
#[derive(Debug, Clone)]
struct ActiveSources {
    vector: bool,
    graph: bool,
    tree: bool,
}

/// Parse which sources are active from the optional filter list.
/// If None or empty, all sources are active.
fn resolve_active_sources(sources: &Option<Vec<String>>) -> ActiveSources {
    match sources {
        Some(list) if !list.is_empty() => ActiveSources {
            vector: list.iter().any(|s| s.eq_ignore_ascii_case("vector")),
            graph: list.iter().any(|s| s.eq_ignore_ascii_case("graph")),
            tree: list.iter().any(|s| s.eq_ignore_ascii_case("tree")),
        },
        _ => ActiveSources {
            vector: true,
            graph: true,
            tree: true,
        },
    }
}

/// Fetch results from Qdrant vector search across specified collections.
/// Source_id filtering is pushed to Qdrant query level for correct top-K at scale.
async fn fetch_vector(
    query: &str,
    tenant_id: &str,
    embed_model: &str,
    collections: &[String],
    limit: usize,
    filters: &SearchFilters,
) -> Vec<RetrievalResult> {
    let mut results = Vec::new();
    let qdrant = QdrantService::new();

    let source_ids = filters.source_ids.as_deref();

    for collection in collections {
        let retriever =
            QdrantRetriever::new(qdrant.clone(), embed_model.to_string(), collection.clone());
        match retriever.search_filtered(query, tenant_id, limit, source_ids).await {
            Ok(r) => {
                results.extend(r);
            }
            Err(e) => warn!(collection = %collection, error = %e, "Vector search failed"),
        }
    }

    results
}

/// Fetch results from the SQL-backed Knowledge Graph.
/// Applies source_id filtering via SQL WHERE clause.
async fn fetch_graph(
    query: &str,
    tenant_id: &str,
    pool: &DbPool,
    limit: usize,
    filters: &SearchFilters,
) -> Vec<RetrievalResult> {
    let retriever = SqlGraphRetriever::new(pool.clone());
    match retriever.search(query, tenant_id, limit).await {
        Ok(graph_results) => {
            let all = graph_to_retrieval_results(&graph_results);
            // Apply source_id filter post-retrieval
            if let Some(ref ids) = filters.source_ids {
                all.into_iter()
                    .filter(|r| {
                        r.metadata.get("source_id")
                            .and_then(|v| v.as_i64())
                            .map(|sid| ids.contains(&sid))
                            .unwrap_or(true) // keep if no source_id in metadata
                    })
                    .collect()
            } else {
                all
            }
        }
        Err(e) => {
            warn!(error = %e, "Graph search failed");
            vec![]
        }
    }
}

/// Fetch results from the Native tree search (LLM).
/// Applies source_id filtering by restricting which data_sources are queried.
/// When vector_candidate_titles is provided, only searches those documents (pre-filter for scale).
async fn fetch_tree(
    query: &str,
    tenant_id: &str,
    pool: &DbPool,
    filters: &SearchFilters,
    vector_candidate_titles: &[String],
) -> Vec<RetrievalResult> {
    use mimir_core_ai::services::llm_router::LlmRouter;
    let router = match LlmRouter::new(pool.clone(), tenant_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to init LlmRouter for tree search: {}", e);
            return vec![];
        }
    };
    
    let retriever = crate::retrieval::tree::NativeTreeRetriever::new();

    // Load data sources with tree indexes, optionally filtered by source_ids
    let docs: Vec<(i64, String, Option<String>, Option<String>)> = if let Some(ref ids) = filters.source_ids {
        if ids.is_empty() {
            return vec![];
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT id, name, CAST(raw_markdown AS CHAR), CAST(pageindex_tree AS CHAR) \
             FROM data_sources WHERE tenant_id = ? AND id IN ({})",
            placeholders
        );
        let mut q = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>)>(&query_str)
            .bind(tenant_id);
        for id in ids {
            q = q.bind(id);
        }
        q.fetch_all(pool).await.unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT id, name, CAST(raw_markdown AS CHAR), CAST(pageindex_tree AS CHAR) \
             FROM data_sources WHERE tenant_id = ? \
             AND pageindex_tree IS NOT NULL AND raw_markdown IS NOT NULL \
             LIMIT 50",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let searchable: Vec<(String, String, String)> = docs
        .into_iter()
        .filter_map(
            |(_, name, content, tree_index)| match (content, tree_index) {
                (Some(c), Some(t)) if !c.is_empty() && !t.is_empty() => Some((name, c, t)),
                _ => None,
            },
        )
        .collect();

    if searchable.is_empty() {
        return vec![];
    }

    // Apply vector pre-filter: only search docs whose names match vector results.
    // This limits LLM calls from N docs to max 10 for scalability.
    let filtered_docs: Vec<(String, String, String)> = if !vector_candidate_titles.is_empty() {
        let (matched, rest): (Vec<_>, Vec<_>) = searchable.into_iter()
            .partition(|(name, _, _)| {
                let name_lower = name.to_lowercase();
                vector_candidate_titles.iter().any(|vt| {
                    let vt_lower = vt.to_lowercase();
                    name_lower.contains(&vt_lower) || vt_lower.contains(&name_lower)
                })
            });
        if matched.is_empty() {
            // Fallback: if no vector candidates matched doc names, take first 5
            rest.into_iter().take(5).collect()
        } else {
            matched.into_iter().take(10).collect()
        }
    } else {
        // No pre-filter available, limit to prevent massive fan-out
        searchable.into_iter().take(10).collect()
    };

    tracing::info!(
        tree_docs = filtered_docs.len(),
        "🌲 Tree search: searching {} candidate documents",
        filtered_docs.len()
    );

    let tree_results = retriever.search_parallel(&router, &filtered_docs, query).await;
    tree_to_retrieval_results(&tree_results)
}

// ── Public Search Function (Shared with Benchmark) ───

/// Run the parallel multi-source search and return reranked results.
///
/// This is the core search logic extracted for reuse by the benchmark handler.
/// It performs: resolve embedding → tokio::join! 3 sources → rerank.
pub async fn run_parallel_search(
    pool: &DbPool,
    query: &str,
    tenant_id: &str,
    weights: &EnsembleWeights,
    limit: usize,
) -> Vec<RetrievalResult> {
    run_parallel_search_filtered(pool, query, tenant_id, weights, limit, &SearchFilters::default(), None).await
}

/// Run the parallel multi-source search with source-level filters.
/// Uses a 2-stage approach:
///   Stage 1: Vector + Graph in parallel
///   Stage 2: Tree search using Vector candidates as pre-filter (max 10 docs)
pub async fn run_parallel_search_filtered(
    pool: &DbPool,
    query: &str,
    tenant_id: &str,
    weights: &EnsembleWeights,
    limit: usize,
    filters: &SearchFilters,
    rerank_config: Option<&crate::routes::rag_eval::RerankConfig>,
) -> Vec<RetrievalResult> {
    // Resolve embedding model
    let iam = IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    let collections: Vec<String> = DEFAULT_COLLECTIONS.iter().map(|s| s.to_string()).collect();

    // Stage 1: Fire Vector + Graph in parallel
    let (vector_results, graph_results) = tokio::join!(
        async {
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_vector(query, tenant_id, &embed_model, &collections, limit, filters),
            )
            .await
            {
                Ok(results) => results,
                Err(_) => vec![],
            }
        },
        async {
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_graph(query, tenant_id, pool, limit, filters),
            )
            .await
            {
                Ok(results) => results,
                Err(_) => vec![],
            }
        },
    );

    // Stage 2: Extract candidate document titles from Vector results for Tree pre-filter
    let vector_candidate_titles: Vec<String> = vector_results.iter()
        .map(|r| r.title.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let tree_results = match tokio::time::timeout(
        std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS * 2), // Tree gets more time
        fetch_tree(query, tenant_id, pool, filters, &vector_candidate_titles),
    )
    .await
    {
        Ok(results) => results,
        Err(_) => vec![],
    };

    let mut all_results = Vec::new();
    all_results.extend(vector_results);
    all_results.extend(graph_results);
    all_results.extend(tree_results);

    // Apply reranking strategy
    if let Some(rc) = rerank_config {
        if rc.enabled && rc.strategy == "cross-encoder" {
            let pre_filtered = rerank_results(&all_results, weights, (limit * 2).max(20));
            if let Ok(router) = mimir_core_ai::services::llm_router::LlmRouter::new(pool.clone(), tenant_id).await {
                if let Ok((reranker, model)) = router.resolve_reranker(rc.model.as_deref()) {
                    return crate::retrieval::ensemble::cross_encoder_rerank(&reranker, &model, query, pre_filtered, limit)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Cross-encoder failed in parallel search: {}. Falling back to RRF.", e);
                            rerank_results_rrf(&all_results, weights, limit)
                        });
                }
            }
        } else if rc.enabled && rc.strategy == "rrf" {
            return rerank_results_rrf(&all_results, weights, limit);
        }
    }
    
    rerank_results(&all_results, weights, limit)
}

// ── Tests (TDD — ISO 29110) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── SearchRequest deserialization ─────────────────

    #[test]
    fn test_search_request_minimal() {
        let json = r#"{"query": "What is Aspirin?"}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "What is Aspirin?");
        assert!(req.tenant_id.is_none());
        assert!(req.weights.is_none());
        assert!(req.limit.is_none());
        assert!(req.sources.is_none());
        assert!(req.collections.is_none());
    }

    #[test]
    fn test_search_request_full() {
        let json = r#"{
            "query": "Side effects of Aspirin",
            "tenant_id": "megacare",
            "weights": {"vector": 0.6, "tree": 0.2, "graph": 0.2},
            "limit": 20,
            "sources": ["vector", "graph"],
            "collections": ["golden_qa"]
        }"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "Side effects of Aspirin");
        assert_eq!(req.tenant_id, Some("megacare".to_string()));
        assert_eq!(req.limit, Some(20));
        let w = req.weights.unwrap();
        assert_eq!(w.vector, 0.6);
        assert_eq!(req.sources.unwrap().len(), 2);
        assert_eq!(req.collections.unwrap(), vec!["golden_qa"]);
    }

    #[test]
    fn test_search_request_with_custom_weights() {
        let json = r#"{"query": "test", "weights": {"vector": 0.8, "tree": 0.1, "graph": 0.1}}"#;
        let req: SearchRequest = serde_json::from_str(json).unwrap();
        let w = req.weights.unwrap();
        assert_eq!(w.vector, 0.8);
        assert_eq!(w.tree, 0.1);
        assert_eq!(w.graph, 0.1);
        assert!(w.validate().is_ok());
    }

    // ── SearchResponse serialization ─────────────────

    #[test]
    fn test_search_response_serialization() {
        let resp = SearchResponse {
            results: vec![RetrievalResult {
                content: "Aspirin info".to_string(),
                title: "Drug Guide".to_string(),
                score: 0.95,
                source_type: "vector".to_string(),
                metadata: json!({}),
            }],
            distribution: json!({"vector": 1, "tree": 0, "graph": 0, "total": 1}),
            weights_used: EnsembleWeights::default(),
            mode_used: "vector".to_string(),
            latency_ms: 42,
            query: "Aspirin".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["results"][0]["title"], "Drug Guide");
        assert_eq!(json["distribution"]["total"], 1);
        assert_eq!(json["mode_used"], "vector");
        assert_eq!(json["latency_ms"], 42);
        assert_eq!(json["query"], "Aspirin");
        assert_eq!(json["weights_used"]["vector"], 0.5);
    }

    #[test]
    fn test_search_response_empty_results() {
        let resp = SearchResponse {
            results: vec![],
            distribution: json!({"vector": 0, "tree": 0, "graph": 0, "total": 0}),
            weights_used: EnsembleWeights::default(),
            mode_used: "none".to_string(),
            latency_ms: 5,
            query: "nonexistent".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["results"].as_array().unwrap().len(), 0);
        assert_eq!(json["mode_used"], "none");
    }

    // ── resolve_active_sources ────────────────────────

    #[test]
    fn test_resolve_sources_default_all() {
        let active = resolve_active_sources(&None);
        assert!(active.vector);
        assert!(active.graph);
        assert!(active.tree);
    }

    #[test]
    fn test_resolve_sources_empty_list() {
        let active = resolve_active_sources(&Some(vec![]));
        assert!(active.vector);
        assert!(active.graph);
        assert!(active.tree);
    }

    #[test]
    fn test_resolve_sources_vector_only() {
        let active = resolve_active_sources(&Some(vec!["vector".to_string()]));
        assert!(active.vector);
        assert!(!active.graph);
        assert!(!active.tree);
    }

    #[test]
    fn test_resolve_sources_mixed() {
        let active = resolve_active_sources(&Some(vec!["vector".to_string(), "tree".to_string()]));
        assert!(active.vector);
        assert!(!active.graph);
        assert!(active.tree);
    }

    #[test]
    fn test_resolve_sources_case_insensitive() {
        let active = resolve_active_sources(&Some(vec!["Vector".to_string(), "GRAPH".to_string()]));
        assert!(active.vector);
        assert!(active.graph);
        assert!(!active.tree);
    }

    // ── Limit clamping ───────────────────────────────

    #[test]
    fn test_limit_default() {
        let limit = None::<usize>.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        assert_eq!(limit, 10);
    }

    #[test]
    fn test_limit_clamped_to_max() {
        let limit = Some(100usize).unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        assert_eq!(limit, 50);
    }

    #[test]
    fn test_limit_within_range() {
        let limit = Some(25usize).unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        assert_eq!(limit, 25);
    }

    // ── Constants ────────────────────────────────────

    #[test]
    fn test_constants_reasonable() {
        assert!(MAX_LIMIT <= 100, "Max limit should be reasonable");
        assert!(DEFAULT_LIMIT <= MAX_LIMIT, "Default should be <= max");
        assert!(SOURCE_TIMEOUT_SECS >= 5, "Timeout should be at least 5s");
        assert!(
            !DEFAULT_COLLECTIONS.is_empty(),
            "Must have default collections"
        );
    }
}
