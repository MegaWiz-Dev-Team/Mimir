//! Search Benchmark Route — Batch evaluation with Hit Rate & MRR
//!
//! - POST /api/search/benchmark — Run batch benchmark against eval set
//! - GET  /api/search/benchmark/history — Get past benchmark runs
//! - POST /api/search/benchmark/eval-set — Create/update an eval set
//! - GET  /api/search/benchmark/eval-sets — List eval sets
//!
//! ISO 29110 — Task 2.3: Batch Benchmark API

use axum::{
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{error, info};
use uuid::Uuid;

use crate::retrieval::qdrant::RetrievalResult;
use crate::retrieval::EnsembleWeights;
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;

// ── Request / Response Types ──────────────────────────

/// Request body for POST /api/search/benchmark
#[derive(Debug, Deserialize)]
pub struct BenchmarkRequest {
    /// Load eval items from a stored eval set.
    #[serde(default)]
    pub eval_set_id: Option<String>,
    /// Inline eval items (used if eval_set_id is not provided).
    #[serde(default)]
    pub items: Option<Vec<BenchmarkItem>>,
    /// Ensemble weights for the search. Default: {0.5, 0.3, 0.2}.
    #[serde(default)]
    pub weights: Option<EnsembleWeights>,
    /// Top-K for hit rate calculation. Default: 5.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Tenant ID override.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Label for this benchmark run (e.g., "round-1").
    #[serde(default)]
    pub label: Option<String>,
}

/// A single benchmark evaluation item (ground truth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkItem {
    /// The search query to test.
    pub query: String,
    /// Expected document titles that should appear in results.
    pub expected_titles: Vec<String>,
    /// Optional: expected content snippet for deeper matching.
    #[serde(default)]
    pub expected_content: Option<String>,
}

/// Full benchmark response with metrics.
#[derive(Debug, Serialize)]
pub struct BenchmarkResponse {
    /// Unique ID for this benchmark run.
    pub benchmark_id: String,
    /// Total number of queries evaluated.
    pub total_queries: usize,
    /// Hit Rate: % of queries where at least 1 relevant doc in top-K.
    pub hit_rate: f64,
    /// Mean Reciprocal Rank: average of 1/rank of first relevant result.
    pub mrr: f64,
    /// Average search latency in milliseconds.
    pub avg_latency_ms: f64,
    /// Weights used for this benchmark.
    pub weights_used: EnsembleWeights,
    /// Per-query breakdown.
    pub per_query: Vec<QueryBenchmarkResult>,
    /// User-assigned label.
    pub label: Option<String>,
}

/// Result of a single query benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryBenchmarkResult {
    /// The query that was tested.
    pub query: String,
    /// Whether any expected document appeared in top-K results.
    pub hit: bool,
    /// 1/rank of the first relevant result, or 0 if no hit.
    pub reciprocal_rank: f64,
    /// Search latency for this query in milliseconds.
    pub latency_ms: u64,
    /// Titles of the top-K results returned.
    pub top_results: Vec<String>,
    /// Rank at which the first expected document was found (1-indexed).
    pub matched_at_rank: Option<usize>,
}

/// Request body for creating an eval set.
#[derive(Debug, Deserialize)]
pub struct CreateEvalSetRequest {
    /// Human-readable name for the eval set.
    pub name: String,
    /// Description of what this eval set tests.
    #[serde(default)]
    pub description: Option<String>,
    /// The evaluation items.
    pub items: Vec<BenchmarkItem>,
    /// Tenant ID override.
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// Query params for listing eval sets
#[derive(Debug, Deserialize)]
pub struct EvalSetListQuery {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
}

/// Query params for benchmark history
#[derive(Debug, Deserialize)]
pub struct BenchmarkHistoryQuery {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
}

// ── Constants ─────────────────────────────────────────

const DEFAULT_BENCHMARK_LIMIT: usize = 5;
const MAX_BENCHMARK_ITEMS: usize = 500;

// ── Route Registration ───────────────────────────────

pub fn search_benchmark_routes() -> Router<DbPool> {
    Router::new()
        .route("/api/search/benchmark", post(benchmark_handler))
        .route("/api/search/benchmark/history", get(benchmark_history))
        .route("/api/search/benchmark/eval-set", post(create_eval_set))
        .route("/api/search/benchmark/eval-sets", get(list_eval_sets))
}

// ── Scoring Functions (Pure — No I/O) ────────────────

/// Calculate Hit Rate: fraction of queries with at least 1 hit in top-K.
///
/// Formula: hit_rate = count(queries_with_hit) / total_queries
pub fn calculate_hit_rate(results: &[QueryBenchmarkResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    let hits = results.iter().filter(|r| r.hit).count();
    hits as f64 / results.len() as f64
}

/// Calculate Mean Reciprocal Rank (MRR).
///
/// Formula: MRR = (1/N) * Σ (1/rank_of_first_relevant_result)
/// If no relevant result found, reciprocal_rank = 0.
pub fn calculate_mrr(results: &[QueryBenchmarkResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    let sum_rr: f64 = results.iter().map(|r| r.reciprocal_rank).sum();
    sum_rr / results.len() as f64
}

/// Check if a retrieved title matches any expected title (fuzzy, case-insensitive).
///
/// Uses substring matching: the expected title is a substring of the retrieved title
/// or vice versa.
pub fn title_matches(retrieved: &str, expected_titles: &[String]) -> bool {
    let retrieved_lower = retrieved.to_lowercase();
    expected_titles.iter().any(|expected| {
        let expected_lower = expected.to_lowercase();
        retrieved_lower.contains(&expected_lower) || expected_lower.contains(&retrieved_lower)
    })
}

/// Evaluate a single query's results against expected titles.
///
/// Returns (hit, reciprocal_rank, matched_at_rank).
pub fn evaluate_query_results(
    results: &[RetrievalResult],
    expected_titles: &[String],
) -> (bool, f64, Option<usize>) {
    for (i, result) in results.iter().enumerate() {
        if title_matches(&result.title, expected_titles) {
            let rank = i + 1; // 1-indexed
            return (true, 1.0 / rank as f64, Some(rank));
        }
    }
    (false, 0.0, None)
}

// ── Handler ──────────────────────────────────────────

/// POST /api/search/benchmark — Run batch benchmark
async fn benchmark_handler(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<BenchmarkRequest>,
) -> impl IntoResponse {
    let start = Instant::now();
    let benchmark_id = Uuid::new_v4().to_string();

    // Resolve tenant
    let tenant_id = tenant_ctx
        .as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or(payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    let limit = payload.limit.unwrap_or(DEFAULT_BENCHMARK_LIMIT);
    let weights = payload.weights.clone().unwrap_or_default();

    // Load eval items: from eval_set_id or inline items
    let items = match load_eval_items(&pool, &tenant_id, &payload).await {
        Ok(items) => items,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": e
                })),
            )
                .into_response();
        }
    };

    if items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "No evaluation items provided. Supply eval_set_id or inline items."
            })),
        )
            .into_response();
    }

    if items.len() > MAX_BENCHMARK_ITEMS {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Too many items. Maximum is {}.", MAX_BENCHMARK_ITEMS)
            })),
        )
            .into_response();
    }

    info!(
        event = "benchmark_start",
        benchmark_id = %benchmark_id,
        total_queries = items.len(),
        limit = limit,
        label = ?payload.label,
        "📊 Starting benchmark run"
    );

    // Run each query through the search pipeline
    let mut per_query_results = Vec::new();

    for item in &items {
        let query_start = Instant::now();

        // Use the same parallel search logic as Task 2.1
        let search_results = crate::routes::search::run_parallel_search(
            &pool,
            &item.query,
            &tenant_id,
            &weights,
            limit,
        )
        .await;

        let query_latency = query_start.elapsed().as_millis() as u64;

        let (hit, rr, matched_rank) =
            evaluate_query_results(&search_results, &item.expected_titles);
        let top_titles: Vec<String> = search_results.iter().map(|r| r.title.clone()).collect();

        per_query_results.push(QueryBenchmarkResult {
            query: item.query.clone(),
            hit,
            reciprocal_rank: rr,
            latency_ms: query_latency,
            top_results: top_titles,
            matched_at_rank: matched_rank,
        });
    }

    // Calculate aggregate metrics
    let hit_rate = calculate_hit_rate(&per_query_results);
    let mrr = calculate_mrr(&per_query_results);
    let avg_latency = if per_query_results.is_empty() {
        0.0
    } else {
        per_query_results
            .iter()
            .map(|r| r.latency_ms as f64)
            .sum::<f64>()
            / per_query_results.len() as f64
    };

    // Persist benchmark result to database
    let _ = persist_benchmark(
        &pool,
        &benchmark_id,
        &tenant_id,
        &payload,
        hit_rate,
        mrr,
        avg_latency,
        &per_query_results,
    )
    .await;

    let total_latency = start.elapsed().as_millis() as u64;

    info!(
        event = "benchmark_complete",
        benchmark_id = %benchmark_id,
        hit_rate = hit_rate,
        mrr = mrr,
        total_queries = per_query_results.len(),
        total_latency_ms = total_latency,
        "✅ Benchmark completed"
    );

    (
        StatusCode::OK,
        Json(BenchmarkResponse {
            benchmark_id,
            total_queries: per_query_results.len(),
            hit_rate,
            mrr,
            avg_latency_ms: avg_latency,
            weights_used: weights,
            per_query: per_query_results,
            label: payload.label.clone(),
        }),
    )
        .into_response()
}

// ── Data Loading ─────────────────────────────────────

/// Load evaluation items from either an eval_set_id or inline items.
async fn load_eval_items(
    pool: &DbPool,
    tenant_id: &str,
    request: &BenchmarkRequest,
) -> Result<Vec<BenchmarkItem>, String> {
    // Priority 1: inline items
    if let Some(ref items) = request.items {
        if !items.is_empty() {
            return Ok(items.clone());
        }
    }

    // Priority 2: eval_set_id from database
    if let Some(ref eval_set_id) = request.eval_set_id {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT items FROM eval_sets WHERE id = ? AND tenant_id = ?")
                .bind(eval_set_id)
                .bind(tenant_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("DB error loading eval set: {}", e))?;

        match row {
            Some((items_json,)) => {
                let items: Vec<BenchmarkItem> = serde_json::from_str(&items_json)
                    .map_err(|e| format!("Invalid eval set JSON: {}", e))?;
                Ok(items)
            }
            None => Err(format!(
                "Eval set '{}' not found for tenant '{}'",
                eval_set_id, tenant_id
            )),
        }
    } else {
        Ok(vec![])
    }
}

/// Persist benchmark results to the search_benchmarks table.
async fn persist_benchmark(
    pool: &DbPool,
    benchmark_id: &str,
    tenant_id: &str,
    request: &BenchmarkRequest,
    hit_rate: f64,
    mrr: f64,
    avg_latency: f64,
    per_query: &[QueryBenchmarkResult],
) -> Result<(), String> {
    let weights_json = serde_json::to_string(&request.weights).unwrap_or_default();
    let per_query_json = serde_json::to_string(per_query).unwrap_or_default();

    sqlx::query(
        r#"INSERT INTO search_benchmarks
            (id, tenant_id, eval_set_id, label, hit_rate, mrr, total_queries, avg_latency_ms, weights_json, per_query_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
    )
    .bind(benchmark_id)
    .bind(tenant_id)
    .bind(&request.eval_set_id)
    .bind(&request.label)
    .bind(hit_rate)
    .bind(mrr)
    .bind(per_query.len() as i32)
    .bind(avg_latency)
    .bind(&weights_json)
    .bind(&per_query_json)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to persist benchmark: {}", e))?;

    Ok(())
}

// ── Eval Set Management ──────────────────────────────

/// POST /api/search/benchmark/eval-set — Create or update an evaluation set
async fn create_eval_set(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<CreateEvalSetRequest>,
) -> impl IntoResponse {
    let tenant_id = tenant_ctx
        .as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or(payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    let id = Uuid::new_v4().to_string();
    let items_json = serde_json::to_string(&payload.items).unwrap_or_default();

    match sqlx::query(
        "INSERT INTO eval_sets (id, tenant_id, name, description, items) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&tenant_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&items_json)
    .execute(&pool)
    .await
    {
        Ok(_) => {
            info!(event = "eval_set_created", id = %id, name = %payload.name, items = payload.items.len());
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id,
                    "name": payload.name,
                    "items_count": payload.items.len(),
                })),
            )
                .into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to create eval set");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to create eval set: {}", e)
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/search/benchmark/eval-sets — List evaluation sets
async fn list_eval_sets(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(params): Query<EvalSetListQuery>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let sets: Vec<(
        String,
        String,
        String,
        Option<String>,
        String,
        Option<chrono::NaiveDateTime>,
    )> = sqlx::query_as(
        "SELECT id, tenant_id, name, description, items, created_at \
         FROM eval_sets WHERE tenant_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let eval_sets: Vec<Value> = sets
        .iter()
        .map(|(id, _, name, desc, items_json, created)| {
            let items_count = serde_json::from_str::<Vec<Value>>(items_json)
                .map(|v| v.len())
                .unwrap_or(0);
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "items_count": items_count,
                "created_at": created,
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "eval_sets": eval_sets,
            "page": page,
            "per_page": per_page,
        })),
    )
        .into_response()
}

/// GET /api/search/benchmark/history — Get past benchmark runs
async fn benchmark_history(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(params): Query<BenchmarkHistoryQuery>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let mut query = "SELECT id, eval_set_id, label, hit_rate, mrr, total_queries, avg_latency_ms, weights_json, created_at \
        FROM search_benchmarks WHERE tenant_id = ?".to_string();

    if params.label.is_some() {
        query.push_str(" AND label = ?");
    }
    query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            f64,
            f64,
            i32,
            f64,
            Option<String>,
            Option<chrono::NaiveDateTime>,
        ),
    >(&query)
    .bind(&tenant_id);

    if let Some(ref label) = params.label {
        q = q.bind(label);
    }

    let runs = q
        .bind(per_page)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let history: Vec<Value> = runs
        .iter()
        .map(
            |(id, eval_set_id, label, hr, mrr, total, latency, weights, created)| {
                json!({
                    "benchmark_id": id,
                    "eval_set_id": eval_set_id,
                    "label": label,
                    "hit_rate": hr,
                    "mrr": mrr,
                    "total_queries": total,
                    "avg_latency_ms": latency,
                    "weights": weights.as_ref().and_then(|w| serde_json::from_str::<Value>(w).ok()),
                    "created_at": created,
                })
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "benchmarks": history,
            "page": page,
            "per_page": per_page,
        })),
    )
        .into_response()
}

// ── Tests (TDD — ISO 29110) ─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_result(title: &str, score: f32, source: &str) -> RetrievalResult {
        RetrievalResult {
            content: format!("Content for {}", title),
            title: title.to_string(),
            score,
            source_type: source.to_string(),
            metadata: json!({}),
        }
    }

    // ── Hit Rate ──────────────────────────────────────

    #[test]
    fn test_hit_rate_all_hits() {
        let results = vec![
            QueryBenchmarkResult {
                query: "q1".into(),
                hit: true,
                reciprocal_rank: 1.0,
                latency_ms: 10,
                top_results: vec![],
                matched_at_rank: Some(1),
            },
            QueryBenchmarkResult {
                query: "q2".into(),
                hit: true,
                reciprocal_rank: 0.5,
                latency_ms: 20,
                top_results: vec![],
                matched_at_rank: Some(2),
            },
        ];
        assert!((calculate_hit_rate(&results) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hit_rate_no_hits() {
        let results = vec![
            QueryBenchmarkResult {
                query: "q1".into(),
                hit: false,
                reciprocal_rank: 0.0,
                latency_ms: 10,
                top_results: vec![],
                matched_at_rank: None,
            },
            QueryBenchmarkResult {
                query: "q2".into(),
                hit: false,
                reciprocal_rank: 0.0,
                latency_ms: 20,
                top_results: vec![],
                matched_at_rank: None,
            },
        ];
        assert!((calculate_hit_rate(&results) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hit_rate_partial() {
        let results = vec![
            QueryBenchmarkResult {
                query: "q1".into(),
                hit: true,
                reciprocal_rank: 1.0,
                latency_ms: 10,
                top_results: vec![],
                matched_at_rank: Some(1),
            },
            QueryBenchmarkResult {
                query: "q2".into(),
                hit: false,
                reciprocal_rank: 0.0,
                latency_ms: 20,
                top_results: vec![],
                matched_at_rank: None,
            },
        ];
        assert!((calculate_hit_rate(&results) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hit_rate_empty() {
        assert!((calculate_hit_rate(&[]) - 0.0).abs() < f64::EPSILON);
    }

    // ── MRR ───────────────────────────────────────────

    #[test]
    fn test_mrr_first_position() {
        let results = vec![QueryBenchmarkResult {
            query: "q1".into(),
            hit: true,
            reciprocal_rank: 1.0,
            latency_ms: 10,
            top_results: vec![],
            matched_at_rank: Some(1),
        }];
        assert!((calculate_mrr(&results) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mrr_second_position() {
        let results = vec![QueryBenchmarkResult {
            query: "q1".into(),
            hit: true,
            reciprocal_rank: 0.5,
            latency_ms: 10,
            top_results: vec![],
            matched_at_rank: Some(2),
        }];
        assert!((calculate_mrr(&results) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mrr_mixed() {
        // Query 1: hit at rank 1 (RR=1.0), Query 2: hit at rank 3 (RR=0.333)
        let results = vec![
            QueryBenchmarkResult {
                query: "q1".into(),
                hit: true,
                reciprocal_rank: 1.0,
                latency_ms: 10,
                top_results: vec![],
                matched_at_rank: Some(1),
            },
            QueryBenchmarkResult {
                query: "q2".into(),
                hit: true,
                reciprocal_rank: 1.0 / 3.0,
                latency_ms: 20,
                top_results: vec![],
                matched_at_rank: Some(3),
            },
        ];
        let expected = (1.0 + 1.0 / 3.0) / 2.0;
        assert!((calculate_mrr(&results) - expected).abs() < 0.001);
    }

    #[test]
    fn test_mrr_no_results() {
        let results = vec![QueryBenchmarkResult {
            query: "q1".into(),
            hit: false,
            reciprocal_rank: 0.0,
            latency_ms: 10,
            top_results: vec![],
            matched_at_rank: None,
        }];
        assert!((calculate_mrr(&results) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mrr_empty() {
        assert!((calculate_mrr(&[]) - 0.0).abs() < f64::EPSILON);
    }

    // ── Title Matching ────────────────────────────────

    #[test]
    fn test_fuzzy_title_match_exact() {
        assert!(title_matches(
            "Aspirin Overview",
            &["Aspirin Overview".to_string()]
        ));
    }

    #[test]
    fn test_fuzzy_title_match_case_insensitive() {
        assert!(title_matches(
            "ASPIRIN Overview",
            &["aspirin overview".to_string()]
        ));
    }

    #[test]
    fn test_fuzzy_title_match_substring() {
        assert!(title_matches(
            "Drug Guide: Aspirin Overview 2024",
            &["aspirin".to_string()]
        ));
    }

    #[test]
    fn test_fuzzy_title_match_reverse_substring() {
        assert!(title_matches(
            "Aspirin",
            &["Aspirin Drug Information".to_string()]
        ));
    }

    #[test]
    fn test_fuzzy_title_no_match() {
        assert!(!title_matches("Ibuprofen Guide", &["aspirin".to_string()]));
    }

    #[test]
    fn test_fuzzy_title_match_multiple_expected() {
        assert!(title_matches(
            "Aspirin Info",
            &["paracetamol".to_string(), "aspirin".to_string()]
        ));
    }

    // ── evaluate_query_results ────────────────────────

    #[test]
    fn test_evaluate_hit_at_rank_1() {
        let results = vec![
            make_result("Aspirin Guide", 0.9, "vector"),
            make_result("Other Doc", 0.7, "tree"),
        ];
        let (hit, rr, rank) = evaluate_query_results(&results, &["Aspirin".to_string()]);
        assert!(hit);
        assert!((rr - 1.0).abs() < f64::EPSILON);
        assert_eq!(rank, Some(1));
    }

    #[test]
    fn test_evaluate_hit_at_rank_3() {
        let results = vec![
            make_result("Unrelated A", 0.9, "vector"),
            make_result("Unrelated B", 0.8, "tree"),
            make_result("Aspirin Side Effects", 0.7, "graph"),
        ];
        let (hit, rr, rank) = evaluate_query_results(&results, &["Aspirin".to_string()]);
        assert!(hit);
        assert!((rr - 1.0 / 3.0).abs() < 0.001);
        assert_eq!(rank, Some(3));
    }

    #[test]
    fn test_evaluate_no_hit() {
        let results = vec![make_result("Unrelated", 0.9, "vector")];
        let (hit, rr, rank) = evaluate_query_results(&results, &["Aspirin".to_string()]);
        assert!(!hit);
        assert!((rr - 0.0).abs() < f64::EPSILON);
        assert_eq!(rank, None);
    }

    #[test]
    fn test_evaluate_empty_results() {
        let (hit, rr, rank) = evaluate_query_results(&[], &["Aspirin".to_string()]);
        assert!(!hit);
        assert!((rr - 0.0).abs() < f64::EPSILON);
        assert_eq!(rank, None);
    }

    // ── BenchmarkItem serialization ──────────────────

    #[test]
    fn test_benchmark_item_roundtrip() {
        let item = BenchmarkItem {
            query: "What is Aspirin?".to_string(),
            expected_titles: vec!["Aspirin Guide".to_string(), "Drug Info".to_string()],
            expected_content: Some("NSAID".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let restored: BenchmarkItem = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.query, item.query);
        assert_eq!(restored.expected_titles.len(), 2);
    }

    #[test]
    fn test_benchmark_item_no_expected_content() {
        let json = r#"{"query": "test", "expected_titles": ["doc1"]}"#;
        let item: BenchmarkItem = serde_json::from_str(json).unwrap();
        assert!(item.expected_content.is_none());
    }

    // ── BenchmarkRequest deserialization ──────────────

    #[test]
    fn test_benchmark_request_inline_items() {
        let json = r#"{
            "items": [{"query": "q1", "expected_titles": ["Aspirin"]}],
            "limit": 5,
            "label": "test-round-1"
        }"#;
        let req: BenchmarkRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.items.as_ref().unwrap().len(), 1);
        assert_eq!(req.label, Some("test-round-1".to_string()));
    }

    #[test]
    fn test_benchmark_request_eval_set() {
        let json = r#"{"eval_set_id": "abc-123"}"#;
        let req: BenchmarkRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.eval_set_id, Some("abc-123".to_string()));
        assert!(req.items.is_none());
    }

    // ── BenchmarkResponse serialization ──────────────

    #[test]
    fn test_benchmark_response_serialization() {
        let resp = BenchmarkResponse {
            benchmark_id: "bench-1".to_string(),
            total_queries: 2,
            hit_rate: 0.5,
            mrr: 0.75,
            avg_latency_ms: 50.0,
            weights_used: EnsembleWeights::default(),
            per_query: vec![],
            label: Some("test".to_string()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["hit_rate"], 0.5);
        assert_eq!(json["mrr"], 0.75);
        assert_eq!(json["label"], "test");
    }

    // ── Constants ────────────────────────────────────

    #[test]
    fn test_benchmark_constants() {
        assert!(
            DEFAULT_BENCHMARK_LIMIT >= 3,
            "Default K should be at least 3"
        );
        assert!(
            MAX_BENCHMARK_ITEMS >= 100,
            "Should support at least 100 items"
        );
    }
}
