//! Agent CRUD operations: list, create, get, update, delete, publish + type definitions.

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tracing::{error, info};
use uuid::Uuid;

use mimir_core_ai::services::db::DbPool;

/// SELECT column list for agent_configs queries.
/// Uses CAST(temperature AS DOUBLE) because MariaDB DECIMAL(3,2) is not compatible with Rust f64.
pub const AGENT_SELECT_COLS: &str = r#"
    id, tenant_id, name, display_name, description, system_prompt, model_id, provider,
    CAST(temperature AS DOUBLE) as temperature, max_tokens, top_k,
    use_rag, use_knowledge_graph, use_pageindex, rag_params, rerank_config,
    tools, mcp_servers, personality_traits, greeting, avatar_url,
    template_id, is_published, api_key, tier, response_mode,
    CAST(created_at AS DATETIME) as created_at, CAST(updated_at AS DATETIME) as updated_at
"#;

// ─── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct AgentConfig {
    pub id: i64,
    pub tenant_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub system_prompt: String,
    pub model_id: String,
    pub provider: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub top_k: Option<i32>,
    pub use_rag: Option<bool>,
    pub use_knowledge_graph: Option<bool>,
    pub use_pageindex: Option<bool>,
    pub rag_params: Option<Value>,
    pub rerank_config: Option<Value>,
    pub tools: Option<Value>,
    pub mcp_servers: Option<Value>,
    pub personality_traits: Option<Value>,
    pub greeting: Option<String>,
    pub avatar_url: Option<String>,
    pub template_id: Option<String>,
    pub is_published: Option<bool>,
    pub api_key: Option<String>,
    pub tier: Option<i32>,
    pub response_mode: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub system_prompt: String,
    pub model_id: String,
    pub provider: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub top_k: Option<i32>,
    pub use_rag: Option<bool>,
    pub use_knowledge_graph: Option<bool>,
    pub use_pageindex: Option<bool>,
    pub rag_params: Option<Value>,
    pub rerank_config: Option<Value>,
    pub tools: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<String>>,
    pub personality_traits: Option<Vec<String>>,
    pub greeting: Option<String>,
    pub avatar_url: Option<String>,
    pub template_id: Option<String>,
    pub tier: Option<i32>,
    pub response_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub top_k: Option<i32>,
    pub use_rag: Option<bool>,
    pub use_knowledge_graph: Option<bool>,
    pub use_pageindex: Option<bool>,
    pub rag_params: Option<Value>,
    pub rerank_config: Option<Value>,
    pub tools: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<String>>,
    pub personality_traits: Option<Vec<String>>,
    pub greeting: Option<String>,
    pub avatar_url: Option<String>,
    pub tier: Option<i32>,
    pub response_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentChatRequest {
    pub message: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentChatResponse {
    pub content: String,
    pub session_id: String,
    pub model_id: String,
    pub provider: String,
    pub latency_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub confidence_score: Option<f64>,
    pub reasoning: Option<String>,
    /// Wave 3 — full structured trace of retrieval + generation for
    /// experiment tracking. Captured per item by eval runner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListAgentsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ConversationListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConversationSession {
    pub session_id: String,
    pub agent_config_id: Option<i64>,
    pub message_count: i64,
    pub first_message_at: Option<chrono::NaiveDateTime>,
    pub last_message_at: Option<chrono::NaiveDateTime>,
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// GET /api/v1/agents — List all agent configs
pub(crate) async fn list_agents(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<ListAgentsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let agents = sqlx::query_as::<_, AgentConfig>(
        &format!("SELECT {} FROM agent_configs WHERE tenant_id = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?", AGENT_SELECT_COLS)
    )
    .bind(tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("Failed to list agents: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agent_configs WHERE tenant_id = ?")
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    Ok(Json(json!({
        "agents": agents,
        "total": total.0,
        "page": page,
        "per_page": per_page
    })))
}

/// POST /api/v1/agents — Create agent config
pub(crate) async fn create_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<CreateAgentRequest>,
) -> Result<(StatusCode, Json<AgentConfig>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let provider = payload.provider.unwrap_or_else(|| "ollama".into());
    let temperature = payload.temperature.unwrap_or(0.7);
    let max_tokens = payload.max_tokens.unwrap_or(2048);
    let top_k = payload.top_k.unwrap_or(5);
    let tier = payload.tier.unwrap_or(2);
    let response_mode = payload.response_mode.unwrap_or_else(|| "streaming".into());
    let use_rag = payload.use_rag.unwrap_or(true);
    let use_kg = payload.use_knowledge_graph.unwrap_or(false);
    let use_pageindex = payload.use_pageindex.unwrap_or(false);
    let rag_params_json = payload.rag_params.as_ref();
    let rerank_config_json = payload.rerank_config.as_ref();
    let tools_json = payload.tools.as_ref().map(|t| json!(t));
    let mcp_servers_json = payload.mcp_servers.as_ref().map(|t| json!(t));
    let traits_json = payload.personality_traits.as_ref().map(|t| json!(t));

    let result = sqlx::query(
        r#"INSERT INTO agent_configs
            (tenant_id, name, display_name, description, system_prompt, model_id, provider,
             temperature, max_tokens, top_k, use_rag, use_knowledge_graph, use_pageindex,
             rag_params, rerank_config,
             tools, mcp_servers, personality_traits, greeting, avatar_url, template_id, tier, response_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(tenant_id)
    .bind(&payload.name)
    .bind(&payload.display_name)
    .bind(&payload.description)
    .bind(&payload.system_prompt)
    .bind(&payload.model_id)
    .bind(&provider)
    .bind(temperature)
    .bind(max_tokens)
    .bind(top_k)
    .bind(use_rag)
    .bind(use_kg)
    .bind(use_pageindex)
    .bind(&rag_params_json)
    .bind(&rerank_config_json)
    .bind(&tools_json)
    .bind(&mcp_servers_json)
    .bind(&traits_json)
    .bind(&payload.greeting)
    .bind(&payload.avatar_url)
    .bind(&payload.template_id)
    .bind(tier)
    .bind(&response_mode)
    .execute(&pool)
    .await
    .map_err(|e| {
        error!("Failed to create agent: {}", e);
        if e.to_string().contains("Duplicate entry") {
            (
                StatusCode::CONFLICT,
                Json(json!({"error": format!("Agent name '{}' already exists", payload.name)})),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        }
    })?;

    let id = result.last_insert_id() as i64;
    info!("Created agent config id={} name={}", id, payload.name);

    let agent = sqlx::query_as::<_, AgentConfig>(&format!(
        "SELECT {} FROM agent_configs WHERE id = ?",
        AGENT_SELECT_COLS
    ))
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::CREATED, Json(agent)))
}

/// GET /api/v1/agents/:id — Get agent config by ID
pub(crate) async fn get_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<AgentConfig>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let agent = sqlx::query_as::<_, AgentConfig>(&format!(
        "SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?",
        AGENT_SELECT_COLS
    ))
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    agent.map(Json).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Agent not found"})),
        )
    })
}

/// PUT /api/v1/agents/:id — Update agent config
pub(crate) async fn update_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateAgentRequest>,
) -> Result<Json<AgentConfig>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Verify agent exists
    let existing = sqlx::query_as::<_, AgentConfig>(&format!(
        "SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?",
        AGENT_SELECT_COLS
    ))
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    if existing.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Agent not found"})),
        ));
    }

    let existing = existing.unwrap();
    let display_name = payload
        .display_name
        .unwrap_or(existing.display_name.unwrap_or_default());
    let description = payload.description.or(existing.description);
    let system_prompt = payload.system_prompt.unwrap_or(existing.system_prompt);
    let model_id = payload.model_id.unwrap_or(existing.model_id);
    let provider = payload.provider.unwrap_or(existing.provider);
    let temperature = payload.temperature.unwrap_or(0.7);
    let max_tokens = payload
        .max_tokens
        .unwrap_or(existing.max_tokens.unwrap_or(2048));
    let top_k = payload.top_k.unwrap_or(existing.top_k.unwrap_or(5));
    let tier = payload.tier.unwrap_or(existing.tier.unwrap_or(2));
    let response_mode = payload.response_mode.clone().unwrap_or_else(|| {
        existing
            .response_mode
            .clone()
            .unwrap_or_else(|| "streaming".into())
    });
    let use_rag = payload.use_rag.unwrap_or(existing.use_rag.unwrap_or(true));
    let use_kg = payload
        .use_knowledge_graph
        .unwrap_or(existing.use_knowledge_graph.unwrap_or(false));
    let use_pageindex = payload
        .use_pageindex
        .unwrap_or(existing.use_pageindex.unwrap_or(false));
    let rag_params_json = payload.rag_params.or(existing.rag_params);
    let rerank_config_json = payload.rerank_config.or(existing.rerank_config);
    let tools_json = payload.tools.map(|t| json!(t)).or(existing.tools);
    let mcp_servers_json = payload.mcp_servers.map(|t| json!(t)).or(existing.mcp_servers);
    let traits_json = payload
        .personality_traits
        .map(|t| json!(t))
        .or(existing.personality_traits);
    let greeting = payload.greeting.or(existing.greeting);
    let avatar_url = payload.avatar_url.or(existing.avatar_url);

    sqlx::query(
        r#"UPDATE agent_configs SET
            display_name = ?, description = ?, system_prompt = ?, model_id = ?, provider = ?,
            temperature = ?, max_tokens = ?, top_k = ?, use_rag = ?, use_knowledge_graph = ?,
            use_pageindex = ?, rag_params = ?, rerank_config = ?,
            tools = ?, mcp_servers = ?, personality_traits = ?, greeting = ?, avatar_url = ?,
            tier = ?, response_mode = ?
        WHERE id = ? AND tenant_id = ?"#,
    )
    .bind(&display_name)
    .bind(&description)
    .bind(&system_prompt)
    .bind(&model_id)
    .bind(&provider)
    .bind(temperature)
    .bind(max_tokens)
    .bind(top_k)
    .bind(use_rag)
    .bind(use_kg)
    .bind(use_pageindex)
    .bind(&rag_params_json)
    .bind(&rerank_config_json)
    .bind(&tools_json)
    .bind(&mcp_servers_json)
    .bind(&traits_json)
    .bind(&greeting)
    .bind(&avatar_url)
    .bind(tier)
    .bind(&response_mode)
    .bind(id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let updated = sqlx::query_as::<_, AgentConfig>(&format!(
        "SELECT {} FROM agent_configs WHERE id = ?",
        AGENT_SELECT_COLS
    ))
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    info!("Updated agent config id={}", id);
    Ok(Json(updated))
}

/// DELETE /api/v1/agents/:id — Delete agent config
pub(crate) async fn delete_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let result = sqlx::query("DELETE FROM agent_configs WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Agent not found"})),
        ));
    }

    info!("Deleted agent config id={}", id);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/agents/:id/publish — Generate API key, set published
pub(crate) async fn publish_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let api_key = format!("ak_{}", Uuid::new_v4().to_string().replace("-", ""));

    let result = sqlx::query(
        "UPDATE agent_configs SET is_published = TRUE, api_key = ? WHERE id = ? AND tenant_id = ?",
    )
    .bind(&api_key)
    .bind(id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Agent not found"})),
        ));
    }

    info!("Published agent id={}, api_key generated", id);
    Ok(Json(json!({
        "id": id,
        "is_published": true,
        "api_key": api_key
    })))
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// TC_MIG_07: API key format is correct
    #[test]
    fn test_api_key_format() {
        let api_key = format!("ak_{}", Uuid::new_v4().to_string().replace("-", ""));
        assert!(api_key.starts_with("ak_"));
        assert_eq!(api_key.len(), 35); // "ak_" + 32 hex chars
    }

    /// TC_MIG_08: AgentConfig struct has tier and response_mode fields
    #[test]
    fn test_agent_config_has_new_fields() {
        let config = AgentConfig {
            id: 1,
            tenant_id: "test".into(),
            name: "test_agent".into(),
            display_name: Some("Test Agent".into()),
            description: None,
            system_prompt: "You are a test agent".into(),
            model_id: "test-model".into(),
            provider: "heimdall".into(),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            top_k: Some(5),
            use_rag: Some(true),
            use_knowledge_graph: Some(false),
            tools: None,
            mcp_servers: None,
            personality_traits: None,
            greeting: Some("Hello".into()),
            avatar_url: None,
            template_id: Some("npc_guide".into()),
            is_published: Some(false),
            api_key: None,
            tier: Some(1),
            response_mode: Some("streaming".into()),
            use_pageindex: Some(false),
            rag_params: None,
            rerank_config: None,
            created_at: None,
            updated_at: None,
        };
        assert_eq!(config.tier, Some(1));
        assert_eq!(config.response_mode, Some("streaming".into()));
    }

    /// TC_MIG_09: CreateAgentRequest defaults for tier and response_mode
    #[test]
    fn test_create_request_defaults() {
        let req: CreateAgentRequest = serde_json::from_str(
            r#"{
            "name": "test",
            "system_prompt": "test prompt",
            "model_id": "test-model"
        }"#,
        )
        .unwrap();
        assert_eq!(
            req.tier, None,
            "Tier should default to None (resolved to 2 in handler)"
        );
        assert_eq!(
            req.response_mode, None,
            "Response mode should default to None"
        );
    }
}
