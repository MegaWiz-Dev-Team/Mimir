//! Feedback & Bug Report Routes (Issue #153)
//!
//! - POST /api/v1/feedback           — submit feedback
//! - GET  /api/v1/feedback           — list reports (paginated)
//! - PUT  /api/v1/feedback/:id       — update status (admin)

use axum::{
    Router,
    routing::{get, post, put},
    extract::{Path, State, Query, Json},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::MySqlPool;
use serde_json::json;
use mimir_core_ai::services::feedback::{
    self, CreateFeedbackRequest, UpdateFeedbackRequest, FeedbackFilter,
};

pub fn feedback_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/", post(submit_feedback).get(list_feedback))
        .route("/{id}", put(update_feedback))
}

/// POST /feedback — submit a new feedback/bug report
async fn submit_feedback(
    State(pool): State<MySqlPool>,
    Json(req): Json<CreateFeedbackRequest>,
) -> impl IntoResponse {
    // TODO: extract tenant_id/user_id from JWT when auth middleware is applied
    let tenant_id = "default_tenant";
    let user_id: Option<&str> = None;

    match feedback::create_feedback(&pool, tenant_id, user_id, &req).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({
            "success": true,
            "id": id
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("Failed to create feedback: {}", e)
        }))).into_response(),
    }
}

/// GET /feedback — list feedback reports with filters
async fn list_feedback(
    State(pool): State<MySqlPool>,
    Query(filter): Query<FeedbackFilter>,
) -> impl IntoResponse {
    let tenant_id = "default_tenant";

    match feedback::list_feedback(&pool, tenant_id, &filter).await {
        Ok(reports) => (StatusCode::OK, Json(json!({
            "data": reports,
            "count": reports.len()
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("Failed to list feedback: {}", e)
        }))).into_response(),
    }
}

/// PUT /feedback/:id — update feedback status/resolution
async fn update_feedback(
    State(pool): State<MySqlPool>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateFeedbackRequest>,
) -> impl IntoResponse {
    let tenant_id = "default_tenant";

    match feedback::update_feedback(&pool, id, tenant_id, &req).await {
        Ok(true) => (StatusCode::OK, Json(json!({"success": true}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "Report not found or no changes"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("Failed to update feedback: {}", e)
        }))).into_response(),
    }
}
