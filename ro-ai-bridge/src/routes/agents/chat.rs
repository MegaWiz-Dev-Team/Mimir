//! Agent chat and conversation listing.

use crate::retrieval::graph::GraphRetriever;
use crate::routes::tenant::extract_tenant_id;
use axum::{
    Json, Extension,
    extract::{Path, State, Query},
    http::{StatusCode, HeaderMap},
};
use serde_json::{json, Value};
use tracing::{info, error};
use uuid::Uuid;
use std::sync::Arc;

use crate::config::Config;
use mimir_core_ai::services::db::DbPool;
use crate::routes::sources::{resolve_llm_credentials, infer_api_base};
use crate::routes::llm_usage::insert_llm_usage_log;

use super::crud::{AgentConfig, AgentChatRequest, AgentChatResponse, ConversationListQuery, ConversationSession, AGENT_SELECT_COLS};

/// POST /api/v1/agents/:id/chat — Chat with agent using its config
pub(crate) async fn agent_chat(
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

    // 4. Knowledge Graph augmentation (if enabled)
    let mut system_prompt = agent.system_prompt.clone();
    if agent.use_knowledge_graph.unwrap_or(false) {
        let graph_retriever = crate::retrieval::graph::SqlGraphRetriever::new(pool.clone());
        match graph_retriever.search(&payload.message, tenant_id, 5).await {
            Ok(graph_results) if !graph_results.is_empty() => {
                let retrieval_results = crate::retrieval::graph::graph_to_retrieval_results(&graph_results);
                let graph_context: Vec<String> = retrieval_results.iter()
                    .map(|r| format!("• {}", r.content))
                    .collect();
                let kg_section = format!(
                    "\n\n[Knowledge Graph Context]\nThe following entities and relationships are relevant:\n{}",
                    graph_context.join("\n")
                );
                system_prompt.push_str(&kg_section);
                info!(event = "kg_augmented", agent_id = id, entities = graph_results.len(), "KG context injected");
            }
            Ok(_) => { /* no relevant entities found, skip */ }
            Err(e) => {
                tracing::warn!(error = %e, "KG retrieval failed, continuing without graph context");
            }
        }
    }

    // 5. Build prompt with system prompt + user message
    let temperature = agent.temperature.unwrap_or(0.7);
    let max_tokens = agent.max_tokens.unwrap_or(2048);

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let url = format!("{}chat/completions", api_base);

    // Build messages array with conversation history
    let mut messages = vec![
        json!({"role": "system", "content": system_prompt}),
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
pub(crate) async fn list_agent_conversations(
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
