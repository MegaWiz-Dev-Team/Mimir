//! OCR evaluation result storage — `asgard_platform` tenant.
//!
//! Endpoints:
//!   POST   /api/v1/eval/ocr/runs           — Ingest a Syn eval result.
//!   GET    /api/v1/eval/ocr/runs           — List runs (paginated, filterable).
//!   GET    /api/v1/eval/ocr/runs/:id       — Run detail + per-image items.
//!
//! The OCR eval surface is intentionally separate from `rag_eval` /
//! `eval_scores` because the OCR data shape is geometric (bboxes + IoU)
//! and would not fit cleanly into the question-scored agent eval tables.
//! Schema lives in `migrations/sprint53_ocr_eval_schema.sql`.
//!
//! Tenant: ALL writes/reads here are scoped to `asgard_platform`. The
//! header X-Tenant-Id is intentionally ignored on these routes so an
//! agent ingest from any tenant context still lands in the right bucket
//! (engineering metrics are cross-cutting). The schema default also
//! enforces this.
//!
//! PII safety: when `is_synthetic = false`, the handler refuses any item
//! with a non-null `image_name`. Real-data runs must use `image_hash`
//! only. Synthetic runs may use either.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
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

const ASGARD_PLATFORM: &str = "asgard_platform";

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

// ─── Handlers ──────────────────────────────────────────────────────────────

/// POST /api/v1/eval/ocr/runs
async fn create_run(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<CreateRunResponse>), (StatusCode, Json<Value>)> {
    // Eval kind allow-list — defends against accidental free-text writes.
    let allowed = ["mAP", "parity", "cer_wer", "grits"];
    if !allowed.contains(&payload.eval_kind.as_str()) {
        return err(StatusCode::BAD_REQUEST, &format!(
            "eval_kind must be one of {allowed:?}; got {:?}",
            payload.eval_kind
        ));
    }

    // PII guard: non-synthetic runs MUST NOT include image_name.
    if !payload.is_synthetic {
        if let Some(bad) = payload.items.iter().find(|i| i.image_name.is_some()) {
            warn!(
                "eval ocr create_run rejected: non-synthetic item has image_name {:?}",
                bad.image_name
            );
            return err(
                StatusCode::BAD_REQUEST,
                "non-synthetic runs must not include image_name; use image_hash only \
                 (refer to asgard_medical.ocr_documents for cross-link)",
            );
        }
    }

    let run_id = Uuid::new_v4().to_string();
    let summary_str = serde_json::to_string(&payload.summary).unwrap_or_else(|_| "{}".into());

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return err500("begin tx", e),
    };

    let insert_run = sqlx::query(
        r#"INSERT INTO ocr_eval_runs
            (id, tenant_id, eval_kind, syn_version, commit_sha, model_name, model_sha256,
             dataset_name, dataset_hash, is_synthetic, iou_threshold,
             n_images, n_gt_regions, n_predictions, summary, started_at, finished_at)
           VALUES (?, ?, ?, ?, ?, ?, ?,
                   ?, ?, ?, ?,
                   ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&run_id)
    .bind(ASGARD_PLATFORM)
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
        return err500("insert ocr_eval_runs", e);
    }

    let mut items_created: i64 = 0;
    for item in &payload.items {
        let item_id = Uuid::new_v4().to_string();
        let metrics_str = item
            .metrics
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".into()));

        let r = sqlx::query(
            r#"INSERT INTO ocr_eval_items
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
            return err500("insert ocr_eval_items", e);
        }
        items_created += 1;
    }

    if let Err(e) = tx.commit().await {
        return err500("commit tx", e);
    }

    info!(
        run_id = %run_id,
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
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Cap limit at 200 to keep response small.
    let limit = q.limit.clamp(1, 200);

    let mut sql = String::from(
        "SELECT id, eval_kind, syn_version, model_name, dataset_name, is_synthetic,
                n_images, n_gt_regions, n_predictions, summary,
                started_at, finished_at, duration_ms, created_at
         FROM ocr_eval_runs
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

    let mut query = sqlx::query(&sql).bind(ASGARD_PLATFORM);
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
        Err(e) => return err500("fetch ocr_eval_runs list", e),
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

    Ok(Json(json!({ "runs": runs, "limit": limit, "offset": q.offset })))
}

/// GET /api/v1/eval/ocr/runs/{id}
async fn get_run(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let row = match sqlx::query(
        r#"SELECT id, eval_kind, syn_version, commit_sha, model_name, model_sha256,
                  dataset_name, dataset_hash, is_synthetic, iou_threshold,
                  n_images, n_gt_regions, n_predictions, summary,
                  started_at, finished_at, duration_ms, created_at
           FROM ocr_eval_runs
           WHERE id = ? AND tenant_id = ?"#,
    )
    .bind(&id)
    .bind(ASGARD_PLATFORM)
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
           FROM ocr_eval_items
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
    warn!("eval_ocr {label}: {e}");
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": format!("{label}: {e}") })),
    ))
}

// ─── Router ────────────────────────────────────────────────────────────────

pub fn eval_ocr_routes() -> Router<DbPool> {
    Router::new()
        .route("/runs", post(create_run).get(list_runs))
        .route("/runs/{id}", get(get_run))
}
