use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

use mimir_core_ai::services::db::DbPool;

// ─── Request / Response types ──────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct PipelineRun {
    pub id: String,
    pub status: String,
    pub provider: String,
    pub model: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub test_run: i8,
}

#[derive(Debug, Serialize, FromRow)]
pub struct RunDetails {
    pub id: String,
    pub status: String,
    pub provider: String,
    pub model: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub test_run: i8,
}

#[derive(Debug, Deserialize)]
pub struct TriggerRunRequest {
    pub provider: String,
    pub model: String,
    pub dry_run: Option<bool>,
}

// ─── Router ────────────────────────────────────────────────────────────

pub fn pipeline_routes() -> Router<DbPool> {
    Router::new()
        .route("/runs", get(list_runs))
        .route("/runs/{id}", get(get_run_details))
        .route("/run", post(trigger_run))
}

// ─── Handlers ──────────────────────────────────────────────────────────

/// GET /api/v1/pipeline/runs — List all pipeline runs
async fn list_runs(State(pool): State<DbPool>) -> Json<Vec<PipelineRun>> {
    match sqlx::query_as::<_, PipelineRun>(
        "SELECT id, status, provider, model, started_at, finished_at, test_run FROM pipeline_runs ORDER BY started_at DESC"
    )
    .fetch_all(&pool)
    .await {
        Ok(runs) => Json(runs),
        Err(e) => {
            tracing::error!("Failed to fetch pipeline runs: {}", e);
            Json(vec![])
        }
    }
}

/// GET /api/v1/pipeline/runs/:id — Get specific details of a pipeline run
async fn get_run_details(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Json<Option<RunDetails>> {
    match sqlx::query_as::<_, RunDetails>(
        "SELECT id, status, provider, model, started_at, finished_at, test_run FROM pipeline_runs WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await {
        Ok(run) => Json(run),
        Err(e) => {
            tracing::error!("Failed to fetch run details for {}: {}", id, e);
            Json(None)
        }
    }
}

/// POST /api/v1/pipeline/run — Trigger a new pipeline run
async fn trigger_run(
    State(_pool): State<DbPool>,
    Json(_req): Json<TriggerRunRequest>,
) -> Json<serde_json::Value> {
    // Basic placeholder (actual running logic usually delegates to background task)
    Json(serde_json::json!({
        "success": true,
        "message": "Pipeline run triggered (Placeholder)"
    }))
}
