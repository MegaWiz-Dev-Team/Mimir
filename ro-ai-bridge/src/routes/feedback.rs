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
    routing::{get, post, put},
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
    Extension(config): Extension<Arc<Config>>,
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
    let github_result = create_github_issue_for_feedback(
        &config,
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

/// Create a GitHub issue for a feedback report
async fn create_github_issue_for_feedback(
    config: &Config,
    pool: &MySqlPool,
    feedback_id: i64,
    req: &CreateFeedbackRequest,
    system_logs: Option<&str>,
    tenant_id: &str,
    user_id: Option<&str>,
) -> anyhow::Result<(String, i32)> {
    // Check GitHub config
    let github_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| anyhow::anyhow!("GITHUB_TOKEN not configured"))?;

    let github_repo_owner =
        std::env::var("GITHUB_REPO_OWNER").unwrap_or_else(|_| "megacare-dev".to_string());
    let github_repo_name =
        std::env::var("GITHUB_REPO_NAME").unwrap_or_else(|_| "Project-Mimir".to_string());

    // Build issue body
    let body = feedback::build_github_issue_body(req, system_logs, feedback_id, tenant_id, user_id);

    // Determine labels
    let mut labels = vec!["user-reported".to_string()];
    match req.report_type.as_str() {
        "bug" => labels.push("bug".to_string()),
        "feedback" => labels.push("feedback".to_string()),
        "feature" => labels.push("enhancement".to_string()),
        _ => {}
    }
    if let Some(ref p) = req.priority {
        if p == "critical" || p == "high" {
            labels.push(format!("priority:{}", p));
        }
    }

    // Create issue via GitHub API
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/issues",
        github_repo_owner, github_repo_name
    );

    let issue_title = format!("[{}] {}", req.report_type.to_uppercase(), req.title);

    let issue_body = json!({
        "title": issue_title,
        "body": body,
        "labels": labels
    });

    info!("Creating GitHub issue: {}", issue_title);

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", github_token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Project-Mimir-Feedback")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&issue_body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GitHub API request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "GitHub API returned {}: {}",
            status,
            error_body
        ));
    }

    let resp_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse GitHub response: {}", e))?;

    let issue_url = resp_json["html_url"].as_str().unwrap_or("").to_string();
    let issue_number = resp_json["number"].as_i64().unwrap_or(0) as i32;

    // Save GitHub link to DB
    if let Err(e) = feedback::update_github_issue(pool, feedback_id, &issue_url, issue_number).await
    {
        error!("Failed to save GitHub issue link: {}", e);
    }

    Ok((issue_url, issue_number))
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
