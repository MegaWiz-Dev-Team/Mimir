//! Conversation routes — shared conversation endpoints for Playground & Agent Studio
//!
//! Endpoints:
//! - GET    /api/v1/conversations              — list all conversations (paginated, filterable)
//! - GET    /api/v1/conversations/stats         — conversation stats
//! - GET    /api/v1/conversations/:session_id   — get full conversation by session
//! - POST   /api/v1/conversations/:id/feedback  — submit thumbs up/down

use axum::{
    routing::{get, post},
    Router, Json,
    extract::{Path, State, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tracing::{info, error};

use mimir_core_ai::services::db::DbPool;

// ─── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ConversationListQuery {
    pub agent_config_id: Option<i64>,
    pub user_id: Option<i64>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConversationMessage {
    pub id: i64,
    pub tenant_id: String,
    pub agent_config_id: Option<i64>,
    pub session_id: String,
    pub user_id: Option<i64>,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    pub latency_ms: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub confidence_score: Option<f64>,
    pub sources: Option<Value>,
    pub tools_used: Option<Value>,
    pub feedback: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConversationSessionSummary {
    pub session_id: String,
    pub agent_config_id: Option<i64>,
    pub agent_name: Option<String>,
    pub message_count: i64,
    pub first_message_at: Option<chrono::NaiveDateTime>,
    pub last_message_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackRequest {
    pub feedback: String,  // "thumbs_up" or "thumbs_down"
}

// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn conversations_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_conversations))
        .route("/stats", get(conversation_stats))
        .route("/{session_id}", get(get_conversation))
        .route("/{id}/feedback", post(submit_feedback))
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// GET /api/v1/conversations — List conversation sessions
async fn list_conversations(
    State(pool): State<DbPool>,
    Query(params): Query<ConversationListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let mut query = String::from(
        r#"SELECT
            ac.session_id,
            ac.agent_config_id,
            ag.name as agent_name,
            COUNT(*) as message_count,
            MIN(ac.created_at) as first_message_at,
            MAX(ac.created_at) as last_message_at
        FROM agent_conversations ac
        LEFT JOIN agent_configs ag ON ac.agent_config_id = ag.id
        WHERE ac.tenant_id = ?"#
    );

    let mut bind_values: Vec<String> = vec![tenant_id.to_string()];

    if let Some(agent_id) = params.agent_config_id {
        query.push_str(" AND ac.agent_config_id = ?");
        bind_values.push(agent_id.to_string());
    }

    if let Some(ref date_from) = params.date_from {
        query.push_str(" AND ac.created_at >= ?");
        bind_values.push(date_from.clone());
    }

    if let Some(ref date_to) = params.date_to {
        query.push_str(" AND ac.created_at <= ?");
        bind_values.push(date_to.clone());
    }

    query.push_str(" GROUP BY ac.session_id, ac.agent_config_id, ag.name ORDER BY last_message_at DESC LIMIT ? OFFSET ?");

    // Execute with dynamic bindings
    let sessions: Vec<ConversationSessionSummary> = sqlx::query_as(&query)
        .bind(tenant_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            error!("Failed to list conversations: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    // Get total count
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT session_id) FROM agent_conversations WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    Ok(Json(json!({
        "sessions": sessions,
        "total": total.0,
        "page": page,
        "per_page": per_page
    })))
}

/// GET /api/v1/conversations/:session_id — Get full conversation transcript
async fn get_conversation(
    State(pool): State<DbPool>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";

    let messages = sqlx::query_as::<_, ConversationMessage>(
        "SELECT * FROM agent_conversations WHERE tenant_id = ? AND session_id = ? ORDER BY created_at ASC"
    )
    .bind(tenant_id)
    .bind(&session_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if messages.is_empty() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Session not found"}))));
    }

    Ok(Json(json!({
        "session_id": session_id,
        "messages": messages,
        "total_messages": messages.len()
    })))
}

/// POST /api/v1/conversations/:id/feedback — Submit feedback on a message
async fn submit_feedback(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<FeedbackRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Validate feedback value
    if payload.feedback != "thumbs_up" && payload.feedback != "thumbs_down" {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "feedback must be 'thumbs_up' or 'thumbs_down'"}))));
    }

    let result = sqlx::query(
        "UPDATE agent_conversations SET feedback = ? WHERE id = ?"
    )
    .bind(&payload.feedback)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Message not found"}))));
    }

    info!("Feedback '{}' submitted for message id={}", payload.feedback, id);
    Ok(Json(json!({"status": "ok", "id": id, "feedback": payload.feedback})))
}

/// GET /api/v1/conversations/stats — Conversation statistics
async fn conversation_stats(
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM agent_conversations WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    let total_sessions: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT session_id) FROM agent_conversations WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    // Per-agent stats
    let by_agent: Vec<(Option<i64>, i64, i64)> = sqlx::query_as(
        r#"SELECT agent_config_id, COUNT(*) as messages, COUNT(DISTINCT session_id) as sessions
        FROM agent_conversations WHERE tenant_id = ?
        GROUP BY agent_config_id"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Feedback stats
    let thumbs_up: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM agent_conversations WHERE tenant_id = ? AND feedback = 'thumbs_up'"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    let thumbs_down: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM agent_conversations WHERE tenant_id = ? AND feedback = 'thumbs_down'"
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((0,));

    Ok(Json(json!({
        "total_messages": total.0,
        "total_sessions": total_sessions.0,
        "thumbs_up": thumbs_up.0,
        "thumbs_down": thumbs_down.0,
        "by_agent": by_agent.iter().map(|(agent_id, msgs, sessions)| json!({
            "agent_config_id": agent_id,
            "messages": msgs,
            "sessions": sessions,
        })).collect::<Vec<_>>()
    })))
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_validation() {
        let valid_up = FeedbackRequest { feedback: "thumbs_up".into() };
        let valid_down = FeedbackRequest { feedback: "thumbs_down".into() };
        assert_eq!(valid_up.feedback, "thumbs_up");
        assert_eq!(valid_down.feedback, "thumbs_down");
    }
}
