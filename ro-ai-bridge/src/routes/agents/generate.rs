//! Agent Generative Builder — Generate agent configs from natural language prompts.
//!
//! POST /api/v1/agents/generate
//! Takes a user description + provider/model selection and uses LLM to draft
//! a complete AgentConfig JSON. Returns the draft for user review before saving.

use crate::config::Config;
use crate::routes::llm_usage::insert_llm_usage_log;
use crate::routes::sources::{infer_api_base, resolve_llm_credentials};
use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

/// Input payload for the generative builder.
#[derive(Debug, Deserialize)]
pub struct GenerateAgentRequest {
    /// Natural language description of the agent the user wants
    pub prompt: String,
    /// LLM provider to use for generation (e.g. "heimdall", "openai", "gemini")
    pub provider: String,
    /// Model ID to use for generation
    pub model_id: String,
}

/// Output: a fully drafted agent config ready for user review + confirmation.
#[derive(Debug, Serialize)]
pub struct GenerateAgentResponse {
    pub draft: GeneratedAgentDraft,
    pub generation_model: String,
    pub generation_provider: String,
    pub latency_ms: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedAgentDraft {
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
}

const META_BUILDER_SYSTEM_PROMPT: &str = r#"
You are the Agent Studio Meta-Builder. Your job is to generate a complete agent configuration from a user's natural language description.

You MUST output a valid JSON object with EXACTLY these fields:
{
  "name": "snake_case_unique_name",
  "display_name": "Human Readable Name",
  "description": "A concise 1-2 sentence description of what this agent does",
  "system_prompt": "The full system prompt that instructs the agent on its behavior, tone, and capabilities. Must be detailed and specific (200+ characters).",
  "model_id": "<use the model_id provided in the user context>",
  "provider": "<use the provider provided in the user context>",
  "temperature": 0.7,
  "max_tokens": 4096,
  "use_rag": true,
  "use_knowledge_graph": false,
  "tools": [],
  "personality_traits": ["helpful", "concise"],
  "greeting": "A friendly greeting message in the agent's persona/language",
  "tier": 2
}

Rules:
1. The system_prompt should be comprehensive, professional, and tailored to the described use case.
2. For medical/clinical agents, set temperature LOW (0.2-0.4), enable use_knowledge_graph, and add safety disclaimers.
3. For creative/writing agents, set temperature HIGHER (0.8-1.2).
4. For customer support, keep temperature moderate (0.5-0.7).
5. personality_traits should be chosen from: helpful, concise, friendly, scholarly, analytical, creative, empathetic, precise, patient, thorough, structured, insightful, wise.
6. If the user mentions Thai language or is clearly Thai-speaking, write the greeting and system_prompt in Thai.
7. The tools array can contain: "vector_search", "graph_search", "tree_search", "WebSearch", "Calculator".
8. Output ONLY the JSON object, no markdown fences, no explanation.
"#;

/// POST /api/v1/agents/generate — Generate an agent config from natural language
pub(crate) async fn generate_agent(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<mimir_core_ai::services::db::DbPool>,
    Json(payload): Json<GenerateAgentRequest>,
) -> Result<Json<GenerateAgentResponse>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    if payload.prompt.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Prompt cannot be empty"})),
        ));
    }

    // Resolve the LLM credentials for the user's chosen provider/model
    let model_config =
        mimir_core_ai::services::db::get_model_by_id(&pool, &payload.model_id)
            .await
            .map_err(|e| {
                error!("Failed to look up model {}: {}", payload.model_id, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Model lookup failed: {}", e)})),
                )
            })?;

    let (api_key, api_base) = resolve_llm_credentials(&config, &model_config, &payload.model_id)?;

    // Build the meta-builder prompt
    let user_prompt = format!(
        "Generate an agent based on this description:\n\"{}\"\n\nThe user wants to use provider=\"{}\" and model_id=\"{}\". Set these values in the output.",
        payload.prompt, payload.provider, payload.model_id
    );

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let url = format!("{}chat/completions", api_base);

    let body = json!({
        "model": payload.model_id,
        "messages": [
            {"role": "system", "content": META_BUILDER_SYSTEM_PROMPT},
            {"role": "user", "content": user_prompt}
        ],
        "max_tokens": 2048,
        "temperature": 0.6
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error!("Meta-Builder HTTP error: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("LLM call failed: {}", e)})),
            )
        })?;

    let latency_ms = start.elapsed().as_millis() as i32;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        error!("Meta-Builder LLM error: {}", error_body);

        let _ = insert_llm_usage_log(
            &pool,
            tenant_id,
            &payload.model_id,
            &payload.provider,
            Some(&url),
            Some("agent_generate"),
            0, 0, 0,
            latency_ms,
            "error",
            Some(&error_body),
        )
        .await;

        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("LLM error: {}", error_body)})),
        ));
    }

    let resp_json: Value = response.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Parse failed: {}", e)})),
        )
    })?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let input_tokens = resp_json["usage"]["prompt_tokens"].as_i64().unwrap_or(0) as i32;
    let output_tokens = resp_json["usage"]["completion_tokens"].as_i64().unwrap_or(0) as i32;
    let total_tokens = resp_json["usage"]["total_tokens"].as_i64().unwrap_or(0) as i32;

    // Log usage
    let _ = insert_llm_usage_log(
        &pool,
        tenant_id,
        &payload.model_id,
        &payload.provider,
        Some(&url),
        Some("agent_generate"),
        input_tokens,
        output_tokens,
        total_tokens,
        latency_ms,
        "success",
        None,
    )
    .await;

    // Parse the LLM output as our draft schema (with self-healing for markdown fences)
    let clean_content = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let draft: GeneratedAgentDraft = serde_json::from_str(clean_content).map_err(|e| {
        error!("Meta-Builder returned invalid JSON: {}\n\nRaw content:\n{}", e, content);
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": format!("AI generated invalid config: {}. Please try again.", e),
                "raw_output": content,
            })),
        )
    })?;

    info!(
        "Meta-Builder generated agent '{}' via {}/{} in {}ms",
        draft.name, payload.provider, payload.model_id, latency_ms
    );

    Ok(Json(GenerateAgentResponse {
        draft,
        generation_model: payload.model_id,
        generation_provider: payload.provider,
        latency_ms,
    }))
}
