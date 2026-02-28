//! Feedback & Bug Report Service (Issue #153)
//!
//! CRUD operations for feedback_reports table.
//! Auto-creates GitHub issues when feedback is submitted.

use sqlx::MySqlPool;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tracing::{info, error, warn};

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
    pub github_issue_url: Option<String>,
    pub github_issue_number: Option<i32>,
    pub system_logs: Option<String>,
    pub client_logs: Option<String>,
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
    /// Client-side logs (console errors, network failures, etc.)
    pub client_logs: Option<serde_json::Value>,
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

/// Create a new feedback report (stores in DB, returns feedback_id)
pub async fn create_feedback(
    pool: &MySqlPool,
    tenant_id: &str,
    user_id: Option<&str>,
    req: &CreateFeedbackRequest,
    system_logs: Option<&str>,
) -> Result<i64> {
    let client_logs_str = req.client_logs.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    let result = sqlx::query(
        r#"INSERT INTO feedback_reports 
           (tenant_id, user_id, report_type, title, description, page_url, browser_info, priority, client_logs, system_logs) 
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(&req.report_type)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.page_url)
    .bind(&req.browser_info)
    .bind(req.priority.as_deref().unwrap_or("medium"))
    .bind(&client_logs_str)
    .bind(system_logs)
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

/// Update the GitHub issue link on a feedback report
pub async fn update_github_issue(
    pool: &MySqlPool,
    feedback_id: i64,
    issue_url: &str,
    issue_number: i32,
) -> Result<()> {
    sqlx::query(
        "UPDATE feedback_reports SET github_issue_url = ?, github_issue_number = ? WHERE id = ?"
    )
    .bind(issue_url)
    .bind(issue_number)
    .bind(feedback_id)
    .execute(pool)
    .await?;

    info!(feedback_id, issue_number, "🔗 Linked feedback to GitHub issue");
    Ok(())
}

/// Collect recent system logs for a tenant (last 10 LLM usage entries + errors)
pub async fn collect_system_logs(pool: &MySqlPool, tenant_id: &str) -> String {
    let mut logs = serde_json::Map::new();

    // Recent LLM usage (last 5 entries)
    match sqlx::query_as::<_, (String, String, i32, String, Option<String>)>(
        "SELECT model, endpoint, total_tokens, status, error_message FROM llm_usage_logs WHERE tenant_id = ? ORDER BY created_at DESC LIMIT 5"
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    {
        Ok(entries) => {
            let usage: Vec<serde_json::Value> = entries.iter().map(|e| {
                serde_json::json!({
                    "model": e.0,
                    "endpoint": e.1,
                    "tokens": e.2,
                    "status": e.3,
                    "error": e.4
                })
            }).collect();
            logs.insert("recent_llm_usage".to_string(), serde_json::Value::Array(usage));
        }
        Err(e) => {
            warn!("Failed to collect LLM usage logs: {}", e);
        }
    }

    // Recent errors from data sources
    match sqlx::query_as::<_, (i64, String, Option<String>)>(
        "SELECT id, name, last_sync_status FROM data_sources WHERE tenant_id = ? AND last_sync_status IN ('FAILED', 'OCR_FAILED', 'ERROR') ORDER BY updated_at DESC LIMIT 5"
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    {
        Ok(entries) => {
            let errors: Vec<serde_json::Value> = entries.iter().map(|e| {
                serde_json::json!({
                    "source_id": e.0,
                    "name": e.1,
                    "status": e.2
                })
            }).collect();
            logs.insert("failed_sources".to_string(), serde_json::Value::Array(errors));
        }
        Err(e) => {
            warn!("Failed to collect source error logs: {}", e);
        }
    }

    // System info
    logs.insert("backend_version".to_string(), serde_json::json!(env!("CARGO_PKG_VERSION")));
    logs.insert("timestamp".to_string(), serde_json::json!(chrono::Utc::now().to_rfc3339()));

    serde_json::to_string_pretty(&logs).unwrap_or_default()
}

/// Build a GitHub Issue body from a feedback report
pub fn build_github_issue_body(
    req: &CreateFeedbackRequest,
    system_logs: Option<&str>,
    feedback_id: i64,
    tenant_id: &str,
    user_id: Option<&str>,
) -> String {
    let mut body = String::new();

    // Type badge
    let type_emoji = match req.report_type.as_str() {
        "bug" => "🐛",
        "feedback" => "💡",
        "feature" => "✨",
        _ => "📝",
    };

    body.push_str(&format!("## {} {} Report (ID: #{})\n\n", type_emoji, req.report_type, feedback_id));

    // Tenant & User info
    body.push_str(&format!("**Tenant:** `{}`\n", tenant_id));
    if let Some(uid) = user_id {
        body.push_str(&format!("**User:** `{}`\n", uid));
    }
    body.push('\n');

    // Priority
    if let Some(ref p) = req.priority {
        let priority_badge = match p.as_str() {
            "critical" => "🔴 Critical",
            "high" => "🟠 High",
            "medium" => "🟡 Medium",
            "low" => "🟢 Low",
            _ => p.as_str(),
        };
        body.push_str(&format!("**Priority:** {}\n\n", priority_badge));
    }

    // Description
    if let Some(ref desc) = req.description {
        body.push_str("### Description\n\n");
        body.push_str(desc);
        body.push_str("\n\n");
    }

    // Page URL
    if let Some(ref url) = req.page_url {
        body.push_str(&format!("**Page:** `{}`\n\n", url));
    }

    // Browser info
    if let Some(ref info) = req.browser_info {
        body.push_str("### Browser Info\n\n");
        body.push_str("```json\n");
        body.push_str(&serde_json::to_string_pretty(info).unwrap_or_default());
        body.push_str("\n```\n\n");
    }

    // Client logs
    if let Some(ref logs) = req.client_logs {
        body.push_str("<details>\n<summary>📋 Client Logs</summary>\n\n");
        body.push_str("```json\n");
        body.push_str(&serde_json::to_string_pretty(logs).unwrap_or_default());
        body.push_str("\n```\n\n");
        body.push_str("</details>\n\n");
    }

    // System logs
    if let Some(logs) = system_logs {
        body.push_str("<details>\n<summary>🖥️ System Logs</summary>\n\n");
        body.push_str("```json\n");
        body.push_str(logs);
        body.push_str("\n```\n\n");
        body.push_str("</details>\n\n");
    }

    body.push_str("---\n*Auto-generated by Project Mimir feedback system*\n");
    body
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

    // UT-014r: submit_feedback — validates request fields
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

    // UT-014u: auto_capture — includes page_url and browser_info
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

    // UT-014s: client_logs field deserialization
    #[test]
    fn test_client_logs_deserialization() {
        let json = r#"{
            "report_type": "bug",
            "title": "Page crash",
            "client_logs": {
                "console_errors": ["TypeError: undefined is not a function"],
                "network_errors": [{"url": "/api/v1/sources", "status": 500}],
                "recent_actions": ["clicked Sources tab", "scrolled down"]
            }
        }"#;

        let req: CreateFeedbackRequest = serde_json::from_str(json).unwrap();
        assert!(req.client_logs.is_some());
        let logs = req.client_logs.unwrap();
        assert_eq!(logs["console_errors"].as_array().unwrap().len(), 1);
        assert_eq!(logs["network_errors"].as_array().unwrap().len(), 1);
    }

    // UT-014t: GitHub issue body builder
    #[test]
    fn test_build_github_issue_body() {
        let req = CreateFeedbackRequest {
            report_type: "bug".to_string(),
            title: "Upload fails".to_string(),
            description: Some("File upload returns 500".to_string()),
            page_url: Some("/sources".to_string()),
            browser_info: Some(serde_json::json!({"userAgent": "Chrome/120"})),
            priority: Some("high".to_string()),
            client_logs: Some(serde_json::json!({"errors": ["500 error"]})),
        };

        let body = build_github_issue_body(&req, Some("{\"version\": \"0.1.0\"}"), 42, "tenant_abc", Some("user@example.com"));

        assert!(body.contains("🐛"));
        assert!(body.contains("bug Report"));
        assert!(body.contains("ID: #42"));
        assert!(body.contains("🟠 High"));
        assert!(body.contains("File upload returns 500"));
        assert!(body.contains("Page:** `/sources`"));
        assert!(body.contains("Client Logs"));
        assert!(body.contains("System Logs"));
        assert!(body.contains("Tenant:** `tenant_abc`"));
        assert!(body.contains("User:** `user@example.com`"));
        assert!(body.contains("Auto-generated by Project Mimir"));
    }

    // UT-014: issue body with minimal fields
    #[test]
    fn test_build_github_issue_body_minimal() {
        let req = CreateFeedbackRequest {
            report_type: "feedback".to_string(),
            title: "Nice feature".to_string(),
            description: None,
            page_url: None,
            browser_info: None,
            priority: None,
            client_logs: None,
        };

        let body = build_github_issue_body(&req, None, 1, "default_tenant", None);
        assert!(body.contains("💡"));
        assert!(body.contains("feedback Report"));
        assert!(!body.contains("Client Logs"));
        assert!(!body.contains("System Logs"));
        assert!(body.contains("Tenant:** `default_tenant`"));
        assert!(!body.contains("User:**"));
    }
}
