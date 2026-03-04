//! Agent Studio — CRUD API for agent configurations, chat, and templates
//!
//! Endpoints:
//! - GET    /api/v1/agents              — list agent configs
//! - POST   /api/v1/agents              — create agent config
//! - GET    /api/v1/agents/templates     — list agent templates
//! - GET    /api/v1/agents/:id           — get agent config
//! - PUT    /api/v1/agents/:id           — update agent config
//! - DELETE /api/v1/agents/:id           — delete agent config
//! - POST   /api/v1/agents/:id/publish   — publish agent (generate API key)
//! - POST   /api/v1/agents/:id/chat      — chat with an agent
//! - GET    /api/v1/agents/:id/conversations — list conversations for agent

use crate::routes::tenant::extract_tenant_id;use axum::{
    routing::{get, post, put, delete},
    Router, Json,
    extract::{Path, State, Query},
    http::{StatusCode, HeaderMap},
    Extension,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tracing::{info, error, warn};
use uuid::Uuid;
use std::sync::Arc;

use crate::config::Config;
use mimir_core_ai::services::db::DbPool;
use crate::routes::sources::{resolve_llm_credentials, infer_api_base};
use crate::routes::llm_usage::insert_llm_usage_log;

/// SELECT column list for agent_configs queries.
/// Uses CAST(temperature AS DOUBLE) because MariaDB DECIMAL(3,2) is not compatible with Rust f64.
const AGENT_SELECT_COLS: &str = r#"
    id, tenant_id, name, display_name, description, system_prompt, model_id, provider,
    CAST(temperature AS DOUBLE) as temperature, max_tokens, top_k,
    use_rag, use_knowledge_graph, tools, personality_traits, greeting, avatar_url,
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
    pub tools: Option<Value>,
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
    pub tools: Option<Vec<String>>,
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
    pub tools: Option<Vec<String>>,
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
}

#[derive(Debug, Deserialize)]
pub struct ListAgentsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AgentTemplate {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub system_prompt: String,
    pub model_id: String,
    pub provider: String,
    pub temperature: f64,
    pub max_tokens: i32,
    pub use_rag: bool,
    pub use_knowledge_graph: bool,
    pub tools: Vec<String>,
    pub personality_traits: Vec<String>,
    pub greeting: String,
    pub tier: i32,
    pub avatar_url: String,
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

// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn agents_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_agents).post(create_agent))
        .route("/templates", get(list_templates))
        .route("/{id}", get(get_agent).put(update_agent).delete(delete_agent))
        .route("/{id}/publish", post(publish_agent))
        .route("/{id}/chat", post(agent_chat))
        .route("/{id}/conversations", get(list_agent_conversations))
}

// ─── Templates ──────────────────────────────────────────────────────────────────

fn get_templates() -> Vec<AgentTemplate> {
    vec![
        AgentTemplate {
            id: "npc_guide".into(),
            name: "npc_guide".into(),
            display_name: "NPC Guide (Tier 1)".into(),
            description: "Simple NPC with action commands (heal, buff, warp)".into(),
            system_prompt: "คุณคือ NPC Guide ในเกม Ragnarok Online สามารถช่วยตอบคำถามพื้นฐาน และดำเนินการคำสั่ง (Action) เช่น Heal, Buff, Warp ให้ผู้เล่นได้ ตอบเป็นภาษาไทยเสมอ พูดสั้นกระชับ".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.7,
            max_tokens: 2048,
            use_rag: false,
            use_knowledge_graph: false,
            tools: vec!["heal".into(), "buff".into(), "warp".into()],
            personality_traits: vec!["helpful".into(), "wise".into(), "concise".into()],
            greeting: "สวัสดีนักผจญภัย! ข้าพร้อมช่วยเหลือท่าน จะให้ข้าช่วยอะไรดี?".into(),
            tier: 1,
            avatar_url: "/avatars/mimir.png".into(),
        },
        AgentTemplate {
            id: "npc_scholar".into(),
            name: "npc_scholar".into(),
            display_name: "NPC Scholar (Tier 2 — RAG)".into(),
            description: "Knowledge expert with RAG retrieval for monster/item data".into(),
            system_prompt: "คุณคือ NPC นักปราชญ์ เชี่ยวชาญการค้นหาข้อมูลจากวิกิและฐานข้อมูล Ragnarok Online ค้นหาข้อมูล Monster, Item, Map จาก Knowledge Base (RAG) อย่างละเอียด ตอบเป็นภาษาไทย อ้างอิงแหล่งข้อมูล".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.5,
            max_tokens: 4096,
            use_rag: true,
            use_knowledge_graph: false,
            tools: vec!["QueryMobDb".into(), "QueryItemDb".into()],
            personality_traits: vec!["scholarly".into(), "thorough".into(), "analytical".into()],
            greeting: "ยินดีต้อนรับสู่หอสมุด! ข้าพร้อมค้นหาข้อมูลจากฐานความรู้ให้ท่าน".into(),
            tier: 2,
            avatar_url: "/avatars/sage_ariel.png".into(),
        },
        AgentTemplate {
            id: "npc_seer".into(),
            name: "npc_seer".into(),
            display_name: "NPC Seer (Tier 2 — Mysterious)".into(),
            description: "Mysterious fortune teller with cryptic RAG responses".into(),
            system_prompt: "คุณคือ NPC นักพยากรณ์ลึกลับ พูดด้วยถ้อยคำเป็นปริศนาและคำพยากรณ์ ใช้ RAG ค้นหาข้อมูลแต่ตอบในสไตล์ลึกลับ ตอบเป็นภาษาไทย".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.8,
            max_tokens: 4096,
            use_rag: true,
            use_knowledge_graph: false,
            tools: vec!["QueryMobDb".into(), "QueryItemDb".into()],
            personality_traits: vec!["mysterious".into(), "cryptic".into(), "enigmatic".into()],
            greeting: "ดวงดาวได้ทำนายการมาเยือนของท่าน... ถามข้ามาเถิด".into(),
            tier: 2,
            avatar_url: "/avatars/fortune_teller.png".into(),
        },
        AgentTemplate {
            id: "npc_blacksmith".into(),
            name: "npc_blacksmith".into(),
            display_name: "NPC Blacksmith (Tier 2 — Equipment)".into(),
            description: "Gruff equipment expert with item database knowledge".into(),
            system_prompt: "คุณคือ NPC ช่างตีเหล็ก พูดตรงๆ ห้วนๆ ถนัดเรื่องอาวุธ ชุดเกราะ และการคราฟ ใช้ RAG ค้นหาข้อมูล Item ตอบเป็นภาษาไทย".into(),
            model_id: "mlx-community/Qwen3.5-35B-A3B-4bit".into(),
            provider: "heimdall".into(),
            temperature: 0.6,
            max_tokens: 2048,
            use_rag: true,
            use_knowledge_graph: false,
            tools: vec!["QueryItemDb".into()],
            personality_traits: vec!["gruff".into(), "practical".into(), "knowledgeable".into()],
            greeting: "หืม? มีธุระอะไรก็ว่ามา ข้าถนัดเรื่องอาวุธชุดเกราะ".into(),
            tier: 2,
            avatar_url: "/avatars/blacksmith.png".into(),
        },
    ]
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// GET /api/v1/agents — List all agent configs
async fn list_agents(
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

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM agent_configs WHERE tenant_id = ?"
    )
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
async fn create_agent(
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
    let tools_json = payload.tools.as_ref().map(|t| json!(t));
    let traits_json = payload.personality_traits.as_ref().map(|t| json!(t));

    let result = sqlx::query(
        r#"INSERT INTO agent_configs
            (tenant_id, name, display_name, description, system_prompt, model_id, provider,
             temperature, max_tokens, top_k, use_rag, use_knowledge_graph,
             tools, personality_traits, greeting, avatar_url, template_id, tier, response_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
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
    .bind(&tools_json)
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
            (StatusCode::CONFLICT, Json(json!({"error": format!("Agent name '{}' already exists", payload.name)})))
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        }
    })?;

    let id = result.last_insert_id() as i64;
    info!("Created agent config id={} name={}", id, payload.name);

    let agent = sqlx::query_as::<_, AgentConfig>(&format!("SELECT {} FROM agent_configs WHERE id = ?", AGENT_SELECT_COLS))
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok((StatusCode::CREATED, Json(agent)))
}

/// GET /api/v1/agents/templates — List predefined templates
async fn list_templates() -> Json<Vec<AgentTemplate>> {
    Json(get_templates())
}

/// GET /api/v1/agents/:id — Get agent config by ID
async fn get_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<AgentConfig>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let agent = sqlx::query_as::<_, AgentConfig>(
        &format!("SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?", AGENT_SELECT_COLS)
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    agent.map(Json).ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Agent not found"})))
    })
}

/// PUT /api/v1/agents/:id — Update agent config
async fn update_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateAgentRequest>,
) -> Result<Json<AgentConfig>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Verify agent exists
    let existing = sqlx::query_as::<_, AgentConfig>(
        &format!("SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?", AGENT_SELECT_COLS)
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if existing.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Agent not found"}))));
    }

    let existing = existing.unwrap();
    let display_name = payload.display_name.unwrap_or(existing.display_name.unwrap_or_default());
    let description = payload.description.or(existing.description);
    let system_prompt = payload.system_prompt.unwrap_or(existing.system_prompt);
    let model_id = payload.model_id.unwrap_or(existing.model_id);
    let provider = payload.provider.unwrap_or(existing.provider);
    let temperature = payload.temperature.unwrap_or(0.7);
    let max_tokens = payload.max_tokens.unwrap_or(existing.max_tokens.unwrap_or(2048));
    let top_k = payload.top_k.unwrap_or(existing.top_k.unwrap_or(5));
    let tier = payload.tier.unwrap_or(existing.tier.unwrap_or(2));
    let response_mode = payload.response_mode.clone().unwrap_or_else(|| existing.response_mode.clone().unwrap_or_else(|| "streaming".into()));
    let use_rag = payload.use_rag.unwrap_or(existing.use_rag.unwrap_or(true));
    let use_kg = payload.use_knowledge_graph.unwrap_or(existing.use_knowledge_graph.unwrap_or(false));
    let tools_json = payload.tools.map(|t| json!(t)).or(existing.tools);
    let traits_json = payload.personality_traits.map(|t| json!(t)).or(existing.personality_traits);
    let greeting = payload.greeting.or(existing.greeting);
    let avatar_url = payload.avatar_url.or(existing.avatar_url);

    sqlx::query(
        r#"UPDATE agent_configs SET
            display_name = ?, description = ?, system_prompt = ?, model_id = ?, provider = ?,
            temperature = ?, max_tokens = ?, top_k = ?, use_rag = ?, use_knowledge_graph = ?,
            tools = ?, personality_traits = ?, greeting = ?, avatar_url = ?,
            tier = ?, response_mode = ?
        WHERE id = ? AND tenant_id = ?"#
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
    .bind(&tools_json)
    .bind(&traits_json)
    .bind(&greeting)
    .bind(&avatar_url)
    .bind(tier)
    .bind(&response_mode)
    .bind(id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let updated = sqlx::query_as::<_, AgentConfig>(&format!("SELECT {} FROM agent_configs WHERE id = ?", AGENT_SELECT_COLS))
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    info!("Updated agent config id={}", id);
    Ok(Json(updated))
}

/// DELETE /api/v1/agents/:id — Delete agent config
async fn delete_agent(
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Agent not found"}))));
    }

    info!("Deleted agent config id={}", id);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/agents/:id/publish — Generate API key, set published
async fn publish_agent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let api_key = format!("ak_{}", Uuid::new_v4().to_string().replace("-", ""));

    let result = sqlx::query(
        "UPDATE agent_configs SET is_published = TRUE, api_key = ? WHERE id = ? AND tenant_id = ?"
    )
    .bind(&api_key)
    .bind(id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Agent not found"}))));
    }

    info!("Published agent id={}, api_key generated", id);
    Ok(Json(json!({
        "id": id,
        "is_published": true,
        "api_key": api_key
    })))
}

/// POST /api/v1/agents/:id/chat — Chat with agent using its config
async fn agent_chat(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<AgentChatRequest>,
) -> Result<Json<AgentChatResponse>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // 1. Load agent config
    let agent = sqlx::query_as::<_, AgentConfig>(
        &format!("SELECT {} FROM agent_configs WHERE id = ? AND tenant_id = ?", AGENT_SELECT_COLS)
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "Agent not found"}))))?;

    let session_id = payload.session_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    // 2. Log user message
    let _ = sqlx::query(
        r#"INSERT INTO agent_conversations
            (tenant_id, agent_config_id, session_id, role, content, model_id)
        VALUES (?, ?, ?, 'user', ?, ?)"#
    )
    .bind(tenant_id)
    .bind(id)
    .bind(&session_id)
    .bind(&payload.message)
    .bind(&agent.model_id)
    .execute(&pool)
    .await;

    // 3. Resolve LLM credentials
    let model_config = mimir_core_ai::services::db::get_model_by_id(&pool, &agent.model_id)
        .await
        .map_err(|e| {
            error!("Failed to look up model {}: {}", agent.model_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Model lookup failed: {}", e)})))
        })?;

    let (api_key, api_base) = resolve_llm_credentials(&config, &model_config, &agent.model_id)?;

    // 4. Build prompt with system prompt + user message
    let temperature = agent.temperature.unwrap_or(0.7);
    let max_tokens = agent.max_tokens.unwrap_or(2048);

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let url = format!("{}chat/completions", api_base);

    // Build messages array with conversation history
    let mut messages = vec![
        json!({"role": "system", "content": agent.system_prompt}),
    ];

    // Load recent history for context (last 10 messages)
    let history: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT role, content FROM agent_conversations
        WHERE session_id = ? AND agent_config_id = ?
        ORDER BY created_at DESC LIMIT 10"#
    )
    .bind(&session_id)
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Add history in chronological order (excluding the just-inserted user message)
    for (role, content) in history.iter().rev().skip(1) {
        messages.push(json!({"role": role, "content": content}));
    }

    // Add current user message
    messages.push(json!({"role": "user", "content": payload.message}));

    let body = json!({
        "model": agent.model_id,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature
    });

    let response = client.post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error!("Agent chat HTTP error: {}", e);
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("LLM call failed: {}", e)})))
        })?;

    let latency_ms = start.elapsed().as_millis() as i32;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        error!("Agent chat LLM error: {}", error_body);

        // Log error usage
        let provider_str = model_config.as_ref().map(|m| m.provider.as_str()).unwrap_or("unknown");
        let _ = insert_llm_usage_log(
            &pool, tenant_id, &agent.model_id, provider_str,
            Some(&url), Some("agent_chat"),
            0, 0, 0, latency_ms, "error", Some(&error_body),
        ).await;

        return Err((StatusCode::BAD_GATEWAY, Json(json!({"error": format!("LLM error: {}", error_body)}))));
    }

    let resp_json: Value = response.json().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Parse failed: {}", e)}))))?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let input_tokens = resp_json["usage"]["prompt_tokens"].as_i64().unwrap_or(0) as i32;
    let output_tokens = resp_json["usage"]["completion_tokens"].as_i64().unwrap_or(0) as i32;
    let total_tokens = resp_json["usage"]["total_tokens"].as_i64().unwrap_or(0) as i32;

    // 5. Log usage
    let provider_str = model_config.as_ref().map(|m| m.provider.as_str()).unwrap_or(&agent.provider);
    let _ = insert_llm_usage_log(
        &pool, tenant_id, &agent.model_id, provider_str,
        Some(&url), Some("agent_chat"),
        input_tokens, output_tokens, total_tokens, latency_ms, "success", None,
    ).await;

    // 6. Log assistant message to conversation
    let _ = sqlx::query(
        r#"INSERT INTO agent_conversations
            (tenant_id, agent_config_id, session_id, role, content, model_id, latency_ms, input_tokens, output_tokens)
        VALUES (?, ?, ?, 'assistant', ?, ?, ?, ?, ?)"#
    )
    .bind(tenant_id)
    .bind(id)
    .bind(&session_id)
    .bind(&content)
    .bind(&agent.model_id)
    .bind(latency_ms)
    .bind(input_tokens)
    .bind(output_tokens)
    .execute(&pool)
    .await;

    info!("Agent chat id={} session={} latency={}ms tokens={}", id, session_id, latency_ms, total_tokens);

    Ok(Json(AgentChatResponse {
        content,
        session_id,
        model_id: agent.model_id,
        provider: agent.provider,
        latency_ms,
        input_tokens,
        output_tokens,
        confidence_score: None,
    }))
}

/// GET /api/v1/agents/:id/conversations — List conversation sessions
async fn list_agent_conversations(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Query(params): Query<ConversationListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let sessions: Vec<ConversationSession> = sqlx::query_as(
        r#"SELECT
            session_id,
            agent_config_id,
            COUNT(*) as message_count,
            MIN(created_at) as first_message_at,
            MAX(created_at) as last_message_at
        FROM agent_conversations
        WHERE tenant_id = ? AND agent_config_id = ?
        GROUP BY session_id, agent_config_id
        ORDER BY last_message_at DESC
        LIMIT ? OFFSET ?"#
    )
    .bind(tenant_id)
    .bind(id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({
        "sessions": sessions,
        "page": page,
        "per_page": per_page
    })))
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// TC_MIG_01: Templates are NPC-themed and valid
    #[test]
    fn test_templates_are_npc_themed() {
        let templates = get_templates();
        assert_eq!(templates.len(), 4, "Should have 4 NPC templates");
        assert_eq!(templates[0].id, "npc_guide");
        assert_eq!(templates[1].id, "npc_scholar");
        assert_eq!(templates[2].id, "npc_seer");
        assert_eq!(templates[3].id, "npc_blacksmith");
    }

    /// TC_MIG_02: All templates have required fields populated
    #[test]
    fn test_templates_have_required_fields() {
        for t in &get_templates() {
            assert!(!t.name.is_empty(), "Template name must not be empty");
            assert!(!t.system_prompt.is_empty(), "System prompt must not be empty");
            assert!(!t.model_id.is_empty(), "Model ID must not be empty");
            assert!(!t.greeting.is_empty(), "Greeting must not be empty");
            assert!(!t.provider.is_empty(), "Provider must not be empty");
            assert!(!t.avatar_url.is_empty(), "Avatar URL must not be empty");
            assert!(!t.personality_traits.is_empty(), "Traits must not be empty");
        }
    }

    /// TC_MIG_03: Template tiers are correct (exactly one Tier 1 guide)
    #[test]
    fn test_template_tiers() {
        let templates = get_templates();
        let tier1_count = templates.iter().filter(|t| t.tier == 1).count();
        let tier2_count = templates.iter().filter(|t| t.tier == 2).count();
        assert_eq!(tier1_count, 1, "Exactly one Tier 1 (NPC Guide) template");
        assert_eq!(tier2_count, 3, "Three Tier 2 (RAG) templates");
    }

    /// TC_MIG_04: Tier 1 template should not use RAG, Tier 2 should use RAG
    #[test]
    fn test_tier_rag_consistency() {
        let templates = get_templates();
        for t in &templates {
            if t.tier == 1 {
                assert!(!t.use_rag, "Tier 1 should not use RAG: {}", t.id);
            } else {
                assert!(t.use_rag, "Tier 2 should use RAG: {}", t.id);
            }
        }
    }

    /// TC_MIG_05: Tier 1 guide has action tools
    #[test]
    fn test_tier1_has_action_tools() {
        let templates = get_templates();
        let guide = templates.iter().find(|t| t.tier == 1).expect("Tier 1 guide");
        assert!(guide.tools.contains(&"heal".to_string()), "Guide should have heal");
        assert!(guide.tools.contains(&"buff".to_string()), "Guide should have buff");
        assert!(guide.tools.contains(&"warp".to_string()), "Guide should have warp");
    }

    /// TC_MIG_06: All NPC templates use Heimdall provider
    #[test]
    fn test_npc_templates_use_heimdall() {
        for t in &get_templates() {
            assert_eq!(t.provider, "heimdall", "NPC template '{}' should use heimdall", t.id);
        }
    }

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
            personality_traits: None,
            greeting: Some("Hello".into()),
            avatar_url: None,
            template_id: Some("npc_guide".into()),
            is_published: Some(false),
            api_key: None,
            tier: Some(1),
            response_mode: Some("streaming".into()),
            created_at: None,
            updated_at: None,
        };
        assert_eq!(config.tier, Some(1));
        assert_eq!(config.response_mode, Some("streaming".into()));
    }

    /// TC_MIG_09: CreateAgentRequest defaults for tier and response_mode
    #[test]
    fn test_create_request_defaults() {
        let req: CreateAgentRequest = serde_json::from_str(r#"{
            "name": "test",
            "system_prompt": "test prompt",
            "model_id": "test-model"
        }"#).unwrap();
        assert_eq!(req.tier, None, "Tier should default to None (resolved to 2 in handler)");
        assert_eq!(req.response_mode, None, "Response mode should default to None");
    }
}
