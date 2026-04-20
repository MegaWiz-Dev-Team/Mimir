//! Feedback & Bug Report Routes (Issue #153)
//!
//! - POST /api/v1/feedback           — submit feedback + auto-create GitHub issue
//! - GET  /api/v1/feedback           — list reports (paginated)
//! - PUT  /api/v1/feedback/:id       — update status (admin)

use crate::config::Config;
use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Extension, Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{post, put},
    Router,
};
use mimir_core_ai::services::feedback::{
    self, CreateFeedbackRequest, FeedbackFilter, UpdateFeedbackRequest,
};
use serde_json::json;
use sqlx::MySqlPool;
use std::sync::Arc;
use tracing::{error, info, warn};

pub fn feedback_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/", post(submit_feedback).get(list_feedback))
        .route("/{id}", put(update_feedback))
}

/// POST /feedback — submit a new feedback/bug report + auto-create GitHub issue
async fn submit_feedback(
    headers: HeaderMap,
    Extension(_config): Extension<Arc<Config>>,
    State(pool): State<MySqlPool>,
    Json(req): Json<CreateFeedbackRequest>,
) -> impl IntoResponse {
    // TODO: extract tenant_id/user_id from JWT when auth middleware is applied
    let tenant_id = extract_tenant_id(&headers);
    let user_id: Option<&str> = None;

    // 1. Collect system logs
    let system_logs = feedback::collect_system_logs(&pool, tenant_id).await;

    // 2. Save to DB
    let feedback_id = match feedback::create_feedback(
        &pool,
        tenant_id,
        user_id,
        &req,
        Some(&system_logs),
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to create feedback: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to create feedback: {}", e)
                })),
            )
                .into_response();
        }
    };

    // 3. Auto-create GitHub issue (async, non-blocking)
    let github_result = feedback::create_github_issue_for_feedback(
        &pool,
        feedback_id,
        &req,
        Some(&system_logs),
        tenant_id,
        user_id,
    )
    .await;

    let mut response = json!({
        "success": true,
        "id": feedback_id
    });

    match github_result {
        Ok((issue_url, issue_number)) => {
            response["github_issue_url"] = json!(issue_url);
            response["github_issue_number"] = json!(issue_number);
            info!(
                feedback_id,
                issue_number, "✅ Feedback submitted with GitHub issue"
            );
        }
        Err(e) => {
            warn!("GitHub issue creation skipped: {}", e);
            response["github_issue_url"] = json!(null);
            response["github_issue_note"] = json!(format!("GitHub issue not created: {}", e));
        }
    }

    (StatusCode::CREATED, Json(response)).into_response()
}

/// GET /feedback — list feedback reports with filters
async fn list_feedback(
    headers: HeaderMap,
    State(pool): State<MySqlPool>,
    Query(filter): Query<FeedbackFilter>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);

    match feedback::list_feedback(&pool, tenant_id, &filter).await {
        Ok(reports) => (
            StatusCode::OK,
            Json(json!({
                "data": reports,
                "count": reports.len()
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to list feedback: {}", e)
            })),
        )
            .into_response(),
    }
}

/// PUT /feedback/:id — update feedback status/resolution
async fn update_feedback(
    headers: HeaderMap,
    State(pool): State<MySqlPool>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateFeedbackRequest>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);

    match feedback::update_feedback(&pool, id, tenant_id, &req).await {
        Ok(true) => (StatusCode::OK, Json(json!({"success": true}))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Report not found or no changes"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to update feedback: {}", e)
            })),
        )
            .into_response(),
    }
}
