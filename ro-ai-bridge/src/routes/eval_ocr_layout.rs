//! OCR LAYOUT evaluation result storage — `asgard_platform` tenant.
//!
//! Endpoints:
//!   POST   /api/v1/eval/ocr/layout/runs           — Ingest a Syn layout eval result.
//!   GET    /api/v1/eval/ocr/layout/runs           — List runs (paginated, filterable).
//!   GET    /api/v1/eval/ocr/layout/runs/:id       — Run detail + per-image items.
//!
//! Targets the `ocr_layout_eval_*` tables (Sprint 53). Named to avoid
//! collision with the pre-existing Sprint 51 schema (`ocr_eval_*`) which
//! handles text-level OCR evaluation (CER/WER per engine). The two
//! schemas are complementary:
//!   ocr_eval_*         (Sprint 51) — text recognition across engines
//!   ocr_layout_eval_*  (Sprint 53, this module) — region detection geometry
//!
//! Schema lives in `migrations/sprint53_ocr_layout_eval_schema.sql`.
//!
//! Tenant: each request is scoped to the `X-Tenant-Id` header, falling
//! back to `asgard_platform` when the header is absent. This lets each
//! domain (asgard_medical / asgard_insurance / asgard_wellness) record and
//! read its own layout-eval runs, while cross-cutting engineering benchmarks
//! still land in `asgard_platform` by default. Writes bind the resolved
//! tenant; list/detail reads filter by it, so a run created under one tenant
//! is invisible (404) to another. The schema carries `tenant_id` per row
//! with an index on `(tenant_id, eval_kind, finished_at)`.
//!
//! PII safety: when `is_synthetic = false`, the handler refuses any item
//! with a non-null `image_name`. Real-data runs must use `image_hash`
//! only. Synthetic runs may use either.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

/// Default tenant for layout-eval rows when no `X-Tenant-Id` header is sent.
/// Cross-cutting engineering benchmarks live here.
const DEFAULT_TENANT: &str = "asgard_platform";

/// Resolve the tenant for this request: the `X-Tenant-Id` header value, or
/// `asgard_platform` when absent/empty. Unlike the shared
/// `routes::tenant::extract_tenant_id` (which defaults to `default_tenant`),
/// eng-metrics default to the platform bucket.
fn resolve_tenant(headers: &HeaderMap) -> String {
    headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_TENANT)
        .to_string()
}

// ─── Request types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub eval_kind: String,
    pub syn_version: String,
    #[serde(default)]
    pub commit_sha: Option<String>,
    pub model_name: String,
    #[serde(default)]
    pub model_sha256: Option<String>,
    pub dataset_name: String,
    #[serde(default)]
    pub dataset_hash: Option<String>,
    #[serde(default)]
    pub is_synthetic: bool,
    #[serde(default)]
    pub iou_threshold: Option<f32>,
    pub n_images: i64,
    #[serde(default)]
    pub n_gt_regions: Option<i64>,
    #[serde(default)]
    pub n_predictions: Option<i64>,
    pub summary: Value,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    #[serde(default)]
    pub items: Vec<CreateRunItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunItem {
    #[serde(default)]
    pub image_name: Option<String>,
    #[serde(default)]
    pub image_hash: Option<String>,
    #[serde(default)]
    pub image_width: Option<i64>,
    #[serde(default)]
    pub image_height: Option<i64>,
    #[serde(default)]
    pub n_gt: i64,
    #[serde(default)]
    pub n_pred: i64,
    #[serde(default)]
    pub n_matched: i64,
    #[serde(default)]
    pub metrics: Option<Value>,
    #[serde(default)]
    pub latency_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateRunResponse {
    pub id: String,
    pub items_created: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub eval_kind: Option<String>,
    #[serde(default)]
    pub syn_version: Option<String>,
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

// ─── Validation ──────────────────────────────────────────────────────────────

/// Layout/geometric eval kinds accepted by this schema. CER/WER (text-level)
/// is intentionally excluded — it belongs in the Sprint 51 ocr_eval_* schema.
const ALLOWED_EVAL_KINDS: [&str; 3] = ["mAP", "parity", "grits"];

/// Pure request validation, shared by the handler and unit tests:
///   1. `eval_kind` must be one of the layout kinds.
///   2. PII guard — a non-synthetic run must not carry any `image_name`
///      (real data is hash-only; names would leak PHI).
/// Returns `Err(message)` on the first violation.
fn validate_create_request(payload: &CreateRunRequest) -> Result<(), String> {
    if !ALLOWED_EVAL_KINDS.contains(&payload.eval_kind.as_str()) {
        return Err(format!(
            "eval_kind must be one of {ALLOWED_EVAL_KINDS:?}; got {:?}",
            payload.eval_kind
        ));
    }

    if !payload.is_synthetic {
        if let Some(bad) = payload.items.iter().find(|i| i.image_name.is_some()) {
            return Err(format!(
                "non-synthetic runs must not include image_name; use image_hash only \
                 (refer to asgard_medical.ocr_documents for cross-link); offending: {:?}",
                bad.image_name
            ));
        }
    }

    Ok(())
}

// ─── Handlers ──────────────────────────────────────────────────────────────

/// POST /api/v1/eval/ocr/runs
async fn create_run(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(payload): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<CreateRunResponse>), (StatusCode, Json<Value>)> {
    let tenant = resolve_tenant(&headers);

    // Reject bad eval_kind / PII-leaking payloads before touching the DB.
    if let Err(msg) = validate_create_request(&payload) {
        warn!("eval ocr create_run rejected: {msg}");
        return err(StatusCode::BAD_REQUEST, &msg);
    }

    let run_id = Uuid::new_v4().to_string();
    let summary_str = serde_json::to_string(&payload.summary).unwrap_or_else(|_| "{}".into());

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return err500("begin tx", e),
    };

    let insert_run = sqlx::query(
        r#"INSERT INTO ocr_layout_eval_runs
            (id, tenant_id, eval_kind, syn_version, commit_sha, model_name, model_sha256,
             dataset_name, dataset_hash, is_synthetic, iou_threshold,
             n_images, n_gt_regions, n_predictions, summary, started_at, finished_at)
           VALUES (?, ?, ?, ?, ?, ?, ?,
                   ?, ?, ?, ?,
                   ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&run_id)
    .bind(&tenant)
    .bind(&payload.eval_kind)
    .bind(&payload.syn_version)
    .bind(&payload.commit_sha)
    .bind(&payload.model_name)
    .bind(&payload.model_sha256)
    .bind(&payload.dataset_name)
    .bind(&payload.dataset_hash)
    .bind(payload.is_synthetic)
    .bind(payload.iou_threshold)
    .bind(payload.n_images)
    .bind(payload.n_gt_regions)
    .bind(payload.n_predictions)
    .bind(&summary_str)
    .bind(payload.started_at)
    .bind(payload.finished_at)
    .execute(&mut *tx)
    .await;

    if let Err(e) = insert_run {
        return err500("insert ocr_layout_eval_runs", e);
    }

    let mut items_created: i64 = 0;
    for item in &payload.items {
        let item_id = Uuid::new_v4().to_string();
        let metrics_str = item
            .metrics
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".into()));

        let r = sqlx::query(
            r#"INSERT INTO ocr_layout_eval_items
                (id, run_id, image_name, image_hash, image_width, image_height,
                 n_gt, n_pred, n_matched, metrics, latency_ms)
               VALUES (?, ?, ?, ?, ?, ?,
                       ?, ?, ?, ?, ?)"#,
        )
        .bind(&item_id)
        .bind(&run_id)
        .bind(&item.image_name)
        .bind(&item.image_hash)
        .bind(item.image_width)
        .bind(item.image_height)
        .bind(item.n_gt)
        .bind(item.n_pred)
        .bind(item.n_matched)
        .bind(&metrics_str)
        .bind(item.latency_ms)
        .execute(&mut *tx)
        .await;

        if let Err(e) = r {
            return err500("insert ocr_layout_eval_items", e);
        }
        items_created += 1;
    }

    if let Err(e) = tx.commit().await {
        return err500("commit tx", e);
    }

    info!(
        run_id = %run_id,
        tenant = %tenant,
        eval_kind = %payload.eval_kind,
        items_created,
        "created OCR eval run"
    );
    Ok((
        StatusCode::CREATED,
        Json(CreateRunResponse {
            id: run_id,
            items_created,
        }),
    ))
}

/// GET /api/v1/eval/ocr/runs?eval_kind=&limit=&offset=
async fn list_runs(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant = resolve_tenant(&headers);
    // Cap limit at 200 to keep response small.
    let limit = q.limit.clamp(1, 200);

    let mut sql = String::from(
        "SELECT id, eval_kind, syn_version, model_name, dataset_name, is_synthetic,
                n_images, n_gt_regions, n_predictions, summary,
                started_at, finished_at, duration_ms, created_at
         FROM ocr_layout_eval_runs
         WHERE tenant_id = ?",
    );
    if q.eval_kind.is_some() {
        sql.push_str(" AND eval_kind = ?");
    }
    if q.syn_version.is_some() {
        sql.push_str(" AND syn_version = ?");
    }
    if q.dataset_name.is_some() {
        sql.push_str(" AND dataset_name = ?");
    }
    sql.push_str(" ORDER BY finished_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql).bind(&tenant);
    if let Some(ref v) = q.eval_kind {
        query = query.bind(v);
    }
    if let Some(ref v) = q.syn_version {
        query = query.bind(v);
    }
    if let Some(ref v) = q.dataset_name {
        query = query.bind(v);
    }
    query = query.bind(limit).bind(q.offset);

    let rows = match query.fetch_all(&pool).await {
        Ok(r) => r,
        Err(e) => return err500("fetch ocr_layout_eval_runs list", e),
    };

    let runs: Vec<Value> = rows
        .iter()
        .map(|r| {
            let summary_str: Option<String> = r.try_get("summary").ok();
            let summary: Value = summary_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            json!({
                "id": r.get::<String, _>("id"),
                "eval_kind": r.get::<String, _>("eval_kind"),
                "syn_version": r.get::<String, _>("syn_version"),
                "model_name": r.get::<String, _>("model_name"),
                "dataset_name": r.get::<String, _>("dataset_name"),
                "is_synthetic": r.get::<bool, _>("is_synthetic"),
                "n_images": r.try_get::<i64, _>("n_images").ok(),
                "n_gt_regions": r.try_get::<i64, _>("n_gt_regions").ok(),
                "n_predictions": r.try_get::<i64, _>("n_predictions").ok(),
                "summary": summary,
                "started_at": r.try_get::<DateTime<Utc>, _>("started_at").ok(),
                "finished_at": r.try_get::<DateTime<Utc>, _>("finished_at").ok(),
                "duration_ms": r.try_get::<i64, _>("duration_ms").ok(),
                "created_at": r.try_get::<DateTime<Utc>, _>("created_at").ok(),
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

/// GET /api/v1/eval/ocr/runs/{id}
async fn get_run(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant = resolve_tenant(&headers);
    let row = match sqlx::query(
        r#"SELECT id, eval_kind, syn_version, commit_sha, model_name, model_sha256,
                  dataset_name, dataset_hash, is_synthetic, iou_threshold,
                  n_images, n_gt_regions, n_predictions, summary,
                  started_at, finished_at, duration_ms, created_at
           FROM ocr_layout_eval_runs
           WHERE id = ? AND tenant_id = ?"#,
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

    let items_rows = match sqlx::query(
        r#"SELECT id, image_name, image_hash, image_width, image_height,
                  n_gt, n_pred, n_matched, metrics, latency_ms, created_at
           FROM ocr_layout_eval_items
           WHERE run_id = ?
           ORDER BY image_name, id"#,
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    {
        Ok(r) => r,
        Err(e) => return err500("fetch items", e),
    };

    let summary_str: Option<String> = row.try_get("summary").ok();
    let summary: Value = summary_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);

    let items: Vec<Value> = items_rows
        .iter()
        .map(|r| {
            let metrics_str: Option<String> = r.try_get("metrics").ok();
            let metrics: Value = metrics_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            json!({
                "id": r.get::<String, _>("id"),
                "image_name": r.try_get::<Option<String>, _>("image_name").ok().flatten(),
                "image_hash": r.try_get::<Option<String>, _>("image_hash").ok().flatten(),
                "image_width": r.try_get::<Option<i64>, _>("image_width").ok().flatten(),
                "image_height": r.try_get::<Option<i64>, _>("image_height").ok().flatten(),
                "n_gt": r.get::<i64, _>("n_gt"),
                "n_pred": r.get::<i64, _>("n_pred"),
                "n_matched": r.get::<i64, _>("n_matched"),
                "metrics": metrics,
                "latency_ms": r.try_get::<Option<i64>, _>("latency_ms").ok().flatten(),
                "created_at": r.try_get::<DateTime<Utc>, _>("created_at").ok(),
            })
        })
        .collect();

    Ok(Json(json!({
        "id": row.get::<String, _>("id"),
        "tenant": tenant,
        "eval_kind": row.get::<String, _>("eval_kind"),
        "syn_version": row.get::<String, _>("syn_version"),
        "commit_sha": row.try_get::<Option<String>, _>("commit_sha").ok().flatten(),
        "model_name": row.get::<String, _>("model_name"),
        "model_sha256": row.try_get::<Option<String>, _>("model_sha256").ok().flatten(),
        "dataset_name": row.get::<String, _>("dataset_name"),
        "dataset_hash": row.try_get::<Option<String>, _>("dataset_hash").ok().flatten(),
        "is_synthetic": row.get::<bool, _>("is_synthetic"),
        "iou_threshold": row.try_get::<Option<f64>, _>("iou_threshold").ok().flatten(),
        "n_images": row.try_get::<i64, _>("n_images").ok(),
        "n_gt_regions": row.try_get::<Option<i64>, _>("n_gt_regions").ok().flatten(),
        "n_predictions": row.try_get::<Option<i64>, _>("n_predictions").ok().flatten(),
        "summary": summary,
        "items": items,
        "started_at": row.try_get::<DateTime<Utc>, _>("started_at").ok(),
        "finished_at": row.try_get::<DateTime<Utc>, _>("finished_at").ok(),
        "duration_ms": row.try_get::<i64, _>("duration_ms").ok(),
        "created_at": row.try_get::<DateTime<Utc>, _>("created_at").ok(),
    })))
}

// ─── Helpers ───────────────────────────────────────────────────────────────
//
// Errors return the tuple `(StatusCode, Json<Value>)` directly so callers
// with different success types (CreateRunResponse vs Value) can share them.
// Wrap with `return Err(err(...));` at the call site.

fn err<T>(status: StatusCode, msg: &str) -> Result<T, (StatusCode, Json<Value>)> {
    Err((status, Json(json!({ "error": msg }))))
}

fn err500<T, E: std::fmt::Display>(
    label: &str,
    e: E,
) -> Result<T, (StatusCode, Json<Value>)> {
    warn!("eval_ocr_layout {label}: {e}");
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": format!("{label}: {e}") })),
    ))
}

// ─── Router ────────────────────────────────────────────────────────────────

pub fn eval_ocr_layout_routes() -> Router<DbPool> {
    Router::new()
        .route("/runs", post(create_run).get(list_runs))
        .route("/runs/{id}", get(get_run))
}

// ─── Tests ─────────────────────────────────────────────────────────────────
// Pure-logic coverage (no DB): tenant resolution + request validation. The
// DB roundtrip is exercised by the runbook's end-to-end check (syn-eval-ingest
// → POST → GET) since route handlers here need a live MariaDB.

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use serde_json::json;

    fn item(image_name: Option<&str>, image_hash: Option<&str>) -> CreateRunItem {
        CreateRunItem {
            image_name: image_name.map(String::from),
            image_hash: image_hash.map(String::from),
            image_width: None,
            image_height: None,
            n_gt: 0,
            n_pred: 0,
            n_matched: 0,
            metrics: None,
            latency_ms: None,
        }
    }

    fn req(eval_kind: &str, is_synthetic: bool, items: Vec<CreateRunItem>) -> CreateRunRequest {
        CreateRunRequest {
            eval_kind: eval_kind.to_string(),
            syn_version: "v0.3.0".into(),
            commit_sha: None,
            model_name: "doclayout-yolo".into(),
            model_sha256: None,
            dataset_name: "synthetic-handwriting-5".into(),
            dataset_hash: None,
            is_synthetic,
            iou_threshold: Some(0.5),
            n_images: items.len() as i64,
            n_gt_regions: None,
            n_predictions: None,
            summary: json!({}),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            items,
        }
    }

    fn headers_with(tenant: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(t) = tenant {
            h.insert("x-tenant-id", t.parse().unwrap());
        }
        h
    }

    // ── resolve_tenant ──────────────────────────────────────────────────────

    #[test]
    fn tenant_defaults_to_platform_when_header_absent() {
        assert_eq!(resolve_tenant(&headers_with(None)), "asgard_platform");
    }

    #[test]
    fn tenant_honors_header() {
        assert_eq!(
            resolve_tenant(&headers_with(Some("asgard_medical"))),
            "asgard_medical"
        );
    }

    #[test]
    fn tenant_blank_or_whitespace_falls_back_to_platform() {
        assert_eq!(resolve_tenant(&headers_with(Some(""))), "asgard_platform");
        assert_eq!(resolve_tenant(&headers_with(Some("   "))), "asgard_platform");
    }

    // ── validate_create_request ───────────────────────────────────────────────

    #[test]
    fn accepts_allowed_eval_kinds() {
        for kind in ["mAP", "parity", "grits"] {
            assert!(validate_create_request(&req(kind, true, vec![])).is_ok(), "{kind}");
        }
    }

    #[test]
    fn rejects_unknown_eval_kind() {
        // CER/WER belongs in the Sprint 51 ocr_eval_* schema, not here.
        let e = validate_create_request(&req("CER", true, vec![])).unwrap_err();
        assert!(e.contains("eval_kind must be one of"), "{e}");
    }

    #[test]
    fn rejects_image_name_on_non_synthetic_run() {
        let r = req("mAP", false, vec![item(Some("patient-cert.png"), Some("abc123"))]);
        let e = validate_create_request(&r).unwrap_err();
        assert!(e.contains("must not include image_name"), "{e}");
    }

    #[test]
    fn allows_image_hash_only_on_non_synthetic_run() {
        let r = req("mAP", false, vec![item(None, Some("abc123"))]);
        assert!(validate_create_request(&r).is_ok());
    }

    #[test]
    fn allows_image_name_on_synthetic_run() {
        let r = req("mAP", true, vec![item(Some("hw-rx-01.png"), None)]);
        assert!(validate_create_request(&r).is_ok());
    }
}
