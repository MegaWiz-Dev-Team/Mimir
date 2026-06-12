//! Agent CRUD operations: list, create, get, update, delete, publish + type definitions.

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use mimir_core_ai::middleware::tenant::TenantContext;
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
    /// Hard-excluded from every JSON response. Use the dedicated
    /// `POST /api/v1/agents/{id}/publish` endpoint to (re)generate.
    /// Kept on the struct so DB round-trips work; `serde(skip)` ensures
    /// no caller — internal UI or external — ever sees it.
    #[serde(skip_serializing)]
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

// ─── Public response shaping ────────────────────────────────────────────────────
//
// The handlers below reshape the raw `AgentConfig` row into a response with
// (a) a nested `capabilities` envelope and (b) a whitelisted `rag_params`
// projection. `api_key` is already dropped via `#[serde(skip_serializing)]`
// on the struct field.

/// Build the capabilities sub-object exposed on both list and detail.
fn capabilities_json(agent: &AgentConfig) -> Value {
    json!({
        "model_id": agent.model_id,
        "provider": agent.provider,
        "temperature": agent.temperature,
        "max_tokens": agent.max_tokens,
        "top_k": agent.top_k,
        "use_rag": agent.use_rag.unwrap_or(false),
        "use_knowledge_graph": agent.use_knowledge_graph.unwrap_or(false),
        "use_pageindex": agent.use_pageindex.unwrap_or(false),
        "tools": json_array_or_empty(agent.tools.as_ref()),
        "mcp_servers": json_array_or_empty(agent.mcp_servers.as_ref()),
    })
}

/// Coerce nullable JSON array column into `[]` (never `null` or non-array).
fn json_array_or_empty(v: Option<&Value>) -> Value {
    match v {
        Some(val) if val.is_array() => val.clone(),
        _ => Value::Array(vec![]),
    }
}

/// `rag_params` is a free-form JSON column. Project only the three keys the
/// engines actually read (`limit`, `alpha`, `output_format`). Any other key
/// stashed by operators or future code is dropped before it leaves Mimir.
fn rag_params_whitelist(v: Option<&Value>) -> Value {
    const ALLOWED: &[&str] = &["limit", "alpha", "output_format"];
    let Some(obj) = v.and_then(|v| v.as_object()) else {
        return Value::Null;
    };
    let mut out = serde_json::Map::new();
    for key in ALLOWED {
        if let Some(val) = obj.get(*key) {
            out.insert((*key).to_string(), val.clone());
        }
    }
    if out.is_empty() {
        Value::Null
    } else {
        Value::Object(out)
    }
}

/// Build the list-response envelope for one agent: legacy top-level fields
/// (id/name/display_name/description/avatar_url/is_published/model_id) PLUS
/// the new `capabilities` nested object. No `system_prompt`, no `greeting`,
/// no `personality_traits` — those land on the detail endpoint only.
fn agent_list_envelope(agent: &AgentConfig) -> Value {
    json!({
        "id": agent.id,
        "name": agent.name,
        "display_name": agent.display_name,
        "description": agent.description,
        "avatar_url": agent.avatar_url,
        "is_published": agent.is_published.unwrap_or(false),
        "model_id": agent.model_id,
        "capabilities": capabilities_json(agent),
    })
}

/// Build the detail-response envelope: list shape + persona + whitelisted
/// `rag_params` + timestamps.
fn agent_detail_envelope(agent: &AgentConfig) -> Value {
    json!({
        "id": agent.id,
        "name": agent.name,
        "display_name": agent.display_name,
        "description": agent.description,
        "avatar_url": agent.avatar_url,
        "greeting": agent.greeting,
        "is_published": agent.is_published.unwrap_or(false),
        "model_id": agent.model_id,
        "system_prompt": agent.system_prompt,
        "personality_traits": json_array_or_empty(agent.personality_traits.as_ref()),
        "created_at": agent.created_at.map(|d| d.and_utc().to_rfc3339()),
        "updated_at": agent.updated_at.map(|d| d.and_utc().to_rfc3339()),
        "capabilities": capabilities_json(agent),
        "rag_params": rag_params_whitelist(agent.rag_params.as_ref()),
    })
}

/// Resolve tenant from the request. Prefers the `TenantContext` set by
/// `flexible_tenant_middleware` (JWT or header fallback). Falls back to the
/// legacy header-only `extract_tenant_id` for routes that haven't migrated
/// yet — the new public read endpoints always have a context.
fn resolved_tenant(ctx: Option<&TenantContext>, headers: &HeaderMap) -> String {
    if let Some(c) = ctx {
        return c.tenant_id.clone();
    }
    extract_tenant_id(headers).to_string()
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// GET /api/v1/agents — List agents available to the calling tenant.
///
/// Returns the list-shape envelope (legacy top-level fields + `capabilities`)
/// without persona or secrets. See [`agent_list_envelope`] for the exact
/// field set; see `docs/api/agents.md` for the contract.
pub(crate) async fn list_agents(
    headers: HeaderMap,
    ctx: Option<Extension<TenantContext>>,
    State(pool): State<DbPool>,
    Query(params): Query<ListAgentsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = resolved_tenant(ctx.as_deref(), &headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let agents = sqlx::query_as::<_, AgentConfig>(
        &format!("SELECT {} FROM agent_configs WHERE tenant_id = ? ORDER BY updated_at DESC LIMIT ? OFFSET ?", AGENT_SELECT_COLS)
    )
    .bind(&tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("Failed to list agents: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "internal_error"})))
    })?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agent_configs WHERE tenant_id = ?")
        .bind(&tenant_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    let agents_envelope: Vec<Value> = agents.iter().map(agent_list_envelope).collect();
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "agents": agents_envelope,
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

/// GET /api/v1/agents/{id_or_name} — Detail view.
///
/// Path resolution: if `{id_or_name}` parses as `i64` → looked up by `id`,
/// otherwise by `name`. Both branches are filtered on the caller's tenant.
///
/// Returns 404 with `{"error":"agent_not_found"}` on miss — **same body and
/// same code path** whether the agent doesn't exist or exists under another
/// tenant. No cross-tenant existence oracle.
///
/// Emits a `tracing::info!(event = "agent.detail.read", …)` audit event on
/// success (forwarded to Tyr via the OTLP / log pipeline).
pub(crate) async fn get_agent(
    headers: HeaderMap,
    ctx: Option<Extension<TenantContext>>,
    State(pool): State<DbPool>,
    Path(id_or_name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = resolved_tenant(ctx.as_deref(), &headers);

    // Resolution: numeric → id, else → name. Pre-bound SQL, no concat.
    let agent: Option<AgentConfig> = if let Ok(id) = id_or_name.parse::<i64>() {
        sqlx::query_as::<_, AgentConfig>(&format!(
            "SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?",
            AGENT_SELECT_COLS
        ))
        .bind(id)
        .bind(&tenant_id)
        .fetch_optional(&pool)
        .await
    } else {
        sqlx::query_as::<_, AgentConfig>(&format!(
            "SELECT {} FROM agent_configs WHERE name = ? AND tenant_id = ?",
            AGENT_SELECT_COLS
        ))
        .bind(&id_or_name)
        .bind(&tenant_id)
        .fetch_optional(&pool)
        .await
    }
    .map_err(|e| {
        error!("get_agent db error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal_error"})),
        )
    })?;

    let agent = agent.ok_or_else(|| {
        // DEBUG-only with the identifier — never INFO/WARN to avoid building
        // a probing oracle in centralized logs (Tyr/Loki).
        tracing::debug!(
            tenant_id = %tenant_id,
            lookup = %id_or_name,
            "get_agent: not found"
        );
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "agent_not_found"})),
        )
    })?;

    // Audit event (M2 in the security plan). Best-effort: emitting via
    // tracing forwards through OTLP to Tyr without failing the request.
    let auth_mode = ctx
        .as_deref()
        .map(|c| if c.role == "viewer" && c.user_id.ends_with("_header") {
            "header_fallback"
        } else {
            "jwt"
        })
        .unwrap_or("legacy_header");
    info!(
        event = "agent.detail.read",
        tenant_id = %tenant_id,
        agent_id = agent.id,
        agent_name = %agent.name,
        auth_mode = auth_mode,
        "agent detail accessed"
    );

    Ok(Json(agent_detail_envelope(&agent)))
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
