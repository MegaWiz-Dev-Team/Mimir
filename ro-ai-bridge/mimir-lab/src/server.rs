//! analytics-api HTTP server — the backend behind Hermodr's analytics MCP tools.
//!
//! Wraps [`crate::api`] over a **per-request** DuckDB engine (sync → run inside
//! `spawn_blocking`) that attaches the tenant's Parquet datasets from a data
//! directory as read-only views, plus the sqlx [`Registry`] for dataset
//! list/profile. Every query is Tyr-audited via the engine's sink.
//!
//! Routes (paths match `Hermodr services/analytics.rs`):
//!   POST /api/v1/analytics/query            → run_sql
//!   POST /api/v1/analytics/plot             → plot (ECharts option)
//!   POST /api/v1/analytics/datasets/list    → dataset_list
//!   POST /api/v1/analytics/datasets/profile → dataset_profile
//!   GET  /healthz

use crate::api::{self, PlotReq, RunSqlReq};
use crate::audit::AuditSink;
use crate::engine::Engine;
use crate::error::{valid_ident, LabError};
use crate::registry::Registry;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    /// Directory of `<table>.parquet` files attached as views for queries.
    pub data_dir: Arc<String>,
    pub audit: Arc<dyn AuditSink>,
    /// `None` → dataset list/profile return 503 (query/plot still work).
    pub registry: Option<Arc<Registry>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/v1/analytics/query", post(query))
        .route("/api/v1/analytics/plot", post(plot))
        .route("/api/v1/analytics/datasets/list", post(datasets_list))
        .route("/api/v1/analytics/datasets/profile", post(datasets_profile))
        // spatial (mimir-geo) — pure handlers, state-agnostic
        .route("/api/v1/analytics/geo/distance", post(crate::geo_api::distance))
        .route("/api/v1/analytics/geo/buffer", post(crate::geo_api::buffer))
        .route("/api/v1/analytics/geo/join", post(crate::geo_api::join))
        .route("/api/v1/analytics/geo/choropleth", post(crate::geo_api::choropleth))
        .route("/api/v1/analytics/geo/h3", post(crate::geo_api::h3_aggregate))
        .route("/api/v1/analytics/geo/ingest", post(crate::geo_api::ingest))
        .route("/api/v1/analytics/stats/moran", post(crate::geo_api::moran))
        .route("/api/v1/analytics/stats/nn", post(crate::geo_api::nn))
        // research RAG (proxies mimir-api knowledge/search) — P5 lit_search
        .route("/api/v1/analytics/lit_search", post(crate::lit_api::lit_search))
        .with_state(state)
}

/// LabError → HTTP response.
struct AppErr(LabError);
impl IntoResponse for AppErr {
    fn into_response(self) -> Response {
        let code = match &self.0 {
            LabError::NotReadOnly(_) | LabError::Api(_) | LabError::BadIdent(_) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (code, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}
impl From<LabError> for AppErr {
    fn from(e: LabError) -> Self {
        AppErr(e)
    }
}

/// Build an engine with the tenant's parquet datasets attached as views.
fn engine_for(state: &AppState, tenant: &str) -> Result<Engine, LabError> {
    let e = Engine::in_memory()?
        .with_audit(state.audit.clone())
        .with_context(Some(tenant.to_string()), None);
    attach_data_dir(&e, &state.data_dir)?;
    Ok(e)
}

/// Attach every dataset file in `dir` as a read-only view named by its stem.
/// Supports `.parquet`, `.csv`/`.tsv`, and `.json`/`.ndjson`/`.jsonl`.
pub fn attach_data_dir(engine: &Engine, dir: &str) -> Result<Vec<String>, LabError> {
    let mut names = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(names), // no data dir yet → no tables
    };
    for entry in rd.flatten() {
        let p = entry.path();
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .map(|s| s.to_ascii_lowercase());
        let reader = match ext.as_deref() {
            Some("parquet") | Some("pq") => "read_parquet",
            Some("csv") | Some("tsv") => "read_csv_auto",
            Some("json") | Some("ndjson") | Some("jsonl") => "read_json_auto",
            _ => continue,
        };
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if valid_ident(stem).is_err() {
            continue; // skip files whose stem isn't a safe identifier
        }
        let path = p.to_string_lossy().replace('\'', "''");
        engine.execute(&format!(
            "CREATE OR REPLACE VIEW {stem} AS SELECT * FROM {reader}('{path}')"
        ))?;
        names.push(stem.to_string());
    }
    Ok(names)
}

async fn query(
    State(st): State<AppState>,
    Json(req): Json<RunSqlReq>,
) -> Result<Json<Value>, AppErr> {
    let v = tokio::task::spawn_blocking(move || -> Result<Value, LabError> {
        let e = engine_for(&st, &req.tenant_id)?;
        api::run_sql(&e, &req)
    })
    .await
    .map_err(|e| AppErr(LabError::Api(format!("task join: {e}"))))??;
    Ok(Json(v))
}

async fn plot(State(st): State<AppState>, Json(req): Json<PlotReq>) -> Result<Json<Value>, AppErr> {
    let v = tokio::task::spawn_blocking(move || -> Result<Value, LabError> {
        let e = engine_for(&st, &req.tenant_id)?;
        api::plot(&e, &req)
    })
    .await
    .map_err(|e| AppErr(LabError::Api(format!("task join: {e}"))))??;
    Ok(Json(v))
}

#[derive(Deserialize)]
struct ListReq {
    tenant_id: String,
}
async fn datasets_list(
    State(st): State<AppState>,
    Json(req): Json<ListReq>,
) -> Result<Json<Value>, AppErr> {
    let reg = st
        .registry
        .as_ref()
        .ok_or_else(|| AppErr(LabError::Api("registry not configured".into())))?;
    let ds = api::dataset_list(reg, &req.tenant_id).await?;
    Ok(Json(json!({ "datasets": ds })))
}

#[derive(Deserialize)]
struct ProfileReq {
    #[allow(dead_code)]
    tenant_id: String,
    dataset_id: String,
}
async fn datasets_profile(
    State(st): State<AppState>,
    Json(req): Json<ProfileReq>,
) -> Result<Json<Value>, AppErr> {
    let reg = st
        .registry
        .as_ref()
        .ok_or_else(|| AppErr(LabError::Api("registry not configured".into())))?;
    let ds = api::dataset_profile(reg, &req.dataset_id).await?;
    Ok(Json(json!({ "dataset": ds })))
}
