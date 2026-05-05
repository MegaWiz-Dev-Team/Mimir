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

use crate::retrieval::graph::{graph_to_retrieval_results, GraphRetriever, Neo4jGraphRetriever, SqlGraphRetriever};
use crate::retrieval::qdrant::{QdrantRetriever, RetrievalResult};
use crate::retrieval::tree::{tree_to_retrieval_results, TreeRetriever};
use crate::retrieval::trace::{self, TraceCollector, TraceEvent};
use crate::retrieval::{determine_mode_used, rerank_results, rerank_results_rrf, source_distribution, EnsembleWeights};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::iam::IamService;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use mimir_core_ai::services::qdrant::QdrantService;
use std::sync::Arc;
use tokio::sync::OnceCell;

// Cached Neo4j service — initialized once on first graph search when USE_NEO4J_GRAPH=true.
static NEO4J_SERVICE: OnceCell<Option<Arc<Neo4jService>>> = OnceCell::const_new();

async fn get_neo4j_service() -> Option<Arc<Neo4jService>> {
    NEO4J_SERVICE
        .get_or_init(|| async {
            if std::env::var("USE_NEO4J_GRAPH").as_deref() == Ok("true") {
                let config = Neo4jConfig::from_env();
                Neo4jService::try_new(&config).await.map(Arc::new)
            } else {
                None
            }
        })
        .await
        .clone()
}

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
    /// Whether to generate an LLM synthesis answer.
    #[serde(default)]
    pub synthesize: Option<bool>,
    /// Optional LLM Provider override (e.g. "google", "ollama").
    #[serde(default)]
    pub provider: Option<String>,
    /// Optional LLM Model ID override.
    #[serde(default)]
    pub model: Option<String>,
    /// Hybrid search alpha (dense vs sparse balance). 0.0=sparse, 1.0=dense. Default: 0.7
    #[serde(default)]
    pub alpha: Option<f64>,
    /// Minimum similarity threshold for vector results. Default: 0.0
    #[serde(default)]
    pub threshold: Option<f64>,
    /// Maximum graph traversal hops (1-3). Default: 2
    #[serde(default)]
    pub hop_limit: Option<i32>,
    /// Whether to include pipeline trace telemetry in the response.
    #[serde(default)]
    pub trace: Option<bool>,
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
    /// The synthesized LLM answer, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthesis: Option<String>,
    /// Pipeline trace telemetry (only present when `trace: true` in request).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_log: Option<Vec<TraceEvent>>,
}

// ── Constants ─────────────────────────────────────────

/// Maximum results allowed per request.
const MAX_LIMIT: usize = 50;

/// Default limit if not specified.
const DEFAULT_LIMIT: usize = 10;

/// Per-source timeout in seconds to prevent one slow source from blocking all.
const SOURCE_TIMEOUT_SECS: u64 = 45;

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
    let trace_enabled = payload.trace.unwrap_or(false);
    let mut trace_collector = TraceCollector::new(trace_enabled);
    
    let alpha = payload.alpha.unwrap_or(0.7);
    let threshold = payload.threshold.unwrap_or(0.0);
    let hop_limit = payload.hop_limit.unwrap_or(2).clamp(1, 3);

    // ── Stage 1: Vector + Graph in parallel via tokio::join! ──

    let ((vector_results, vector_trace), (graph_results, graph_trace)) = tokio::join!(
        // Vector search (with timeout)
        async {
            if !active_sources.vector {
                return (vec![], None);
            }
            let step_start = Instant::now();
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_vector(&query, &tenant_id, &embed_model, &collections, limit, &filters, alpha, threshold),
            )
            .await
            {
                Ok(results) => {
                    let trace_ev = if trace_enabled {
                        let output_summary = if results.is_empty() {
                            "No results".to_string()
                        } else {
                            let top3: Vec<String> = results.iter().take(3)
                                .map(|r| format!("• {} (score: {:.2})", r.title.chars().take(50).collect::<String>(), r.score))
                                .collect();
                            let avg_score = results.iter().map(|r| r.score as f64).sum::<f64>() / results.len() as f64;
                            format!("{} chunks (avg score: {:.2})\n{}", results.len(), avg_score, top3.join("\n"))
                        };

                        Some(trace::trace_success(
                            "Vector Search",
                            step_start,
                            json!({
                                "top_k": limit,
                                "embed_model": &embed_model,
                                "collections": &collections,
                                "weight": format!("{:.2}", weights.vector),
                                "alpha": format!("{:.2}", alpha),
                                "threshold": format!("{:.2}", threshold),
                            }),
                            &query,
                            &output_summary,
                            1,
                            results.len(),
                        ))
                    } else { None };
                    (results, trace_ev)
                }
                Err(_) => {
                    warn!(source = "vector", "⏰ Vector search timed out after {}s", SOURCE_TIMEOUT_SECS);
                    let trace_ev = if trace_enabled { Some(trace::trace_timeout("Vector Search", SOURCE_TIMEOUT_SECS)) } else { None };
                    (vec![], trace_ev)
                }
            }
        },
        // Graph search (with timeout)
        async {
            if !active_sources.graph {
                return (vec![], None);
            }
            let step_start = Instant::now();
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_graph(&query, &tenant_id, &pool, limit, &filters, hop_limit),
            )
            .await
            {
                Ok(results) => {
                    let trace_ev = if trace_enabled {
                        let output_summary = if results.is_empty() {
                            "No relations found".to_string()
                        } else {
                            let top3: Vec<String> = results.iter().take(3)
                                .map(|r| format!("• {} (score: {:.2})", r.title.chars().take(50).collect::<String>(), r.score))
                                .collect();
                            format!("{} relations\n{}", results.len(), top3.join("\n"))
                        };

                        Some(trace::trace_success(
                            "Graph Search",
                            step_start,
                            json!({
                                "hop_limit": hop_limit,
                                "weight": format!("{:.2}", weights.graph),
                            }),
                            &query,
                            &output_summary,
                            1,
                            results.len(),
                        ))
                    } else { None };
                    (results, trace_ev)
                }
                Err(_) => {
                    warn!(source = "graph", "⏰ Graph search timed out after {}s", SOURCE_TIMEOUT_SECS);
                    let trace_ev = if trace_enabled { Some(trace::trace_timeout("Graph Search", SOURCE_TIMEOUT_SECS)) } else { None };
                    (vec![], trace_ev)
                }
            }
        },
    );

    // Collect trace events from Stage 1
    if let Some(ev) = vector_trace { trace_collector.push(ev); }
    if let Some(ev) = graph_trace { trace_collector.push(ev); }

    // ── Stage 2: Tree search using Vector candidates as pre-filter ──
    let tree_results = if active_sources.tree {
        let vector_candidate_titles: Vec<String> = vector_results.iter()
            .map(|r| r.title.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let pool_start = Instant::now();
        // Trace: Unified Candidate Pool
        if trace_enabled {
            let top3_candidates: Vec<String> = vector_candidate_titles.iter().take(5)
                .map(|t| format!("• {}", t))
                .collect();
            let summary_str = if vector_candidate_titles.is_empty() {
                "No candidate titles found".to_string()
            } else {
                format!("{} unique candidate titles:\n{}", vector_candidate_titles.len(), top3_candidates.join("\n"))
            };

            trace_collector.push(trace::trace_success(
                "Unified Candidate Pool",
                pool_start, // near-instant
                json!({"sources": ["vector", "graph"]}),
                &format!("{} vector + {} graph titles", vector_results.len(), graph_results.len()),
                &summary_str,
                vector_results.len() + graph_results.len(),
                vector_candidate_titles.len(),
            ));
        }

        let tree_start = Instant::now();
        let tree_res = fetch_tree(&pool, &tenant_id, &filters, &vector_candidate_titles, &query, &embed_model).await;

        // Trace: Tree Search
        if trace_enabled {
            let output_summary = if tree_res.is_empty() {
                "No results extracted".to_string()
            } else {
                format!(
                    "{} results extracted\n{}",
                    tree_res.len(),
                    tree_res.iter().take(3)
                        .map(|r| format!("• {}", r.title.chars().take(50).collect::<String>()))
                        .collect::<Vec<_>>().join("\n")
                )
            };

            trace_collector.push(trace::trace_success(
                "Tree Search",
                tree_start,
                json!({
                    "strategy": "Vector Routing",
                    "embed_model": &embed_model,
                    "candidate_docs": vector_candidate_titles.len(),
                    "weight": format!("{:.2}", weights.tree),
                }),
                &format!("{} candidate docs", vector_candidate_titles.len()),
                &output_summary,
                vector_candidate_titles.len(),
                tree_res.len(),
            ));
        }

        tree_res
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
    let rerank_start = Instant::now();
    let rerank_strategy_name: &str;
    let ranked = if let Some(ref rc) = payload.rerank {
        if rc.enabled && rc.strategy == "cross-encoder" {
            rerank_strategy_name = "cross-encoder";
            let pre_filtered = rerank_results(&all_results, &weights, (limit * 2).max(20));
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
            rerank_strategy_name = "rrf";
            rerank_results_rrf(&all_results, &weights, limit)
        } else {
            rerank_strategy_name = "weighted";
            rerank_results(&all_results, &weights, limit)
        }
    } else {
        rerank_strategy_name = "weighted";
        rerank_results(&all_results, &weights, limit)
    };

    // Trace: Reranking
    if trace_enabled {
        let rerank_model_name = if let Some(ref rc) = payload.rerank { rc.model.clone() } else { None };
        trace_collector.push(trace::trace_success(
            "Reranking",
            rerank_start,
            json!({
                "strategy": rerank_strategy_name,
                "weights": { 
                    "vector": format!("{:.2}", weights.vector), 
                    "tree": format!("{:.2}", weights.tree), 
                    "graph": format!("{:.2}", weights.graph) 
                },
                "rerank_model": rerank_model_name,
                "final_top_k": limit,
            }),
            &format!("{} raw results", all_results.len()),
            &format!("{} ranked results", ranked.len()),
            all_results.len(),
            ranked.len(),
        ));
    }

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

    let synthesis = if payload.synthesize.unwrap_or(false) && !ranked.is_empty() {
        if let Ok(ref router) = mimir_core_ai::services::llm_router::LlmRouter::new(pool.clone(), &tenant_id).await {
            if let Ok((client, model)) = router.resolve_client_with_overrides("generation", payload.provider.as_deref(), payload.model.as_deref()) {
                let context_parts: Vec<String> = ranked.iter().take(10).enumerate().map(|(i, r)| {
                    format!("Source {}:\nTitle: {}\nContent:\n{}", i+1, r.title, r.content)
                }).collect();
                let context_str = context_parts.join("\n\n---\n\n");
                
                let prompt = format!(
                    "Based on the following retrieved context, answer the user's query comprehensively.\n\nContext:\n{}\n\nQuery: {}",
                    context_str, query
                );
                
                match client.prompt(&model, "You are a helpful expert assistant.", &prompt, 1024, 0.7).await {
                    Ok(response) => Some(response),
                    Err(e) => {
                        tracing::warn!("Synthesis generation failed: {}", e);
                        Some(format!("Failed to generate synthesis: {}", e))
                    }
                }
            } else {
                Some("Failed to resolve generation client".to_string())
            }
        } else {
            None
        }
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(SearchResponse {
            results: ranked,
            distribution,
            weights_used: weights,
            mode_used,
            latency_ms,
            query,
            synthesis,
            trace_log: trace_collector.finish(),
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
    model: &str,
    collections: &[String],
    limit: usize,
    filters: &SearchFilters,
    alpha: f64,
    threshold: f64,
) -> Vec<RetrievalResult> {
    let mut results = Vec::new();
    let qdrant = QdrantService::new();

    let source_ids = filters.source_ids.as_deref();

    for collection in collections {
        let retriever = QdrantRetriever::new(qdrant.clone(), model.to_string(), collection.to_string());
        let result = retriever
            .search_filtered(query, tenant_id, limit, source_ids, alpha, threshold)
            .await;
        match result {
            Ok(mut r) => {
                if collection == "primekg-entities" {
                    for item in &mut r {
                        item.source_type = "primekg".to_string();
                    }
                } else if collection == "clinical-wisdom" {
                    for item in &mut r {
                        item.source_type = "clinical".to_string();
                    }
                }
                results.extend(r);
            }
            Err(e) => warn!(collection = %collection, error = %e, "Vector search failed"),
        }
    }

    results
}

/// Fetch results from the Knowledge Graph.
/// Routes to Neo4jGraphRetriever when USE_NEO4J_GRAPH=true, else SqlGraphRetriever.
async fn fetch_graph(
    query: &str,
    tenant_id: &str,
    pool: &DbPool,
    limit: usize,
    filters: &SearchFilters,
    hop_limit: i32,
) -> Vec<RetrievalResult> {
    let graph_results = if let Some(neo4j) = get_neo4j_service().await {
        let retriever = Neo4jGraphRetriever::new(neo4j);
        retriever.search(query, tenant_id, limit).await
    } else {
        let retriever = SqlGraphRetriever::new(pool.clone());
        retriever.search_with_hops(query, tenant_id, limit, hop_limit).await
    };
    match graph_results {
        Ok(graph_results) => {
            let all = graph_to_retrieval_results(&graph_results);
            // Apply source_id filter post-retrieval
            if let Some(ref ids) = filters.source_ids {
                let mut filtered: Vec<_> = all.into_iter()
                    .filter(|r| {
                        r.metadata.get("source_id")
                            .and_then(|v| v.as_i64())
                            .map(|sid| ids.contains(&sid))
                            .unwrap_or(true) // keep if no source_id in metadata
                    })
                    .collect();
                filtered.truncate(limit);
                filtered
            } else {
                let mut limited = all;
                limited.truncate(limit);
                limited
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
/// Filters documents using `vector_candidate_titles` optionally, then runs the LLM tree extractor.
async fn fetch_tree(
    pool: &DbPool,
    tenant_id: &str,
    filters: &SearchFilters,
    unified_candidate_titles: &[String],
    query: &str,
    embed_model: &str,
) -> Vec<RetrievalResult> {
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

    // Apply unified pre-filter: only search docs whose names match vector or graph results.
    // This limits LLM calls from N docs to max 10 for scalability.
    let filtered_docs: Vec<(String, String, String)> = if !unified_candidate_titles.is_empty() {
        let (matched, rest): (Vec<_>, Vec<_>) = searchable.into_iter()
            .partition(|(name, _, _)| {
                let name_lower = name.to_lowercase();
                unified_candidate_titles.iter().any(|vt| {
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

    let tree_results = retriever.search_parallel(embed_model, &filtered_docs, query).await;
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
    run_parallel_search_filtered(pool, query, tenant_id, weights, limit, &SearchFilters::default(), None, 0.7, 0.0, 2, None).await
}

/// Run the parallel multi-source search with source-level filters.
/// Uses a 2-stage approach:
///   Stage 1: Vector + Graph in parallel
///   Stage 2: Tree search using Vector candidates as pre-filter (max 10 docs)
///
/// `extra_collections`: optional extra Qdrant collections to search in addition to defaults.
///   Pass `Some(&["primekg-entities"])` for agents with PrimeKG tool enabled.
pub async fn run_parallel_search_filtered(
    pool: &DbPool,
    query: &str,
    tenant_id: &str,
    weights: &EnsembleWeights,
    limit: usize,
    filters: &SearchFilters,
    rerank_config: Option<&crate::routes::rag_eval::RerankConfig>,
    alpha: f64,
    threshold: f64,
    hop_limit: i32,
    extra_collections: Option<&[&str]>,
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

    let mut collections: Vec<String> = DEFAULT_COLLECTIONS.iter().map(|s| s.to_string()).collect();
    if let Some(extras) = extra_collections {
        for col in extras {
            let s = col.to_string();
            if !collections.contains(&s) {
                collections.push(s);
            }
        }
    }

    // Stage 1: Fire Vector + Graph in parallel
    let (vector_results, graph_results) = tokio::join!(
        async {
            match tokio::time::timeout(
                std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS),
                fetch_vector(query, tenant_id, &embed_model, &collections, limit, filters, alpha, threshold),
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
                fetch_graph(query, tenant_id, pool, limit, filters, hop_limit),
            )
            .await
            {
                Ok(results) => results,
                Err(_) => vec![],
            }
        },
    );

    // Stage 2: Extract candidate document titles from Vector AND Graph results for Tree pre-filter
    let unified_candidate_titles: Vec<String> = vector_results.iter()
        .chain(graph_results.iter())
        .map(|r| r.title.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let tree_results = match tokio::time::timeout(
        std::time::Duration::from_secs(SOURCE_TIMEOUT_SECS * 2), // Tree gets more time
        fetch_tree(pool, tenant_id, filters, &unified_candidate_titles, query, &embed_model),
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
            synthesis: None,
            trace_log: None,
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
            synthesis: None,
            trace_log: None,
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
