//! Underwriter pipeline-run registry API.
//!
//! POST /api/v1/underwriter/pipeline-runs — upsert one run record.
//! GET  /api/v1/underwriter/pipeline-runs/{batch_id} — fetch a run by batch_id.
//!
//! Iris (the underwriting orchestrator) is the writer: it registers each run
//! keyed by `batch_id` so the run can be reconstructed across the monitoring
//! sinks (Vardr/Tyr/Laminar). This is the authoritative registry write — the
//! HTTP eval endpoint (`/api/v1/evaluations/run`) is a slow sync LLM judge and
//! is unsuitable for this lightweight bookkeeping.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;

#[derive(Debug, Deserialize)]
pub struct PipelineRunUpsert {
    pub batch_id: String,
    pub tenant_id: Option<String>,
    pub dataset_id: String,
    pub record_id: String,
    pub pipeline_id: String,
    pub model_step3: Option<String>,
    pub model_step4: Option<String>,
    /// Unix epoch seconds (Iris uses `SystemTime`); stored via `FROM_UNIXTIME`.
    pub started_at: f64,
    pub completed_at: Option<f64>,
    pub total_elapsed_s: Option<f64>,
    pub status: String,
    pub risk_band: Option<String>,
    pub risk_score: Option<i32>,
    pub hitl_required: Option<bool>,
    pub diagnoses_count: Option<i32>,
    #[serde(default)]
    pub summary_json: Value,
    #[serde(default)]
    pub perf_json: Value,
    #[serde(default)]
    pub telemetry_json: Value,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PipelineRunRow {
    pub batch_id: String,
    pub tenant_id: String,
    pub dataset_id: String,
    pub record_id: String,
    pub pipeline_id: String,
    pub model_step3: Option<String>,
    pub model_step4: Option<String>,
    pub status: String,
    pub risk_band: Option<String>,
    pub risk_score: Option<i32>,
    pub hitl_required: Option<bool>,
    pub diagnoses_count: Option<i32>,
}

pub fn underwriter_routes() -> Router<DbPool> {
    Router::new()
        .route("/pipeline-runs", post(upsert_pipeline_run))
        .route("/pipeline-runs/{batch_id}", get(get_pipeline_run))
}

/// POST /api/v1/underwriter/pipeline-runs — upsert by batch_id (REPLACE INTO).
async fn upsert_pipeline_run(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(req): Json<PipelineRunUpsert>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = req
        .tenant_id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    // Normalize status to the table's ENUM domain.
    let status = match req.status.as_str() {
        "completed" | "failed" | "running" => req.status.clone(),
        _ => "running".to_string(),
    };

    let summary = serde_json::to_string(&req.summary_json).unwrap_or_else(|_| "{}".into());
    let perf = serde_json::to_string(&req.perf_json).unwrap_or_else(|_| "{}".into());
    let telemetry = serde_json::to_string(&req.telemetry_json).unwrap_or_else(|_| "{}".into());

    sqlx::query(
        "REPLACE INTO underwriter_pipeline_runs (
            batch_id, tenant_id, dataset_id, record_id, pipeline_id,
            model_step3, model_step4,
            started_at, completed_at, total_elapsed_s, status,
            risk_band, risk_score, hitl_required, diagnoses_count,
            summary_json, perf_json, telemetry_json
        ) VALUES (
            ?, ?, ?, ?, ?,
            ?, ?,
            FROM_UNIXTIME(?), FROM_UNIXTIME(?), ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?
        )",
    )
    .bind(&req.batch_id)
    .bind(&tenant_id)
    .bind(&req.dataset_id)
    .bind(&req.record_id)
    .bind(&req.pipeline_id)
    .bind(&req.model_step3)
    .bind(&req.model_step4)
    .bind(req.started_at)
    .bind(req.completed_at)
    .bind(req.total_elapsed_s)
    .bind(&status)
    .bind(&req.risk_band)
    .bind(req.risk_score)
    .bind(req.hitl_required)
    .bind(req.diagnoses_count)
    .bind(&summary)
    .bind(&perf)
    .bind(&telemetry)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    info!(
        "📒 underwriter run registered: batch_id={} status={} tenant={}",
        req.batch_id, status, tenant_id
    );

    Ok((
        StatusCode::OK,
        Json(json!({"batch_id": req.batch_id, "status": status})),
    ))
}

/// GET /api/v1/underwriter/pipeline-runs/{batch_id}
async fn get_pipeline_run(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(batch_id): Path<String>,
) -> Result<Json<PipelineRunRow>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let row = sqlx::query_as::<_, PipelineRunRow>(
        "SELECT batch_id, tenant_id, dataset_id, record_id, pipeline_id,
                model_step3, model_step4, status, risk_band, risk_score,
                hitl_required, diagnoses_count
         FROM underwriter_pipeline_runs
         WHERE batch_id = ? AND tenant_id = ?",
    )
    .bind(&batch_id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    match row {
        Some(r) => Ok(Json(r)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "pipeline run not found"})),
        )),
    }
}
