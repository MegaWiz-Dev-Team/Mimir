//! Feedback & Bug Report Service (Issue #153)
//!
//! CRUD operations for feedback_reports table.

use sqlx::MySqlPool;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FeedbackReport {
    pub id: i64,
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub report_type: String,
    pub title: String,
    pub description: Option<String>,
    pub page_url: Option<String>,
    pub browser_info: Option<serde_json::Value>,
    pub screenshot_url: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub resolution: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFeedbackRequest {
    pub report_type: String,
    pub title: String,
    pub description: Option<String>,
    pub page_url: Option<String>,
    pub browser_info: Option<serde_json::Value>,
    pub priority: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeedbackRequest {
    pub status: Option<String>,
    pub resolution: Option<String>,
    pub priority: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackFilter {
    pub report_type: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// Create a new feedback report
pub async fn create_feedback(
    pool: &MySqlPool,
    tenant_id: &str,
    user_id: Option<&str>,
    req: &CreateFeedbackRequest,
) -> Result<i64> {
    let result = sqlx::query(
        r#"INSERT INTO feedback_reports 
           (tenant_id, user_id, report_type, title, description, page_url, browser_info, priority) 
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(&req.report_type)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.page_url)
    .bind(&req.browser_info)
    .bind(req.priority.as_deref().unwrap_or("medium"))
    .execute(pool)
    .await?;

    info!(
        tenant_id = tenant_id,
        report_type = %req.report_type,
        title = %req.title,
        "📝 Feedback report created"
    );

    Ok(result.last_insert_id() as i64)
}

/// List feedback reports with filtering and pagination
pub async fn list_feedback(
    pool: &MySqlPool,
    tenant_id: &str,
    filter: &FeedbackFilter,
) -> Result<Vec<FeedbackReport>> {
    let page = filter.page.unwrap_or(1).max(1);
    let per_page = filter.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let mut query = String::from(
        "SELECT * FROM feedback_reports WHERE tenant_id = ?"
    );
    let mut params: Vec<String> = vec![tenant_id.to_string()];

    if let Some(ref rt) = filter.report_type {
        query.push_str(" AND report_type = ?");
        params.push(rt.clone());
    }
    if let Some(ref st) = filter.status {
        query.push_str(" AND status = ?");
        params.push(st.clone());
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    // Use dynamic query building
    let mut q = sqlx::query_as::<_, FeedbackReport>(&query);
    for p in &params {
        q = q.bind(p);
    }
    q = q.bind(per_page).bind(offset);

    let reports = q.fetch_all(pool).await?;
    Ok(reports)
}

/// Update feedback report status/resolution
pub async fn update_feedback(
    pool: &MySqlPool,
    feedback_id: i64,
    tenant_id: &str,
    req: &UpdateFeedbackRequest,
) -> Result<bool> {
    let mut sets = Vec::new();

    if req.status.is_some() {
        sets.push("status = ?");
    }
    if req.resolution.is_some() {
        sets.push("resolution = ?");
    }
    if req.priority.is_some() {
        sets.push("priority = ?");
    }

    if sets.is_empty() {
        return Ok(false);
    }

    let query = format!(
        "UPDATE feedback_reports SET {} WHERE id = ? AND tenant_id = ?",
        sets.join(", ")
    );

    let mut q = sqlx::query(&query);
    if let Some(ref s) = req.status {
        q = q.bind(s);
    }
    if let Some(ref r) = req.resolution {
        q = q.bind(r);
    }
    if let Some(ref p) = req.priority {
        q = q.bind(p);
    }
    q = q.bind(feedback_id).bind(tenant_id);

    let result = q.execute(pool).await?;
    Ok(result.rows_affected() > 0)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-014r: submit_feedback — validates request fields
    // ========================================
    #[test]
    fn test_create_feedback_request_deserialization() {
        let json = r#"{
            "report_type": "bug",
            "title": "Login button broken",
            "description": "Cannot click login on mobile",
            "page_url": "/login",
            "browser_info": {"userAgent": "Mozilla/5.0", "viewport": "375x812"},
            "priority": "high"
        }"#;

        let req: CreateFeedbackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.report_type, "bug");
        assert_eq!(req.title, "Login button broken");
        assert_eq!(req.priority.as_deref(), Some("high"));
        assert!(req.browser_info.is_some());
    }

    // ========================================
    // UT-014u: auto_capture — includes page_url and browser_info
    // ========================================
    #[test]
    fn test_auto_capture_fields() {
        let json = r#"{
            "report_type": "feedback",
            "title": "Great feature!",
            "page_url": "/agents/42",
            "browser_info": {"userAgent": "Chrome/120", "viewport": "1920x1080"}
        }"#;

        let req: CreateFeedbackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.page_url.as_deref(), Some("/agents/42"));
        let info = req.browser_info.unwrap();
        assert_eq!(info["userAgent"], "Chrome/120");
        assert_eq!(info["viewport"], "1920x1080");
    }
}
