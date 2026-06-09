//! API-facing handlers behind the Hermodr analytics MCP tools
//! (`dataset_list` / `dataset_profile` / `run_sql` / `plot`).
//!
//! The query/plot handlers are **synchronous** (DuckDB is sync) and pure over a
//! borrowed [`Engine`], so the analytics-api server runs them inside
//! `tokio::task::spawn_blocking`. The dataset handlers are thin async wrappers
//! over [`Registry`]. JSON shapes here are the MCP tool contract.

use crate::engine::Engine;
use crate::error::{LabError, Result};
use crate::registry::{Dataset, Registry};
use crate::schema::ColumnSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const DEFAULT_ROW_LIMIT: usize = 1000;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

// ── run_sql ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RunSqlReq {
    pub tenant_id: String,
    pub sql: String,
    #[serde(default)]
    pub row_limit: Option<usize>,
}

/// Execute a read-only query (capped + timed + audited by the engine) and
/// return `{ columns, rows, truncated, row_count }`.
pub fn run_sql(engine: &Engine, req: &RunSqlReq) -> Result<Value> {
    let cap = req.row_limit.unwrap_or(DEFAULT_ROW_LIMIT);
    let r = engine.query_readonly_timeout(&req.sql, cap, DEFAULT_TIMEOUT)?;
    Ok(json!({
        "columns": r.columns.iter().map(col_json).collect::<Vec<_>>(),
        "rows": r.rows,
        "truncated": r.truncated,
        "row_count": r.rows.len(),
    }))
}

fn col_json(c: &ColumnSchema) -> Value {
    json!({ "name": c.name, "type": c.sql_type, "nullable": c.nullable })
}

// ── plot ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PlotReq {
    pub tenant_id: String,
    pub sql: String,
    pub chart_type: String,
    pub x: String,
    pub y: String,
}

/// Run a query and return an Apache ECharts `option` object (the portal renders
/// it). Not an image — a spec, per ADR-024.
pub fn plot(engine: &Engine, req: &PlotReq) -> Result<Value> {
    let r = engine.query_readonly_timeout(&req.sql, DEFAULT_ROW_LIMIT, DEFAULT_TIMEOUT)?;
    let xi = col_index(&r.columns, &req.x)?;
    let yi = col_index(&r.columns, &req.y)?;

    let xs: Vec<String> = r
        .rows
        .iter()
        .map(|row| row.get(xi).cloned().flatten().unwrap_or_default())
        .collect();
    let ys: Vec<f64> = r
        .rows
        .iter()
        .map(|row| {
            row.get(yi)
                .and_then(|v| v.as_deref())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0)
        })
        .collect();

    let option = match req.chart_type.as_str() {
        "pie" => json!({
            "tooltip": { "trigger": "item" },
            "series": [{
                "type": "pie",
                "data": xs.iter().zip(ys.iter())
                    .map(|(n, v)| json!({ "name": n, "value": v }))
                    .collect::<Vec<_>>()
            }]
        }),
        kind @ ("bar" | "line" | "scatter") => json!({
            "tooltip": { "trigger": "axis" },
            "xAxis": { "type": "category", "name": req.x, "data": xs },
            "yAxis": { "type": "value", "name": req.y },
            "series": [{ "type": kind, "data": ys }]
        }),
        other => return Err(LabError::Api(format!("unsupported chart_type '{other}'"))),
    };
    Ok(json!({ "echarts": option }))
}

fn col_index(cols: &[ColumnSchema], name: &str) -> Result<usize> {
    cols.iter()
        .position(|c| c.name == name)
        .ok_or_else(|| LabError::Api(format!("column '{name}' not in result set")))
}

// ── dataset_list / dataset_profile ───────────────────────────────────────────

/// `dataset_list` — datasets for a tenant.
pub async fn dataset_list(reg: &Registry, tenant_id: &str) -> Result<Vec<Dataset>> {
    reg.list_datasets(tenant_id).await
}

/// `dataset_profile` — one dataset by id (None if absent).
pub async fn dataset_profile(reg: &Registry, dataset_id: &str) -> Result<Option<Dataset>> {
    reg.get_dataset(dataset_id).await
}
