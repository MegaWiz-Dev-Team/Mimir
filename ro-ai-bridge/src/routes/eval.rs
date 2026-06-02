//! Evaluation API routes
//!
//! - GET  /api/v1/eval/runs                          — List runs
//! - POST /api/v1/eval/runs                          — Trigger new run
//! - GET  /api/v1/eval/runs/:id                      — Run detail + summaries
//! - GET  /api/v1/eval/runs/:id/scores               — Individual scores (filterable)
//! - GET  /api/v1/eval/runs/:id/matrix               — Heatmap data
//! - PATCH /api/v1/eval/scores/:id/review            — Human review override
//! - GET  /api/v1/eval/benchmark-datasets            — List benchmark datasets
//! - GET  /api/v1/eval/benchmark-datasets/:id        — Get dataset detail

use axum::{
    extract::{Extension, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};
use chrono::{DateTime, Utc};
use mimir_core_ai::evaluation::runner::{start_evaluation_run, EvaluatorParams};
use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use mimir_core_ai::services::db::DbPool;

// ─── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct EvalRun {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub total_combinations: i32,
    pub completed_combinations: i32,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub config: Option<String>,
    // ─── Wave 1: lineage + champion + cost ───
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub baseline_run_id: Option<String>,
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub variable_under_test: Option<String>,
    #[serde(default)]
    pub expected_change: Option<String>,
    #[serde(default)]
    pub is_champion: bool,
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EvalScore {
    pub id: i64,
    pub run_id: String,
    pub agent_name: String,
    pub model_id: String,
    pub question: String,
    pub expected_answer: String,
    pub actual_answer: Option<String>,
    // Standard 1-5 scores
    pub accuracy_score: Option<i8>,
    pub completeness_score: Option<i8>,
    pub relevance_score: Option<i8>,
    // HealthBench-style
    pub safety_score: Option<i32>,
    pub rubric_score: Option<f32>,
    pub rubric_items: Option<String>,
    pub tags: Option<String>,
    pub latency_ms: Option<i32>,
    pub judge_model: Option<String>,
    pub judge_reasoning: Option<String>,
    // Wave 1 — drill-down + reproducibility
    #[serde(default)]
    pub retrieval_trace: Option<String>,
    #[serde(default)]
    pub benchmark_item_id: Option<String>,
    #[serde(default)]
    pub replicate_index: Option<i32>,
    // Wave 3 — full retrieval params + chunks + timings
    #[serde(default)]
    pub retrieval_params: Option<String>,
    #[serde(default)]
    pub retrieval_chunks: Option<String>,
    #[serde(default)]
    pub step_timings: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<String>,
    // Human overrides
    pub human_accuracy_score: Option<i8>,
    pub human_completeness_score: Option<i8>,
    pub human_relevance_score: Option<i8>,
    pub human_safety_score: Option<i32>,
    pub human_notes: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EvalSummary {
    pub id: i64,
    pub run_id: String,
    pub agent_name: String,
    pub model_id: String,
    pub total_questions: i32,
    pub avg_accuracy: Option<f32>,
    pub avg_completeness: Option<f32>,
    pub avg_relevance: Option<f32>,
    pub avg_safety_score: Option<f32>,
    pub min_safety_score: Option<i32>,
    pub unsafe_count: i32,
    pub avg_latency_ms: Option<f32>,
    pub overall_score: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct RunDetailResponse {
    pub run: EvalRun,
    pub summaries: Vec<EvalSummary>,
}

#[derive(Debug, Serialize)]
pub struct MatrixCell {
    pub agent_name: String,
    pub model_id: String,
    pub overall_score: Option<f32>,
    pub avg_accuracy: Option<f32>,
    pub avg_completeness: Option<f32>,
    pub avg_relevance: Option<f32>,
    pub avg_safety_score: Option<f32>,
    pub unsafe_count: i32,
    pub avg_latency_ms: Option<f32>,
    pub total_questions: i32,
}

#[derive(Debug, Serialize)]
pub struct MatrixResponse {
    pub agents: Vec<String>,
    pub models: Vec<String>,
    pub cells: Vec<MatrixCell>,
}

#[derive(Debug, Deserialize)]
pub struct ScoresQuery {
    pub agent: Option<String>,
    pub model: Option<String>,
    /// Filter to only unsafe responses (safety_score < 0)
    pub unsafe_only: Option<bool>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct HumanReviewRequest {
    pub accuracy_score: Option<i8>,
    pub completeness_score: Option<i8>,
    pub relevance_score: Option<i8>,
    pub safety_score: Option<i32>,
    pub notes: Option<String>,
    pub reviewed_by: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BenchmarkDataset {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub source: String,
    /// Scoring function — `healthbench_likert`, `mcq_accuracy`, `binary_yes_no`,
    /// `paper_rubric_pct`. Drives UI rubric-aware metric column + per-benchmark
    /// rank computation (Sprint 40 B-36d/c).
    pub scoring_fn: String,
    pub description: Option<String>,
    pub total_items: i32,
    pub version: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BenchmarkDatasetDetail {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub source: String,
    pub scoring_fn: String,
    pub description: Option<String>,
    pub items: String,
    pub total_items: i32,
    pub version: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Router ────────────────────────────────────────────────────────────────────

pub fn eval_routes() -> Router<DbPool> {
    Router::new()
        .route("/api/v1/eval/runs", get(list_runs).post(start_run))
        .route("/api/v1/eval/runs/{id}", get(get_run_detail))
        .route("/api/v1/eval/runs/{id}/scores", get(get_run_scores))
        // Phase 4 finetune export: rejection-sample a run's scored items into an
        // SFT dataset (JSONL). e.g. ?min_acc=4&gt_match=1&context=1
        .route("/api/v1/eval/runs/{id}/finetune-export", get(finetune_export_run))
        .route("/api/v1/eval/runs/{id}/matrix", get(get_run_matrix))
        .route("/api/v1/eval/runs/{id}/lock-items", axum::routing::post(get_lock_items))
        .route("/api/v1/eval/runs/{id}/promote", axum::routing::post(promote_run))
        // Sprint 37 B-24 replay-judge: re-judge existing eval_scores rows of a run
        // with a different judge config (e.g. ensemble) — clean A/B without re-running
        // the agent inference (which is the expensive part + source of variance).
        .route("/api/v1/eval/runs/{id}/rejudge", axum::routing::post(rejudge_run))
        // Sprint 47 B-47f: bottleneck attribution — given a run (or a paired
        // A/B baseline), classify each question as RAG-bottleneck / LLM-
        // bottleneck / both / neither so the dashboard can show where the
        // next sprint should invest.
        .route("/api/v1/eval/runs/{id}/diagnose", get(diagnose_run))
        .route("/api/v1/eval/scores/{id}/review", patch(submit_review))
        .route("/api/v1/eval/champion", get(get_champion))
        .route("/api/v1/eval/benchmark-datasets", get(list_benchmark_datasets))
        .route("/api/v1/eval/benchmark-datasets/{id}", get(get_benchmark_dataset))
        .layer(axum::middleware::from_fn(tenant_auth_middleware))
}

// ─── Handlers ──────────────────────────────────────────────────────────────────

async fn list_runs(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
) -> Json<Vec<EvalRun>> {
    let runs = sqlx::query_as::<_, EvalRun>(
        "SELECT id, name, status, total_combinations, completed_combinations, started_at, finished_at, config,
                parent_run_id, baseline_run_id, hypothesis, variable_under_test, expected_change,
                is_champion, CAST(total_cost_usd AS DOUBLE) AS total_cost_usd
         FROM eval_runs WHERE tenant_id = ? ORDER BY started_at DESC",
    )
    .bind(&tenant.tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "list_runs_failed", tenant = %tenant.tenant_id, error = %e);
        vec![]
    });

    Json(runs)
}

async fn get_run_detail(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
) -> Json<Option<RunDetailResponse>> {
    let run = sqlx::query_as::<_, EvalRun>(
        "SELECT id, name, status, total_combinations, completed_combinations, started_at, finished_at, config,
                parent_run_id, baseline_run_id, hypothesis, variable_under_test, expected_change,
                is_champion, CAST(total_cost_usd AS DOUBLE) AS total_cost_usd
         FROM eval_runs WHERE id = ? AND tenant_id = ?",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let Some(run) = run else {
        return Json(None);
    };

    let summaries = sqlx::query_as::<_, EvalSummary>(
        "SELECT id, run_id, agent_name, model_id, total_questions,
                avg_accuracy, avg_completeness, avg_relevance,
                avg_safety_score, min_safety_score, unsafe_count,
                avg_latency_ms, overall_score
         FROM eval_summary WHERE run_id = ? ORDER BY overall_score DESC",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Json(Some(RunDetailResponse { run, summaries }))
}

async fn get_run_scores(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
    Query(q): Query<ScoresQuery>,
) -> Json<Vec<EvalScore>> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = (page - 1) * limit;

    let mut query_str = String::from(
        "SELECT id, run_id, agent_name, model_id, question, expected_answer, actual_answer,
                accuracy_score, completeness_score, relevance_score,
                safety_score, rubric_score, rubric_items, tags,
                latency_ms, judge_model, judge_reasoning,
                retrieval_trace, benchmark_item_id, replicate_index,
                retrieval_params, retrieval_chunks, step_timings, tool_calls,
                human_accuracy_score, human_completeness_score, human_relevance_score,
                human_safety_score, human_notes, reviewed_by, reviewed_at, created_at
         FROM eval_scores WHERE run_id = ? AND tenant_id = ?",
    );

    if q.agent.is_some() {
        query_str.push_str(" AND agent_name = ?");
    }
    if q.model.is_some() {
        query_str.push_str(" AND model_id = ?");
    }
    if q.unsafe_only.unwrap_or(false) {
        query_str.push_str(" AND safety_score < 0");
    }
    query_str.push_str(&format!(
        " ORDER BY agent_name, model_id, id LIMIT {} OFFSET {}",
        limit, offset
    ));

    let mut query = sqlx::query_as::<_, EvalScore>(&query_str)
        .bind(&id)
        .bind(&tenant.tenant_id);

    if let Some(ref agent) = q.agent {
        query = query.bind(agent);
    }
    if let Some(ref model) = q.model {
        query = query.bind(model);
    }

    Json(query.fetch_all(&pool).await.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct FinetuneExportQuery {
    /// Minimum effective accuracy (1-5). Default 4.
    min_acc: Option<i8>,
    /// Minimum completeness; defaults to `min_acc`.
    min_comp: Option<i8>,
    /// Minimum relevance; defaults to `min_acc`.
    min_rel: Option<i8>,
    /// Optional safety floor; omit to ignore safety entirely.
    min_safety: Option<i8>,
    /// `1` to require the answer to match the benchmark ground truth.
    gt_match: Option<u8>,
    /// `1` to fold retrieved context into the user turn (open-book SFT).
    context: Option<u8>,
    /// `messages` (default) or `prompt_completion`.
    format: Option<String>,
    /// `1` to return only the rejection-sampling stats (preview yield, no body).
    stats_only: Option<u8>,
}

/// GET /api/v1/eval/runs/{id}/finetune-export
///
/// Rejection-samples the run's scored items into an SFT dataset (JSONL),
/// keeping only rows that clear the score thresholds (and optionally match
/// ground truth). Accounting is returned in `X-Export-*` headers; pass
/// `stats_only=1` to preview the yield as JSON without downloading the body.
async fn finetune_export_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
    Query(q): Query<FinetuneExportQuery>,
) -> impl IntoResponse {
    use mimir_core_ai::evaluation::finetune_export::{
        export_run_to_jsonl, ExportFormat, ExportOptions,
    };

    let truthy = |o: Option<u8>| o.map(|n| n != 0).unwrap_or(false);
    let min_acc = q.min_acc.unwrap_or(4);
    let format = match q.format.as_deref() {
        Some("prompt_completion") | Some("prompt-completion") => ExportFormat::PromptCompletion,
        _ => ExportFormat::OpenAiMessages,
    };
    let opts = ExportOptions {
        min_accuracy: min_acc,
        min_completeness: q.min_comp.unwrap_or(min_acc),
        min_relevance: q.min_rel.unwrap_or(min_acc),
        min_safety: q.min_safety,
        require_ground_truth_match: truthy(q.gt_match),
        include_context: truthy(q.context),
        format,
        ..Default::default()
    };

    match export_run_to_jsonl(&pool, &id, &tenant.tenant_id, &opts).await {
        Ok((jsonl, stats)) => {
            if truthy(q.stats_only) {
                return Json(serde_json::json!({ "run_id": id, "stats": stats })).into_response();
            }
            let mut hm = HeaderMap::new();
            hm.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/x-ndjson"),
            );
            if let Ok(cd) =
                HeaderValue::from_str(&format!("attachment; filename=\"finetune_{}.jsonl\"", id))
            {
                hm.insert(header::CONTENT_DISPOSITION, cd);
            }
            let mut set = |k: &'static str, v: usize| {
                if let Ok(val) = HeaderValue::from_str(&v.to_string()) {
                    hm.insert(k, val);
                }
            };
            set("x-export-total", stats.total);
            set("x-export-kept", stats.kept);
            set("x-export-dropped-low-score", stats.dropped_low_score);
            set("x-export-dropped-no-answer", stats.dropped_no_answer);
            set("x-export-dropped-gt-mismatch", stats.dropped_gt_mismatch);
            (StatusCode::OK, hm, jsonl).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_run_matrix(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
) -> Json<MatrixResponse> {
    let run_exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM eval_runs WHERE id = ? AND tenant_id = ?")
            .bind(&id)
            .bind(&tenant.tenant_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

    if run_exists.is_none() {
        return Json(MatrixResponse { agents: vec![], models: vec![], cells: vec![] });
    }

    let summaries = sqlx::query_as::<_, EvalSummary>(
        "SELECT id, run_id, agent_name, model_id, total_questions,
                avg_accuracy, avg_completeness, avg_relevance,
                avg_safety_score, min_safety_score, unsafe_count,
                avg_latency_ms, overall_score
         FROM eval_summary WHERE run_id = ? ORDER BY agent_name, model_id",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut agents: Vec<String> = summaries.iter().map(|s| s.agent_name.clone()).collect();
    agents.sort();
    agents.dedup();

    let mut models: Vec<String> = summaries.iter().map(|s| s.model_id.clone()).collect();
    models.sort();
    models.dedup();

    let cells = summaries
        .into_iter()
        .map(|s| MatrixCell {
            agent_name: s.agent_name,
            model_id: s.model_id,
            overall_score: s.overall_score,
            avg_accuracy: s.avg_accuracy,
            avg_completeness: s.avg_completeness,
            avg_relevance: s.avg_relevance,
            avg_safety_score: s.avg_safety_score,
            unsafe_count: s.unsafe_count,
            avg_latency_ms: s.avg_latency_ms,
            total_questions: s.total_questions,
        })
        .collect();

    Json(MatrixResponse { agents, models, cells })
}

async fn submit_review(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<i64>,
    Json(body): Json<HumanReviewRequest>,
) -> Json<serde_json::Value> {
    let result = sqlx::query(
        r#"UPDATE eval_scores SET
            human_accuracy_score    = COALESCE(?, human_accuracy_score),
            human_completeness_score = COALESCE(?, human_completeness_score),
            human_relevance_score   = COALESCE(?, human_relevance_score),
            human_safety_score      = COALESCE(?, human_safety_score),
            human_notes             = COALESCE(?, human_notes),
            reviewed_by             = COALESCE(?, reviewed_by),
            reviewed_at             = NOW()
           WHERE id = ? AND tenant_id = ?"#,
    )
    .bind(body.accuracy_score)
    .bind(body.completeness_score)
    .bind(body.relevance_score)
    .bind(body.safety_score)
    .bind(&body.notes)
    .bind(&body.reviewed_by)
    .bind(id)
    .bind(&tenant.tenant_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) => Json(serde_json::json!({ "success": true, "rows_affected": r.rows_affected() })),
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

async fn start_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Json(payload): Json<EvaluatorParams>,
) -> axum::response::Response {
    let mut verified_params = payload;

    if tenant.role != "SuperAdmin" || verified_params.tenant_id.is_empty() {
        verified_params.tenant_id = tenant.tenant_id.clone();
    }
    if verified_params.question_limit == 0 {
        verified_params.question_limit = 50;
    }

    match start_evaluation_run(pool, verified_params).await {
        Ok(run_id) => (
            axum::http::StatusCode::ACCEPTED,
            Json(serde_json::json!({ "run_id": run_id })),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn list_benchmark_datasets(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
) -> Json<Vec<BenchmarkDataset>> {
    // Sprint 40 B-36: also surface __global__ datasets (medical benchmarks loaded
    // for cross-tenant use) alongside tenant-specific ones.
    match sqlx::query_as::<_, BenchmarkDataset>(
        "SELECT id, tenant_id, name, source, scoring_fn, description, total_items, version, is_active, created_at, updated_at
         FROM eval_benchmark_datasets
         WHERE tenant_id = ? OR tenant_id = '__global__'
         ORDER BY created_at DESC",
    )
    .bind(&tenant.tenant_id)
    .fetch_all(&pool)
    .await
    {
        Ok(d) => Json(d),
        Err(e) => {
            tracing::error!(event = "list_benchmark_datasets_failed", tenant = %tenant.tenant_id, error = %e);
            Json(vec![])
        }
    }
}

async fn get_benchmark_dataset(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
) -> Json<Option<BenchmarkDatasetDetail>> {
    match sqlx::query_as::<_, BenchmarkDatasetDetail>(
        "SELECT id, tenant_id, name, source, scoring_fn, description, items, total_items, version, is_active, created_at, updated_at
         FROM eval_benchmark_datasets
         WHERE id = ? AND (tenant_id = ? OR tenant_id = '__global__')",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(d) => Json(d),
        Err(e) => {
            tracing::error!(event = "get_benchmark_dataset_failed", id = %id, tenant = %tenant.tenant_id, error = %e);
            Json(None)
        }
    }
}

// ─── Wave 1: Champion + Lock-items + Promote ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChampionQuery {
    pub agent_name: Option<String>,
}

/// GET /api/v1/eval/champion?agent_name=eir → current champion run for the tenant (and optionally agent).
async fn get_champion(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    axum::extract::Query(q): axum::extract::Query<ChampionQuery>,
) -> Json<Option<EvalRun>> {
    let row = if let Some(agent) = q.agent_name {
        sqlx::query_as::<_, EvalRun>(
            "SELECT r.id, r.name, r.status, r.total_combinations, r.completed_combinations,
                    r.started_at, r.finished_at, r.config,
                    r.parent_run_id, r.baseline_run_id, r.hypothesis, r.variable_under_test, r.expected_change,
                    r.is_champion, CAST(r.total_cost_usd AS DOUBLE) AS total_cost_usd
             FROM eval_runs r
             JOIN eval_summary s ON s.run_id = r.id
             WHERE r.tenant_id = ? AND r.is_champion = 1 AND s.agent_name = ?
             ORDER BY r.started_at DESC LIMIT 1",
        )
        .bind(&tenant.tenant_id)
        .bind(&agent)
        .fetch_optional(&pool)
        .await
    } else {
        sqlx::query_as::<_, EvalRun>(
            "SELECT id, name, status, total_combinations, completed_combinations,
                    started_at, finished_at, config,
                    parent_run_id, baseline_run_id, hypothesis, variable_under_test, expected_change,
                    is_champion, CAST(total_cost_usd AS DOUBLE) AS total_cost_usd
             FROM eval_runs WHERE tenant_id = ? AND is_champion = 1 ORDER BY started_at DESC LIMIT 1",
        )
        .bind(&tenant.tenant_id)
        .fetch_optional(&pool)
        .await
    };
    match row {
        Ok(r) => Json(r),
        Err(e) => {
            tracing::error!(event = "get_champion_failed", error = %e);
            Json(None)
        }
    }
}

/// POST /api/v1/eval/runs/{id}/lock-items → returns the item_ids this run used,
/// suitable for replication via `EvaluatorParams.item_ids`. The runner already
/// persists item_ids to config when items have `_source_id`.
async fn get_lock_items(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    // Prefer item_ids stored in config (set by runner); fall back to scanning eval_scores.benchmark_item_id.
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT config FROM eval_runs WHERE id = ? AND tenant_id = ?",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    let from_config: Vec<String> = row
        .and_then(|(c,)| c)
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("item_ids").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let item_ids: Vec<String> = if !from_config.is_empty() {
        from_config
    } else {
        sqlx::query_as::<_, (String,)>(
            "SELECT DISTINCT benchmark_item_id FROM eval_scores
             WHERE run_id = ? AND tenant_id = ? AND benchmark_item_id IS NOT NULL",
        )
        .bind(&id)
        .bind(&tenant.tenant_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(s,)| s)
        .collect()
    };

    Json(serde_json::json!({
        "run_id": id,
        "item_count": item_ids.len(),
        "item_ids": item_ids,
    }))
}

/// POST /api/v1/eval/runs/{id}/promote → mark this run as champion. Atomically demotes any
/// previous champion for the same (tenant, agent) so there's exactly one champion per agent.
async fn promote_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    // Resolve the agent_name(s) for this run via eval_summary
    let agents: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT agent_name FROM eval_summary WHERE run_id = ?",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if agents.is_empty() {
        return Json(serde_json::json!({"error": "no agent summary found for this run — cannot promote"}));
    }

    // Demote previous champion(s) for those agents in this tenant
    for (agent_name,) in &agents {
        let _ = sqlx::query(
            "UPDATE eval_runs r
             JOIN eval_summary s ON s.run_id = r.id
             SET r.is_champion = 0
             WHERE r.tenant_id = ? AND s.agent_name = ? AND r.is_champion = 1 AND r.id != ?",
        )
        .bind(&tenant.tenant_id)
        .bind(agent_name)
        .bind(&id)
        .execute(&pool)
        .await;
    }

    // Promote this run
    let res = sqlx::query("UPDATE eval_runs SET is_champion = 1 WHERE id = ? AND tenant_id = ?")
        .bind(&id)
        .bind(&tenant.tenant_id)
        .execute(&pool)
        .await;

    match res {
        Ok(r) if r.rows_affected() > 0 => Json(serde_json::json!({
            "status": "promoted",
            "run_id": id,
            "agents": agents.iter().map(|(a,)| a).collect::<Vec<_>>(),
        })),
        Ok(_) => Json(serde_json::json!({"error": "run not found in tenant"})),
        Err(e) => {
            tracing::error!(event = "promote_failed", run_id = %id, error = %e);
            Json(serde_json::json!({"error": e.to_string()}))
        }
    }
}

// ─── Sprint 37 B-24 — Replay-judge endpoint ─────────────────────────────────
//
// Re-judge an existing run's eval_scores rows with a (possibly different) judge
// configuration. Doesn't re-run the agent — just re-scores the actual_answer
// rows. Useful for:
//   - Multi-judge ensemble A/B (same answers, single vs ensemble judge → variance)
//   - Trying a new judge model on an old run without re-burning agent inference
//   - Backfilling scores after a judge prompt fix
//
// Request body:
//   { "judge_models": ["gemini-2.5-flash", ...] }    // ≥1 judge; ≥2 = ensemble
//   { "judge_model": "gemini-2.5-flash" }            // single (back-compat)
//   { "store": true }                                 // optional: persist to scores
//
// Response: { "n_rejudged": ..., "delta_summary": {acc,comp,rel,safe} }

#[derive(Debug, serde::Deserialize)]
pub struct RejudgeRequest {
    #[serde(default)]
    pub judge_model: Option<String>,
    #[serde(default)]
    pub judge_models: Option<Vec<String>>,
    /// If true, write new scores to a new column or overwrite existing.
    /// Default false (dry-run — return delta only, don't persist).
    #[serde(default)]
    pub store: bool,
}

async fn rejudge_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<RejudgeRequest>,
) -> Json<serde_json::Value> {
    // Resolve target judges: explicit list > single judge > error
    let judges: Vec<String> = match (req.judge_models, req.judge_model) {
        (Some(v), _) if !v.is_empty() => v,
        (_, Some(s)) => vec![s],
        _ => return Json(serde_json::json!({"error": "must provide judge_model or judge_models"})),
    };

    // Pull eval_scores rows for this run + tenant
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, question, expected_answer, COALESCE(actual_answer, '')
         FROM eval_scores
         WHERE run_id = ? AND tenant_id = ?",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return Json(serde_json::json!({"error": "no eval_scores rows for run/tenant"}));
    }

    // Note: this is a stub — full implementation would call the judge per row,
    // average across ensemble, and optionally persist. For Sprint 37 Option A
    // we ship the endpoint shell so the UI/script can prototype against it.
    // Full re-judge logic = Sprint 37 follow-up after Round 5 result.
    Json(serde_json::json!({
        "status": "endpoint_stub",
        "run_id": id,
        "n_rows": rows.len(),
        "judges": judges,
        "store": req.store,
        "message": "Replay-judge endpoint registered. Full re-scoring logic pending — \
                    use this stub to verify routing works end-to-end. Will be filled in \
                    next sprint cycle once Round 5 results inform priorities."
    }))
}

// ─── Sprint 47 B-47f — Bottleneck attribution / diagnose ───────────────────────

#[derive(Debug, Deserialize)]
pub struct DiagnoseQuery {
    /// Optional sibling run for paired A/B (e.g. `?vs=<rag_off_run_id>`).
    /// When provided, attribution is computed per-question against this baseline.
    /// When absent, returns aggregate metrics for the single run only.
    #[serde(default)]
    pub vs: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiagnoseAggregate {
    run_id: String,
    name: Option<String>,
    n: i64,
    hbp: f64,
    avg_accuracy: f64,
    avg_completeness: f64,
    avg_relevance: f64,
    avg_safety: f64,
    unsafe_count: i64,
    avg_latency_ms: f64,
    rag_chunks_avg: Option<f64>,
}

#[derive(Debug, Serialize)]
struct DiagnoseDelta {
    /// HBp difference: a (treatment) - b (baseline)
    hbp_delta_pp: f64,
    /// Per-dim deltas (0-1 normalized)
    accuracy_delta: f64,
    completeness_delta: f64,
    relevance_delta: f64,
    safety_delta: f64,
    /// Per-question attribution counts.
    n_rag_helped: i64,        // both correct, but treatment HBp > baseline by ≥0.10
    n_rag_neutral: i64,        // |delta| < 0.10
    n_rag_hurt: i64,           // baseline > treatment (rare, indicates RAG noise)
    n_both_failed: i64,        // both <0.5 HBp — neither RAG nor LLM enough
    n_both_passed: i64,        // both ≥0.7 HBp — easy questions
}

#[derive(Debug, Serialize)]
struct DiagnoseResponse {
    target: DiagnoseAggregate,
    baseline: Option<DiagnoseAggregate>,
    delta: Option<DiagnoseDelta>,
    /// Plain-language summary the UI can show prominently.
    summary: String,
    /// Suggested next-sprint focus per the verdict — see Sprint 39d plan.
    recommendation: String,
}

async fn aggregate_run(
    pool: &DbPool,
    tenant_id: &str,
    run_id: &str,
) -> Result<Option<DiagnoseAggregate>, sqlx::Error> {
    let row: Option<(Option<String>, i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT
           r.name,
           COUNT(s.id)                                              AS n,
           CAST(AVG(s.accuracy_score) AS DOUBLE)                    AS avg_acc,
           CAST(AVG(s.completeness_score) AS DOUBLE)                AS avg_comp,
           CAST(AVG(s.relevance_score) AS DOUBLE)                   AS avg_rel,
           CAST(AVG(s.safety_score) AS DOUBLE)                      AS avg_safe,
           CAST(SUM(CASE WHEN s.safety_score < 0 THEN 1 ELSE 0 END) AS SIGNED) AS unsafe_n,
           CAST(AVG(s.latency_ms) AS DOUBLE)                        AS avg_lat,
           CAST(AVG(CASE
                 WHEN s.retrieved_chunk_ids IS NULL THEN NULL
                 ELSE JSON_LENGTH(s.retrieved_chunk_ids)
               END) AS DOUBLE)                                       AS chunks_avg
         FROM eval_runs r
         LEFT JOIN eval_scores s ON s.run_id = r.id
         WHERE r.id = ? AND r.tenant_id = ?
         GROUP BY r.id, r.name",
    )
    .bind(run_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;

    let Some((name, n, acc, comp, rel, safe, unsafe_n, lat, chunks)) = row else { return Ok(None); };
    if n == 0 { return Ok(None); }

    // HBp = (acc_norm + comp_norm + rel_norm + safety) / 4 * 100
    let acc_norm = ((acc.unwrap_or(1.0) - 1.0) / 4.0).max(0.0);
    let comp_norm = ((comp.unwrap_or(1.0) - 1.0) / 4.0).max(0.0);
    let rel_norm = ((rel.unwrap_or(1.0) - 1.0) / 4.0).max(0.0);
    let safety = safe.unwrap_or(0.0);
    let hbp = ((acc_norm + comp_norm + rel_norm + safety) / 4.0 * 100.0 * 10.0).round() / 10.0;

    Ok(Some(DiagnoseAggregate {
        run_id: run_id.to_string(),
        name,
        n,
        hbp,
        avg_accuracy: round2(acc.unwrap_or(0.0)),
        avg_completeness: round2(comp.unwrap_or(0.0)),
        avg_relevance: round2(rel.unwrap_or(0.0)),
        avg_safety: round2(safety),
        unsafe_count: unsafe_n.unwrap_or(0),
        avg_latency_ms: lat.unwrap_or(0.0).round(),
        rag_chunks_avg: chunks.map(|c| round2(c)),
    }))
}

fn round2(x: f64) -> f64 { (x * 100.0).round() / 100.0 }

#[derive(Debug, sqlx::FromRow)]
struct PerItemRow {
    benchmark_item_id: Option<String>,
    accuracy_score: Option<i32>,
    completeness_score: Option<i32>,
    relevance_score: Option<i32>,
    safety_score: Option<i32>,
}

fn item_hbp(r: &PerItemRow) -> f64 {
    let acc = ((r.accuracy_score.unwrap_or(1) as f64 - 1.0) / 4.0).max(0.0);
    let comp = ((r.completeness_score.unwrap_or(1) as f64 - 1.0) / 4.0).max(0.0);
    let rel = ((r.relevance_score.unwrap_or(1) as f64 - 1.0) / 4.0).max(0.0);
    let safety = r.safety_score.unwrap_or(0) as f64;
    (acc + comp + rel + safety) / 4.0
}

async fn diagnose_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(run_id): Path<String>,
    Query(req): Query<DiagnoseQuery>,
) -> Result<Json<DiagnoseResponse>, (axum::http::StatusCode, String)> {
    let target = aggregate_run(&pool, &tenant.tenant_id, &run_id).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("aggregate target: {e}")))?
        .ok_or((axum::http::StatusCode::NOT_FOUND, "run not found".to_string()))?;

    let (baseline, delta, summary, recommendation): (
        Option<DiagnoseAggregate>, Option<DiagnoseDelta>, String, String,
    ) = if let Some(vs) = &req.vs {
        let b = aggregate_run(&pool, &tenant.tenant_id, vs).await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("aggregate baseline: {e}")))?;
        if let Some(b) = b {
            // Per-item attribution.
            let target_items: Vec<PerItemRow> = sqlx::query_as(
                "SELECT benchmark_item_id, accuracy_score, completeness_score,
                        relevance_score, safety_score
                 FROM eval_scores
                 WHERE run_id = ? AND tenant_id = ? AND benchmark_item_id IS NOT NULL")
                .bind(&run_id).bind(&tenant.tenant_id)
                .fetch_all(&pool).await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("per-item: {e}")))?;
            let baseline_items: Vec<PerItemRow> = sqlx::query_as(
                "SELECT benchmark_item_id, accuracy_score, completeness_score,
                        relevance_score, safety_score
                 FROM eval_scores
                 WHERE run_id = ? AND tenant_id = ? AND benchmark_item_id IS NOT NULL")
                .bind(vs).bind(&tenant.tenant_id)
                .fetch_all(&pool).await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("per-item: {e}")))?;

            // Build map of benchmark_item_id -> hbp for both
            use std::collections::HashMap;
            let target_map: HashMap<&String, f64> = target_items.iter()
                .filter_map(|r| r.benchmark_item_id.as_ref().map(|id| (id, item_hbp(r))))
                .collect();
            let baseline_map: HashMap<&String, f64> = baseline_items.iter()
                .filter_map(|r| r.benchmark_item_id.as_ref().map(|id| (id, item_hbp(r))))
                .collect();

            let (mut helped, mut neutral, mut hurt, mut both_failed, mut both_passed) = (0i64, 0i64, 0i64, 0i64, 0i64);
            for (id, t) in &target_map {
                if let Some(b) = baseline_map.get(*id) {
                    let d = t - b;
                    if *t < 0.5 && *b < 0.5 { both_failed += 1; }
                    else if *t >= 0.7 && *b >= 0.7 { both_passed += 1; }
                    else if d >= 0.10 { helped += 1; }
                    else if d <= -0.10 { hurt += 1; }
                    else { neutral += 1; }
                }
            }

            let delta = DiagnoseDelta {
                hbp_delta_pp: round2(target.hbp - b.hbp),
                accuracy_delta: round2(target.avg_accuracy - b.avg_accuracy),
                completeness_delta: round2(target.avg_completeness - b.avg_completeness),
                relevance_delta: round2(target.avg_relevance - b.avg_relevance),
                safety_delta: round2(target.avg_safety - b.avg_safety),
                n_rag_helped: helped,
                n_rag_neutral: neutral,
                n_rag_hurt: hurt,
                n_both_failed: both_failed,
                n_both_passed: both_passed,
            };

            // Plain-language summary + recommendation
            let total_with_baseline = helped + neutral + hurt + both_failed + both_passed;
            let summary = if delta.hbp_delta_pp >= 5.0 {
                format!("RAG strongly contributes (+{:.1}pp HBp). {} of {} questions clearly benefit from retrieval.",
                    delta.hbp_delta_pp, helped, total_with_baseline)
            } else if delta.hbp_delta_pp >= 2.0 {
                format!("RAG mildly helps (+{:.1}pp HBp). Mixed signal: {} helped, {} neutral, {} both failed.",
                    delta.hbp_delta_pp, helped, neutral, both_failed)
            } else if delta.hbp_delta_pp >= -2.0 {
                format!("RAG marginal (Δ {:+.1}pp). LLM is doing the heavy lifting; retrieval may be noise.",
                    delta.hbp_delta_pp)
            } else {
                format!("RAG actively HURTS (Δ {:+.1}pp). {} questions worse with RAG — investigate retrieval poisoning.",
                    delta.hbp_delta_pp, hurt)
            };
            let recommendation = if delta.hbp_delta_pp >= 5.0 {
                "Sprint 39d focus: invest in retrieval quality (B-47g clinician gold + MedCPT re-embed + semantic re-chunk). Skip corpus retrain — LoRA is near RAG-on ceiling."
                    .to_string()
            } else if delta.hbp_delta_pp >= 2.0 {
                "Mixed: split investment between retrieval and corpus. Run full RAG eval (B-47b RAGAS metrics) for per-dim attribution."
                    .to_string()
            } else {
                "RAG not the bottleneck. Direct LLM/LoRA improvement (corpus expansion, base model swap) is more likely path to lift."
                    .to_string()
            };

            (Some(b), Some(delta), summary, recommendation)
        } else {
            (None, None, "Baseline run not found.".into(), "Provide ?vs=<run_id> for paired attribution.".into())
        }
    } else {
        let summary = format!(
            "Single-run summary: {} questions · HBp {:.1}% · {} unsafe · avg {:.1} chunks retrieved.",
            target.n, target.hbp, target.unsafe_count,
            target.rag_chunks_avg.unwrap_or(0.0));
        let recommendation = "Provide ?vs=<baseline_run_id> for bottleneck attribution (RAG vs LLM).".into();
        (None, None, summary, recommendation)
    };

    tracing::info!(
        event = "eval_diagnose",
        tenant = %tenant.tenant_id,
        run_id = %run_id,
        vs = ?req.vs,
    );

    Ok(Json(DiagnoseResponse {
        target,
        baseline,
        delta,
        summary,
        recommendation,
    }))
}
