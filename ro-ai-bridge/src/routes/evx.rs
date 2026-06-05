//! Unified evaluation scoreboard API (evx_* layer).
//!
//! - GET /api/v1/eval/scoreboard?family=ner   — one cross-family scoreboard
//!
//! Backed by the `evx_scoreboard` view: one row per (target, dataset) run with
//! its primary metric. Works identically for QA / RAG / OCR / OCR-layout / NER
//! / coding — the UI registry decides how to label and colour each family.
//!
//! Tenant: rows are scoped to the caller's tenant (TenantContext), plus the
//! cross-cutting `asgard_platform` engineering rows (tenant_id IS NULL/that).

use axum::{
    extract::{Extension, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;

#[derive(Debug, Serialize, FromRow)]
pub struct ScoreboardRow {
    pub family: String,
    pub run_id: String,
    pub experiment_id: Option<String>,
    pub tenant_id: Option<String>,
    pub target_kind: String,
    pub target_name: String,
    pub model_id: Option<String>,
    pub runtime: Option<String>,
    pub dataset_id: Option<String>,
    pub n_items: i32,
    pub primary_metric: Option<String>,
    pub primary_value: Option<f64>,
    pub unit: Option<String>,
    pub higher_is_better: Option<i8>,
    pub ci_low: Option<f64>,
    pub ci_high: Option<f64>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ScoreboardQuery {
    /// Optional family filter (qa | rag | ocr | ocr_layout | ner | coding ...).
    pub family: Option<String>,
}

async fn get_scoreboard(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Query(q): Query<ScoreboardQuery>,
) -> Json<Vec<ScoreboardRow>> {
    let rows = sqlx::query_as::<_, ScoreboardRow>(
        "SELECT family, run_id, experiment_id, tenant_id, target_kind, target_name,
                model_id, runtime, dataset_id, n_items, primary_metric,
                primary_value, unit, higher_is_better, ci_low, ci_high, finished_at
         FROM evx_scoreboard
         WHERE (tenant_id = ? OR tenant_id IS NULL)
           AND (? IS NULL OR family = ?)
         ORDER BY family ASC, finished_at DESC",
    )
    .bind(&tenant.tenant_id)
    .bind(&q.family)
    .bind(&q.family)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "evx_scoreboard_failed", tenant = %tenant.tenant_id, error = %e);
        Vec::new()
    });

    Json(rows)
}

pub fn evx_routes() -> Router<DbPool> {
    Router::new().route("/api/v1/eval/scoreboard", get(get_scoreboard))
}
