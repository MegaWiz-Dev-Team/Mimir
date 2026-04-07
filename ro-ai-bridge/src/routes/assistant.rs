use axum::{
    extract::{State, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::Deserialize;
use serde_json::json;
use tracing::error;

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::llm_router::{LlmRouter, UniversalClient};

#[derive(Deserialize, Debug)]
pub struct AssistantRequest {
    pub message: String,
    pub history: Option<Vec<AssistantMessage>>,
    pub current_page: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssistantMessage {
    pub role: String,
    pub content: String,
}

pub fn assistant_routes() -> Router<DbPool> {
    Router::new().route("/help", post(handle_assistant_chat))
}

/// POST /api/v1/assistant/help — Mimir Global Help Assistant
pub async fn handle_assistant_chat(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<AssistantRequest>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers).to_string();

    let router: LlmRouter = match LlmRouter::new(pool.clone(), &tenant_id).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to init LLM Router for assistant: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to initialize LLM routing."})),
            );
        }
    };

    let (client, model) = match router.resolve_client("generation") {
        Ok(pair) => pair,
        Err(e) => {
            error!("Failed to resolve LLM client for assistant: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to resolve LLM: {}", e)})),
            );
        }
    };

    let sys_prompt = format!(
        "You are 'Mimir Assistant', an embedded AI guide in the Mimir AI Dashboard. \
        The Mimir platform is an advanced Agentic Medical RAG (Retrieval-Augmented Generation) system. \
        The user is currently on the page: {}. \
        Your job is to answer questions about how to use the dashboard, explain features, \
        or clarify metrics (e.g. MRR, Hit Rate@K, NDCG). \
        Be concise, helpful, and polite. Reply in the same language the user speaks (mostly Thai). \
        If the user wants to report a bug or request a feature, kindly provide them a brief confirmation \
        and mention that you will help them record the feedback.",
        payload.current_page
    );

    // Build conversation context from history
    let mut user_input = String::new();
    if let Some(history) = &payload.history {
        for msg in history {
            user_input.push_str(&format!("{}: {}\n", msg.role, msg.content));
        }
    }
    user_input.push_str(&format!("user: {}", payload.message));

    match client.prompt(&model, &sys_prompt, &user_input, 1024, 0.3).await {
        Ok(reply) => {
            (StatusCode::OK, Json(json!({ "reply": reply })))
        }
        Err(e) => {
            error!("LLM routing failed for assistant: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to generate response: {}", e)})),
            )
        }
    }
}
