//! Sprint 39 — Mimir Curator + LoRA MLOps Tracking API routes
//!
//! Curator (Mimir's annotation workflow, see ADR-001):
//!   - POST   /api/v1/training/datasets                              — create dataset
//!   - GET    /api/v1/training/datasets                              — list datasets (tenant-filtered + shared)
//!   - GET    /api/v1/training/datasets/:id                          — dataset detail + counts
//!   - POST   /api/v1/training/datasets/:id/items                    — bulk import JSONL items
//!   - GET    /api/v1/training/datasets/:id/queue                    — next pending item for reviewer
//!   - POST   /api/v1/training/datasets/:id/items/:item_id/review    — submit rating
//!   - GET    /api/v1/training/datasets/:id/export.jsonl             — stream approved items
//!
//! LoRA training tracking (MLOps, see ADR-002):
//!   - POST   /api/v1/training/runs                                  — register a training run
//!   - GET    /api/v1/training/runs                                  — list runs
//!   - GET    /api/v1/training/runs/:id                              — run detail (hyperparams, loss curve)
//!   - POST   /api/v1/training/runs/:id/log                          — append loss-curve tick (called by trainer)
//!   - PATCH  /api/v1/training/runs/:id                              — update status / adapter_path / merged_model_id
//!
//! Tenant scoping rule:
//!   - Datasets / runs with `tenant_id IS NULL` are SHARED (visible to all tenants).
//!   - Non-NULL `tenant_id` = scoped to that tenant only.
//!   - Reads always include `(tenant_id IS NULL OR tenant_id = ?)`.
//!   - Writes (create / import / review / log) require explicit `tenant_id`
//!     (defaults to caller's `tenant.tenant_id`; pass `tenant_id: null` in body to share).

use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use mimir_core_ai::middleware::dual_mode_auth::dual_mode_auth_middleware;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

// ─── Curator: Datasets ───────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct CorpusDataset {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tenant_id: Option<String>,
    pub source: Option<String>,
    pub metadata: Option<String>,
    pub status: String,
    pub total_items: i32,
    pub approved_items: i32,
    pub rejected_items: i32,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDatasetRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// `null` = shared (visible to all tenants); omitted = caller's tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<Option<String>>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub metadata: Option<JsonValue>,
}

// ─── Curator: Items ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct CorpusItem {
    pub id: i64,
    pub dataset_id: String,
    pub question: String,
    pub ai_answer: String,
    pub expected_answer: Option<String>,
    pub citations: Option<String>,
    pub accuracy_score: Option<i8>,
    pub completeness_score: Option<i8>,
    pub relevance_score: Option<i8>,
    pub safety_score: Option<i8>,
    pub improved_answer: Option<String>,
    pub specialty: Option<String>,
    /// JSON array of cross-cutting tags, e.g. `["pharmacy","geriatric"]`.
    /// Stored as string; clients parse to Vec<String> or display as chips.
    /// Sprint 39 multi-tag (2026-05-06).
    pub tags: Option<String>,
    pub status: String,
    pub reviewer_id: Option<String>,
    pub reviewer_notes: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub tenant_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ImportItem {
    pub question: String,
    pub ai_answer: String,
    #[serde(default)]
    pub expected_answer: Option<String>,
    #[serde(default)]
    pub citations: Option<JsonValue>,
    #[serde(default)]
    pub specialty: Option<String>,
    /// Cross-cutting tags (Sprint 39 multi-tag). Stored as JSON array.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ImportItemsRequest {
    pub items: Vec<ImportItem>,
}

#[derive(Debug, Serialize)]
pub struct ImportItemsResponse {
    pub imported: usize,
    pub dataset_id: String,
}

#[derive(Debug, Deserialize)]
pub struct QueueQuery {
    /// If set, return next item assigned to (or unclaimed for) this reviewer.
    #[serde(default)]
    pub reviewer: Option<String>,
    /// Optional specialty filter.
    #[serde(default)]
    pub specialty: Option<String>,
    /// Optional tag filter — match items whose `tags` JSON array contains this string.
    /// Sprint 39 multi-tag (2026-05-06).
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewSubmission {
    /// 1-5; required to mark APPROVED. Pass null + status=REJECTED to reject.
    #[serde(default)]
    pub accuracy_score: Option<i8>,
    #[serde(default)]
    pub completeness_score: Option<i8>,
    #[serde(default)]
    pub relevance_score: Option<i8>,
    /// 1 = safe, 0 = unsafe.
    #[serde(default)]
    pub safety_score: Option<i8>,
    #[serde(default)]
    pub improved_answer: Option<String>,
    #[serde(default)]
    pub specialty: Option<String>,
    /// Cross-cutting tags (Sprint 39 multi-tag). Replaces existing tags on save.
    /// Pass empty array to clear; pass None (omit) to leave unchanged.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub notes: Option<String>,
    /// One of APPROVED / REJECTED / FLAGGED.
    pub status: String,
}

// ─── LoRA training runs ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct LoraRun {
    pub id: String,
    pub name: Option<String>,
    pub dataset_id: Option<String>,
    pub dataset_snapshot_hash: Option<String>,
    pub base_model_id: String,
    pub hyperparams: Option<String>,
    pub loss_curve: Option<String>,
    pub adapter_path: Option<String>,
    pub merged_model_id: Option<String>,
    pub status: String,
    pub status_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub tenant_id: Option<String>,
    pub created_by: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub dataset_snapshot_hash: Option<String>,
    pub base_model_id: String,
    #[serde(default)]
    pub hyperparams: Option<JsonValue>,
    /// `Some(None)` = explicitly shared. Omitted = caller's tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<Option<String>>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogTickRequest {
    /// Append-mode metric tick. Server appends to `loss_curve` JSON array.
    pub step: i32,
    pub loss: f32,
    #[serde(default)]
    pub val_loss: Option<f32>,
    #[serde(default)]
    pub lr: Option<f32>,
    #[serde(default)]
    pub extra: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct PatchRunRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub adapter_path: Option<String>,
    #[serde(default)]
    pub merged_model_id: Option<String>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
}

// ─── Router ──────────────────────────────────────────────────────────────────

pub fn training_routes() -> Router<DbPool> {
    Router::new()
        // Curator
        .route("/api/v1/training/datasets", get(list_datasets).post(create_dataset))
        .route("/api/v1/training/datasets/{id}", get(get_dataset))
        .route("/api/v1/training/datasets/{id}/items", post(import_items))
        .route("/api/v1/training/datasets/{id}/queue", get(get_queue_next))
        .route(
            "/api/v1/training/datasets/{id}/items/{item_id}/review",
            post(submit_review),
        )
        .route(
            "/api/v1/training/datasets/{id}/export.jsonl",
            get(export_dataset_jsonl),
        )
        // LoRA training runs
        .route("/api/v1/training/runs", get(list_runs).post(create_run))
        .route("/api/v1/training/runs/{id}", get(get_run).patch(patch_run))
        .route("/api/v1/training/runs/{id}/log", post(log_tick))
        .layer(axum::middleware::from_fn(dual_mode_auth_middleware))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Resolve effective tenant_id for write operations. None = explicitly shared.
fn resolve_tenant_for_write(
    body_field: &Option<Option<String>>,
    caller: &TenantContext,
) -> Option<String> {
    match body_field {
        Some(None) => None,
        Some(Some(t)) => Some(t.clone()),
        None => Some(caller.tenant_id.clone()),
    }
}

// ─── Curator handlers ────────────────────────────────────────────────────────

async fn list_datasets(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
) -> Json<Vec<CorpusDataset>> {
    let rows = sqlx::query_as::<_, CorpusDataset>(
        "SELECT id, name, description, tenant_id, source, metadata, status,
                total_items, approved_items, rejected_items, created_at, created_by
         FROM training_corpus_datasets
         WHERE tenant_id IS NULL OR tenant_id = ?
         ORDER BY created_at DESC",
    )
    .bind(&tenant.tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "list_datasets_failed", tenant = %tenant.tenant_id, error = %e);
        vec![]
    });
    Json(rows)
}

async fn create_dataset(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Json(req): Json<CreateDatasetRequest>,
) -> Result<Json<CorpusDataset>, (StatusCode, String)> {
    let id = Uuid::new_v4().to_string();
    let effective_tenant = resolve_tenant_for_write(&req.tenant_id, &tenant);
    let metadata_str = req.metadata.as_ref().map(|m| m.to_string());
    sqlx::query(
        "INSERT INTO training_corpus_datasets
            (id, name, description, tenant_id, source, metadata, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&effective_tenant)
    .bind(&req.source)
    .bind(&metadata_str)
    .bind(&tenant.user_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("create failed: {e}")))?;

    let row = sqlx::query_as::<_, CorpusDataset>(
        "SELECT id, name, description, tenant_id, source, metadata, status,
                total_items, approved_items, rejected_items, created_at, created_by
         FROM training_corpus_datasets WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read-back failed: {e}")))?;

    Ok(Json(row))
}

async fn get_dataset(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
) -> Result<Json<CorpusDataset>, StatusCode> {
    sqlx::query_as::<_, CorpusDataset>(
        "SELECT id, name, description, tenant_id, source, metadata, status,
                total_items, approved_items, rejected_items, created_at, created_by
         FROM training_corpus_datasets
         WHERE id = ? AND (tenant_id IS NULL OR tenant_id = ?)",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map(Json)
    .ok_or(StatusCode::NOT_FOUND)
}

async fn import_items(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(dataset_id): Path<String>,
    Json(req): Json<ImportItemsRequest>,
) -> Result<Json<ImportItemsResponse>, (StatusCode, String)> {
    // Verify access to dataset.
    let dataset_tenant = sqlx::query_scalar::<_, Option<String>>(
        "SELECT tenant_id FROM training_corpus_datasets
         WHERE id = ? AND (tenant_id IS NULL OR tenant_id = ?)",
    )
    .bind(&dataset_id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("dataset lookup: {e}")))?
    .ok_or((StatusCode::NOT_FOUND, "dataset not found".into()))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("tx start: {e}")))?;
    let mut count = 0usize;
    for item in &req.items {
        let citations_str = item.citations.as_ref().map(|c| c.to_string());
        let tags_str = item
            .tags
            .as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "null".into()));
        sqlx::query(
            "INSERT INTO training_corpus_items
                (dataset_id, question, ai_answer, expected_answer, citations,
                 specialty, tags, tenant_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&dataset_id)
        .bind(&item.question)
        .bind(&item.ai_answer)
        .bind(&item.expected_answer)
        .bind(&citations_str)
        .bind(&item.specialty)
        .bind(&tags_str)
        .bind(&dataset_tenant)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("insert item: {e}")))?;
        count += 1;
    }
    sqlx::query(
        "UPDATE training_corpus_datasets SET total_items = total_items + ? WHERE id = ?",
    )
    .bind(count as i64)
    .bind(&dataset_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("update count: {e}")))?;
    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("commit: {e}")))?;

    Ok(Json(ImportItemsResponse {
        imported: count,
        dataset_id,
    }))
}

async fn get_queue_next(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(dataset_id): Path<String>,
    Query(q): Query<QueueQuery>,
) -> Result<Json<Option<CorpusItem>>, StatusCode> {
    // Pull next PENDING item not yet claimed (reviewer_id IS NULL) optionally
    // filtered by specialty. Tenant-scoped via dataset.
    let mut sql = String::from(
        "SELECT i.id, i.dataset_id, i.question, i.ai_answer, i.expected_answer,
                i.citations, i.accuracy_score, i.completeness_score, i.relevance_score,
                i.safety_score, i.improved_answer, i.specialty, i.tags, i.status,
                i.reviewer_id, i.reviewer_notes, i.reviewed_at, i.tenant_id, i.created_at
         FROM training_corpus_items i
         JOIN training_corpus_datasets d ON i.dataset_id = d.id
         WHERE i.dataset_id = ?
           AND i.status = 'PENDING'
           AND (d.tenant_id IS NULL OR d.tenant_id = ?)",
    );
    if q.specialty.is_some() {
        sql.push_str(" AND i.specialty = ?");
    }
    if q.tag.is_some() {
        // JSON_CONTAINS finds tag in the JSON array. tag string must be a JSON-quoted value.
        sql.push_str(" AND JSON_CONTAINS(i.tags, JSON_QUOTE(?))");
    }
    sql.push_str(" ORDER BY i.id ASC LIMIT 1");

    let mut query = sqlx::query_as::<_, CorpusItem>(&sql)
        .bind(&dataset_id)
        .bind(&tenant.tenant_id);
    if let Some(sp) = &q.specialty {
        query = query.bind(sp);
    }
    if let Some(tag) = &q.tag {
        query = query.bind(tag);
    }
    let _ = q.reviewer; // reserved for future per-reviewer queue assignment
    let row = query
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(row))
}

async fn submit_review(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path((dataset_id, item_id)): Path<(String, i64)>,
    Json(req): Json<ReviewSubmission>,
) -> Result<Json<CorpusItem>, (StatusCode, String)> {
    let status = match req.status.as_str() {
        "APPROVED" | "REJECTED" | "FLAGGED" => req.status.clone(),
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("invalid status: {other} (want APPROVED|REJECTED|FLAGGED)"),
            ))
        }
    };

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("tx start: {e}")))?;

    // Tags semantics:
    //   None        → leave existing tags unchanged
    //   Some(empty) → clear tags to NULL
    //   Some(items) → replace with provided JSON array
    let tags_str = req
        .tags
        .as_ref()
        .map(|t| {
            if t.is_empty() {
                None
            } else {
                Some(serde_json::to_string(t).unwrap_or_else(|_| "null".into()))
            }
        });
    // Three-state for SQL: distinguish "do not touch" from "set NULL" from "set value".
    // We do this by branching on tags_str.
    let updated = match tags_str {
        Some(maybe_json) => {
            sqlx::query(
                "UPDATE training_corpus_items i
                 JOIN training_corpus_datasets d ON i.dataset_id = d.id
                 SET i.accuracy_score = ?, i.completeness_score = ?, i.relevance_score = ?,
                     i.safety_score = ?, i.improved_answer = COALESCE(?, i.improved_answer),
                     i.specialty = COALESCE(?, i.specialty),
                     i.tags = ?,
                     i.reviewer_id = ?, i.reviewer_notes = ?, i.status = ?,
                     i.reviewed_at = NOW()
                 WHERE i.id = ? AND i.dataset_id = ?
                   AND (d.tenant_id IS NULL OR d.tenant_id = ?)",
            )
            .bind(req.accuracy_score)
            .bind(req.completeness_score)
            .bind(req.relevance_score)
            .bind(req.safety_score)
            .bind(&req.improved_answer)
            .bind(&req.specialty)
            .bind(&maybe_json)
            .bind(&tenant.user_id)
            .bind(&req.notes)
            .bind(&status)
            .bind(item_id)
            .bind(&dataset_id)
            .bind(&tenant.tenant_id)
            .execute(&mut *tx)
            .await
        }
        None => {
            sqlx::query(
                "UPDATE training_corpus_items i
                 JOIN training_corpus_datasets d ON i.dataset_id = d.id
                 SET i.accuracy_score = ?, i.completeness_score = ?, i.relevance_score = ?,
                     i.safety_score = ?, i.improved_answer = COALESCE(?, i.improved_answer),
                     i.specialty = COALESCE(?, i.specialty),
                     i.reviewer_id = ?, i.reviewer_notes = ?, i.status = ?,
                     i.reviewed_at = NOW()
                 WHERE i.id = ? AND i.dataset_id = ?
                   AND (d.tenant_id IS NULL OR d.tenant_id = ?)",
            )
            .bind(req.accuracy_score)
            .bind(req.completeness_score)
            .bind(req.relevance_score)
            .bind(req.safety_score)
            .bind(&req.improved_answer)
            .bind(&req.specialty)
            .bind(&tenant.user_id)
            .bind(&req.notes)
            .bind(&status)
            .bind(item_id)
            .bind(&dataset_id)
            .bind(&tenant.tenant_id)
            .execute(&mut *tx)
            .await
        }
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("update review: {e}")))?;

    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "item not found".into()));
    }

    // Maintain rollup counters on the dataset.
    let counter_col = match status.as_str() {
        "APPROVED" => Some("approved_items"),
        "REJECTED" => Some("rejected_items"),
        _ => None,
    };
    if let Some(col) = counter_col {
        let q = format!(
            "UPDATE training_corpus_datasets SET {col} = {col} + 1 WHERE id = ?"
        );
        sqlx::query(&q)
            .bind(&dataset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("rollup: {e}")))?;
    }

    let row = sqlx::query_as::<_, CorpusItem>(
        "SELECT id, dataset_id, question, ai_answer, expected_answer, citations,
                accuracy_score, completeness_score, relevance_score, safety_score,
                improved_answer, specialty, tags, status, reviewer_id, reviewer_notes,
                reviewed_at, tenant_id, created_at
         FROM training_corpus_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read-back: {e}")))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("commit: {e}")))?;

    Ok(Json(row))
}

async fn export_dataset_jsonl(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(dataset_id): Path<String>,
) -> Result<Response, StatusCode> {
    // Verify access.
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT id FROM training_corpus_datasets
         WHERE id = ? AND (tenant_id IS NULL OR tenant_id = ?)",
    )
    .bind(&dataset_id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let rows: Vec<(String, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT question, ai_answer, improved_answer, specialty, tags
         FROM training_corpus_items
         WHERE dataset_id = ? AND status = 'APPROVED'
         ORDER BY id ASC",
    )
    .bind(&dataset_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut body = String::new();
    for (q, ai, improved, specialty, tags) in rows {
        // Use improved_answer if present (the "gold" target), else ai_answer as-is.
        let completion = improved.unwrap_or(ai);
        // Parse tags JSON (Vec<String>) for export metadata; fall back to empty array.
        let tags_array: Vec<String> = tags
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let line = serde_json::json!({
            "prompt": q,
            "completion": completion,
            "metadata": {
                "specialty": specialty,
                "tags": tags_array,
                "dataset_id": dataset_id,
            }
        });
        body.push_str(&line.to_string());
        body.push('\n');
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{dataset_id}.jsonl\""),
        )
        .body(Body::from(body))
        .unwrap())
}

// ─── LoRA training run handlers ──────────────────────────────────────────────

async fn list_runs(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
) -> Json<Vec<LoraRun>> {
    let rows = sqlx::query_as::<_, LoraRun>(
        "SELECT id, name, dataset_id, dataset_snapshot_hash, base_model_id,
                hyperparams, loss_curve, adapter_path, merged_model_id,
                status, status_message, started_at, finished_at,
                tenant_id, created_by, notes
         FROM lora_training_runs
         WHERE tenant_id IS NULL OR tenant_id = ?
         ORDER BY started_at DESC",
    )
    .bind(&tenant.tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "list_lora_runs_failed", tenant = %tenant.tenant_id, error = %e);
        vec![]
    });
    Json(rows)
}

async fn create_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Json(req): Json<CreateRunRequest>,
) -> Result<Json<LoraRun>, (StatusCode, String)> {
    let id = Uuid::new_v4().to_string();
    let effective_tenant = resolve_tenant_for_write(&req.tenant_id, &tenant);
    let hyperparams_str = req.hyperparams.as_ref().map(|h| h.to_string());

    sqlx::query(
        "INSERT INTO lora_training_runs
            (id, name, dataset_id, dataset_snapshot_hash, base_model_id,
             hyperparams, status, tenant_id, created_by, notes)
         VALUES (?, ?, ?, ?, ?, ?, 'PENDING', ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.dataset_id)
    .bind(&req.dataset_snapshot_hash)
    .bind(&req.base_model_id)
    .bind(&hyperparams_str)
    .bind(&effective_tenant)
    .bind(&tenant.user_id)
    .bind(&req.notes)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("insert run: {e}")))?;

    let row = sqlx::query_as::<_, LoraRun>(
        "SELECT id, name, dataset_id, dataset_snapshot_hash, base_model_id,
                hyperparams, loss_curve, adapter_path, merged_model_id,
                status, status_message, started_at, finished_at,
                tenant_id, created_by, notes
         FROM lora_training_runs WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read-back: {e}")))?;
    Ok(Json(row))
}

async fn get_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
) -> Result<Json<LoraRun>, StatusCode> {
    sqlx::query_as::<_, LoraRun>(
        "SELECT id, name, dataset_id, dataset_snapshot_hash, base_model_id,
                hyperparams, loss_curve, adapter_path, merged_model_id,
                status, status_message, started_at, finished_at,
                tenant_id, created_by, notes
         FROM lora_training_runs
         WHERE id = ? AND (tenant_id IS NULL OR tenant_id = ?)",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map(Json)
    .ok_or(StatusCode::NOT_FOUND)
}

async fn log_tick(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
    Json(req): Json<LogTickRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Append tick to loss_curve JSON array.
    let tick = serde_json::json!({
        "step": req.step,
        "loss": req.loss,
        "val_loss": req.val_loss,
        "lr": req.lr,
        "extra": req.extra,
        "ts": Utc::now().to_rfc3339(),
    });

    // Use JSON_ARRAY_APPEND for atomic append; initialize to [] if NULL.
    let updated = sqlx::query(
        "UPDATE lora_training_runs
         SET loss_curve = JSON_ARRAY_APPEND(IFNULL(loss_curve, JSON_ARRAY()), '$', CAST(? AS JSON)),
             status = CASE WHEN status = 'PENDING' THEN 'RUNNING' ELSE status END
         WHERE id = ? AND (tenant_id IS NULL OR tenant_id = ?)",
    )
    .bind(tick.to_string())
    .bind(&id)
    .bind(&tenant.tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("append tick: {e}")))?;
    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "run not found".into()));
    }
    Ok(StatusCode::ACCEPTED)
}

async fn patch_run(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
    Json(req): Json<PatchRunRequest>,
) -> Result<Json<LoraRun>, (StatusCode, String)> {
    if let Some(status) = &req.status {
        match status.as_str() {
            "PENDING" | "RUNNING" | "COMPLETED" | "FAILED" | "CANCELLED" => {}
            other => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("invalid status: {other}"),
                ))
            }
        }
    }
    sqlx::query(
        "UPDATE lora_training_runs
         SET status         = COALESCE(?, status),
             status_message = COALESCE(?, status_message),
             adapter_path   = COALESCE(?, adapter_path),
             merged_model_id = COALESCE(?, merged_model_id),
             finished_at    = COALESCE(?, finished_at)
         WHERE id = ? AND (tenant_id IS NULL OR tenant_id = ?)",
    )
    .bind(&req.status)
    .bind(&req.status_message)
    .bind(&req.adapter_path)
    .bind(&req.merged_model_id)
    .bind(&req.finished_at)
    .bind(&id)
    .bind(&tenant.tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("patch run: {e}")))?;

    sqlx::query_as::<_, LoraRun>(
        "SELECT id, name, dataset_id, dataset_snapshot_hash, base_model_id,
                hyperparams, loss_curve, adapter_path, merged_model_id,
                status, status_message, started_at, finished_at,
                tenant_id, created_by, notes
         FROM lora_training_runs WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map(Json)
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read-back: {e}")))
}
