//! OCR Annotation Workflow API
//!
//! Multi-user annotation endpoints for creating ground truth data for OCR benchmarks.
//! Endpoints:
//! - GET  /datasets           — list datasets with annotation progress
//! - GET  /tasks              — list annotation tasks for a dataset
//! - GET  /tasks/{task_id}    — get single task (marks as in_progress)
//! - POST /tasks/{task_id}/save — save annotation
//! - GET  /tasks/{task_id}/image — stream image bytes

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::path::PathBuf;
use tracing::{error, info};

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct DatasetProgress {
    pub id: String,
    pub name: String,
    pub version: i32,
    pub total_cases: i64,
    pub completed: i64,
    pub in_progress: i64,
    pub pending: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AnnotationTask {
    pub id: String,
    pub case_id: String,
    pub case_id_label: String,
    pub image_path: String,
    pub status: String,
    pub annotator_id: Option<String>,
    pub confidence: Option<String>,
    pub ground_truth: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TaskDetail {
    pub id: String,
    pub case_id: String,
    pub image_path: String,
    pub ground_truth: Option<String>,
    pub status: String,
    pub annotator_id: Option<String>,
    pub confidence: Option<String>,
    pub issues: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskQuery {
    pub dataset_id: String,
    #[serde(default)]
    pub status: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SaveAnnotationRequest {
    pub ground_truth: String,
    pub confidence: String, // "high" | "medium" | "low"
    #[serde(default)]
    pub issues: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub final_submit: bool, // if true, mark completed
}

// ─── Routes ────────────────────────────────────────────────────────────────

pub fn ocr_annotation_routes() -> Router<DbPool> {
    Router::new()
        .route("/datasets", get(list_datasets))
        .route("/tasks", get(list_tasks))
        .route("/tasks/:task_id", get(get_task))
        .route("/tasks/:task_id/save", post(save_annotation))
        .route("/tasks/:task_id/image", get(stream_image))
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// GET /datasets — List datasets with annotation progress
async fn list_datasets(
    State(pool): State<DbPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = &tenant_ctx.tenant_id;

    let datasets: Vec<DatasetProgress> = sqlx::query_as(
        r#"
        SELECT
            d.id,
            d.name,
            d.version,
            COUNT(DISTINCT c.id) as total_cases,
            COALESCE(SUM(CASE WHEN a.status = 'completed' THEN 1 ELSE 0 END), 0) as completed,
            COALESCE(SUM(CASE WHEN a.status = 'in_progress' THEN 1 ELSE 0 END), 0) as in_progress,
            COALESCE(SUM(CASE WHEN a.status = 'pending' THEN 1 ELSE 0 END), 0) as pending
        FROM ocr_eval_datasets d
        LEFT JOIN ocr_eval_cases c ON c.dataset_id = d.id
        LEFT JOIN ocr_annotation_tasks a ON a.case_id = c.id
        WHERE d.tenant_id = ? AND d.is_active = 1
        GROUP BY d.id, d.name, d.version
        ORDER BY d.created_at DESC
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("Failed to list datasets: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{:?}", e)})),
        )
    })?;

    Ok(Json(json!({
        "datasets": datasets,
        "total": datasets.len()
    })))
}

/// GET /tasks — List annotation tasks for a dataset
async fn list_tasks(
    State(pool): State<DbPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Query(params): Query<TaskQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = &tenant_ctx.tenant_id;
    let limit = params.limit.unwrap_or(50).min(100) as i64;
    let offset = params.offset.unwrap_or(0) as i64;

    // Build query based on status filter
    let status_clause = if params.status.is_empty() {
        String::new()
    } else {
        format!("AND a.status = '{}'", params.status)
    };

    let query_str = format!(
        r#"
        SELECT
            a.id,
            a.case_id,
            c.case_id as case_id_label,
            c.image_path,
            a.status,
            a.annotator_id,
            a.confidence,
            a.ground_truth
        FROM ocr_annotation_tasks a
        JOIN ocr_eval_cases c ON c.id = a.case_id
        WHERE a.dataset_id = ? AND a.tenant_id = ? {}
        ORDER BY a.created_at ASC
        LIMIT ? OFFSET ?
        "#,
        status_clause
    );

    let tasks: Vec<AnnotationTask> = sqlx::query_as(&query_str)
        .bind(&params.dataset_id)
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            error!("Failed to list tasks: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{:?}", e)})),
            )
        })?;

    Ok(Json(json!({
        "tasks": tasks,
        "count": tasks.len(),
        "limit": limit,
        "offset": offset
    })))
}

/// GET /tasks/{task_id} — Get task detail (marks as in_progress)
async fn get_task(
    State(pool): State<DbPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Path(task_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = &tenant_ctx.tenant_id;

    // Mark as in_progress if pending
    sqlx::query(
        "UPDATE ocr_annotation_tasks SET status = 'in_progress', started_at = NOW() WHERE id = ? AND tenant_id = ? AND status = 'pending'"
    )
    .bind(&task_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .ok();

    let task: TaskDetail = sqlx::query_as(
        r#"
        SELECT
            a.id,
            a.case_id,
            c.image_path,
            a.ground_truth,
            a.status,
            a.annotator_id,
            a.confidence,
            a.issues,
            a.notes
        FROM ocr_annotation_tasks a
        JOIN ocr_eval_cases c ON c.id = a.case_id
        WHERE a.id = ? AND a.tenant_id = ?
        "#,
    )
    .bind(&task_id)
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        error!("Failed to get task: {:?}", e);
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Task not found"})),
        )
    })?;

    info!("Fetched task {} for annotation", task_id);

    Ok(Json(json!({
        "task": task
    })))
}

/// POST /tasks/{task_id}/save — Save annotation
async fn save_annotation(
    State(pool): State<DbPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Path(task_id): Path<String>,
    Json(payload): Json<SaveAnnotationRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = &tenant_ctx.tenant_id;
    let user_id = &tenant_ctx.user_id;
    let issues_json = serde_json::to_string(&payload.issues).unwrap_or_default();

    // Determine final status
    let final_status = if payload.final_submit { "completed" } else { "in_progress" };
    let completed_at = if payload.final_submit { "NOW()" } else { "NULL" };

    // Update annotation task
    let query = format!(
        r#"
        UPDATE ocr_annotation_tasks
        SET
            ground_truth = ?,
            confidence = ?,
            issues = ?,
            notes = ?,
            annotator_id = ?,
            status = '{}',
            completed_at = {}
        WHERE id = ? AND tenant_id = ?
        "#,
        final_status, completed_at
    );

    sqlx::query(&query)
        .bind(&payload.ground_truth)
        .bind(&payload.confidence)
        .bind(&issues_json)
        .bind(&payload.notes)
        .bind(user_id)
        .bind(&task_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            error!("Failed to save annotation: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("{:?}", e)})),
            )
        })?;

    // If completed, also update ocr_eval_cases.ground_truth
    if payload.final_submit {
        sqlx::query(
            "UPDATE ocr_eval_cases SET ground_truth = ? WHERE id = (SELECT case_id FROM ocr_annotation_tasks WHERE id = ?)"
        )
        .bind(&payload.ground_truth)
        .bind(&task_id)
        .execute(&pool)
        .await
        .ok();
    }

    info!("Saved annotation for task {}, status={}", task_id, final_status);

    Ok(Json(json!({
        "status": "saved",
        "final": payload.final_submit,
        "annotator": user_id
    })))
}

/// GET /tasks/{task_id}/image — Stream image bytes
async fn stream_image(
    State(pool): State<DbPool>,
    Extension(tenant_ctx): Extension<TenantContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let tenant_id = &tenant_ctx.tenant_id;

    // Get image path from task
    let image_path: (String,) = sqlx::query_as(
        "SELECT c.image_path FROM ocr_annotation_tasks a JOIN ocr_eval_cases c ON c.id = a.case_id WHERE a.id = ? AND a.tenant_id = ?"
    )
    .bind(&task_id)
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| (StatusCode::NOT_FOUND, "Task not found".to_string()))?;

    let image_base = std::env::var("IMAGE_BASE_PATH").unwrap_or_else(|_| "/data/images".to_string());
    let full_path = PathBuf::from(&image_base).join(&image_path.0);

    info!("Serving image: {:?}", full_path);

    // Read file
    let data = tokio::fs::read(&full_path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Failed to read image: {}", e)))?;

    // Determine content type from file extension
    let content_type = if image_path.0.ends_with(".png") {
        "image/png"
    } else if image_path.0.ends_with(".gif") {
        "image/gif"
    } else if image_path.0.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    };

    Ok((
        [(CONTENT_TYPE, content_type)],
        data,
    ))
}
