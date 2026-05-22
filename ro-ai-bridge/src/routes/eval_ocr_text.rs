//! OCR *text-level* evaluation read API — `ocr_eval_*` schema (Sprint 51).
//!
//! Endpoints (read-only — runs are ingested out-of-band by the engine bench):
//!   GET /api/v1/eval/ocr/text/runs        — List runs (paginated, filterable).
//!   GET /api/v1/eval/ocr/text/runs/:id    — Run detail: per-engine summary +
//!                                            per-(case,engine) CER/WER rows.
//!
//! Sibling of `eval_ocr_layout` (region geometry). This axis measures text
//! recognition quality — CER/WER per engine (apple-vision, typhoon-local, …).
//! The two are complementary; see the runbook.
//!
//! Tenant: scoped by `X-Tenant-Id`, default `asgard_platform` (same rule as the
//! layout route). A run under one tenant is invisible (404) to another.
//!
//! PII: real datasets (medical certs) carry PHI in `ground_truth` /
//! `extracted_text`. This read API **never returns those raw text columns** —
//! only metrics (CER/WER, status, timing, char counts). Inspecting raw text is
//! a separate, deliberately out-of-scope action.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use mimir_core_ai::services::db::DbPool;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use tracing::warn;

const DEFAULT_TENANT: &str = "asgard_platform";

/// Resolve tenant from `X-Tenant-Id`, defaulting to the platform bucket.
/// Mirrors `eval_ocr_layout::resolve_tenant`.
fn resolve_tenant(headers: &HeaderMap) -> String {
    headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_TENANT)
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub dataset_name: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}
fn default_limit() -> i64 {
    20
}

// ─── Handlers ──────────────────────────────────────────────────────────────

/// GET /runs — list text-eval runs for the active tenant.
async fn list_runs(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant = resolve_tenant(&headers);
    let limit = q.limit.clamp(1, 200);

    let mut sql = String::from(
        "SELECT r.id, r.name, r.prompt_label, r.engines, r.started_at, r.finished_at,
                d.name AS dataset_name, d.source AS dataset_source,
                (SELECT COUNT(*) FROM ocr_eval_results res WHERE res.run_id = r.id) AS n_results
         FROM ocr_eval_runs r
         JOIN ocr_eval_datasets d ON d.id = r.dataset_id
         WHERE r.tenant_id = ?",
    );
    if q.dataset_name.is_some() {
        sql.push_str(" AND d.name = ?");
    }
    sql.push_str(" ORDER BY r.started_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql).bind(&tenant);
    if let Some(ref v) = q.dataset_name {
        query = query.bind(v);
    }
    query = query.bind(limit).bind(q.offset);

    let rows = match query.fetch_all(&pool).await {
        Ok(r) => r,
        Err(e) => return err500("fetch ocr_eval_runs list", e),
    };

    let runs: Vec<Value> = rows
        .iter()
        .map(|r| {
            let engines_str: Option<String> = r.try_get("engines").ok();
            let engines: Value = engines_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            json!({
                "id": r.get::<String, _>("id"),
                "name": r.try_get::<Option<String>, _>("name").ok().flatten(),
                "prompt_label": r.try_get::<Option<String>, _>("prompt_label").ok().flatten(),
                "engines": engines,
                "dataset_name": r.get::<String, _>("dataset_name"),
                "dataset_source": r.get::<String, _>("dataset_source"),
                "n_results": r.try_get::<i64, _>("n_results").ok(),
                "started_at": r.try_get::<DateTime<Utc>, _>("started_at").ok(),
                "finished_at": r.try_get::<Option<DateTime<Utc>>, _>("finished_at").ok().flatten(),
            })
        })
        .collect();

    Ok(Json(json!({
        "runs": runs,
        "tenant": tenant,
        "limit": limit,
        "offset": q.offset
    })))
}

/// GET /runs/:id — run detail with per-engine aggregate + per-case metrics.
async fn get_run(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant = resolve_tenant(&headers);

    let run = match sqlx::query(
        "SELECT r.id, r.name, r.prompt_label, r.engines, r.metadata,
                r.started_at, r.finished_at, r.notes,
                d.name AS dataset_name, d.source AS dataset_source, d.description AS dataset_desc
         FROM ocr_eval_runs r
         JOIN ocr_eval_datasets d ON d.id = r.dataset_id
         WHERE r.id = ? AND r.tenant_id = ?",
    )
    .bind(&id)
    .bind(&tenant)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return err(StatusCode::NOT_FOUND, &format!("run {id} not found")),
        Err(e) => return err500("fetch run", e),
    };

    // Per-engine aggregate. CAST to DOUBLE so DECIMAL(8,4) maps cleanly to f64.
    let engine_rows = match sqlx::query(
        "SELECT engine,
                COUNT(*)                                   AS n,
                SUM(status = 'ok')                         AS n_ok,
                CAST(AVG(cer) AS DOUBLE)                   AS mean_cer,
                CAST(AVG(wer) AS DOUBLE)                   AS mean_wer,
                CAST(MIN(cer) AS DOUBLE)                   AS min_cer,
                CAST(MAX(cer) AS DOUBLE)                   AS max_cer,
                CAST(AVG(wall_ms) AS DOUBLE)               AS mean_wall_ms
         FROM ocr_eval_results
         WHERE run_id = ?
         GROUP BY engine
         ORDER BY mean_cer ASC",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return err500("fetch per-engine aggregate", e),
    };

    let engines: Vec<Value> = engine_rows
        .iter()
        .map(|r| {
            json!({
                "engine": r.get::<String, _>("engine"),
                "n": r.try_get::<i64, _>("n").ok(),
                "n_ok": r.try_get::<i64, _>("n_ok").ok(),
                "mean_cer": r.try_get::<Option<f64>, _>("mean_cer").ok().flatten(),
                "mean_wer": r.try_get::<Option<f64>, _>("mean_wer").ok().flatten(),
                "min_cer": r.try_get::<Option<f64>, _>("min_cer").ok().flatten(),
                "max_cer": r.try_get::<Option<f64>, _>("max_cer").ok().flatten(),
                "mean_wall_ms": r.try_get::<Option<f64>, _>("mean_wall_ms").ok().flatten(),
            })
        })
        .collect();

    // Per-(case, engine) metrics. NO raw text columns (PII).
    let result_rows = match sqlx::query(
        "SELECT c.case_id AS case_ext, c.doc_type, c.gt_chars,
                res.engine, res.status,
                CAST(res.cer AS DOUBLE) AS cer,
                CAST(res.wer AS DOUBLE) AS wer,
                res.wall_ms, res.extracted_chars
         FROM ocr_eval_results res
         JOIN ocr_eval_cases c ON c.id = res.case_id
         WHERE res.run_id = ?
         ORDER BY c.case_id, res.engine",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return err500("fetch per-case results", e),
    };

    let results: Vec<Value> = result_rows
        .iter()
        .map(|r| {
            json!({
                "case_id": r.get::<String, _>("case_ext"),
                "doc_type": r.try_get::<Option<String>, _>("doc_type").ok().flatten(),
                "gt_chars": r.try_get::<Option<i32>, _>("gt_chars").ok().flatten(),
                "engine": r.get::<String, _>("engine"),
                "status": r.get::<String, _>("status"),
                "cer": r.try_get::<Option<f64>, _>("cer").ok().flatten(),
                "wer": r.try_get::<Option<f64>, _>("wer").ok().flatten(),
                "wall_ms": r.try_get::<Option<i32>, _>("wall_ms").ok().flatten(),
                "extracted_chars": r.try_get::<Option<i32>, _>("extracted_chars").ok().flatten(),
            })
        })
        .collect();

    let engines_list_str: Option<String> = run.try_get("engines").ok();
    let engines_list: Value = engines_list_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);

    Ok(Json(json!({
        "id": run.get::<String, _>("id"),
        "tenant": tenant,
        "name": run.try_get::<Option<String>, _>("name").ok().flatten(),
        "prompt_label": run.try_get::<Option<String>, _>("prompt_label").ok().flatten(),
        "engines": engines_list,
        "dataset_name": run.get::<String, _>("dataset_name"),
        "dataset_source": run.get::<String, _>("dataset_source"),
        "dataset_desc": run.try_get::<Option<String>, _>("dataset_desc").ok().flatten(),
        "started_at": run.try_get::<DateTime<Utc>, _>("started_at").ok(),
        "finished_at": run.try_get::<Option<DateTime<Utc>>, _>("finished_at").ok().flatten(),
        "notes": run.try_get::<Option<String>, _>("notes").ok().flatten(),
        "engine_summary": engines,
        "results": results,
    })))
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn err<T>(status: StatusCode, msg: &str) -> Result<T, (StatusCode, Json<Value>)> {
    Err((status, Json(json!({ "error": msg }))))
}

fn err500<T, E: std::fmt::Display>(label: &str, e: E) -> Result<T, (StatusCode, Json<Value>)> {
    warn!("eval_ocr_text {label}: {e}");
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": format!("{label}: {e}") })),
    ))
}

// ─── Router ────────────────────────────────────────────────────────────────

pub fn eval_ocr_text_routes() -> Router<DbPool> {
    Router::new()
        .route("/runs", get(list_runs))
        .route("/runs/{id}", get(get_run))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    fn headers_with(tenant: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(t) = tenant {
            h.insert("x-tenant-id", t.parse().unwrap());
        }
        h
    }

    #[test]
    fn tenant_defaults_to_platform() {
        assert_eq!(resolve_tenant(&headers_with(None)), "asgard_platform");
        assert_eq!(resolve_tenant(&headers_with(Some(""))), "asgard_platform");
        assert_eq!(resolve_tenant(&headers_with(Some("  "))), "asgard_platform");
    }

    #[test]
    fn tenant_honors_header() {
        assert_eq!(
            resolve_tenant(&headers_with(Some("asgard_medical"))),
            "asgard_medical"
        );
    }
}
