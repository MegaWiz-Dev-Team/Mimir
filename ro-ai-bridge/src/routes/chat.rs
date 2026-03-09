//! Agent Chat Routes — shared between main binary and monitor binary
//!
//! - POST   /chat          — non-streaming chat
//! - POST   /chat/stream   — SSE streaming chat

use axum::{
    routing::post,
    Router, Json, extract::{State, Extension},
    response::{IntoResponse, sse::Event, Sse},
    http::{StatusCode, HeaderMap},
};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::qdrant::QdrantService;
use mimir_core_ai::services::iam::IamService;
use mimir_core_ai::rag_engine::{OracleRagAgent, LlmProvider};
use mimir_core_ai::models::persona::Persona;
use ro_ai_domain_game::simple_npc::SimpleNpcAgent;
use serde::{Deserialize, Serialize};
use tracing::info;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures::stream::Stream;

// ─── Request / Response Types ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChatRequest {
    pub tier: i8,
    pub message: String,
    pub persona: String,
    pub session_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Serialize)]
struct ChatResponse {
    content: String,
    tier: i8,
    persona: String,
    latency_ms: u64,
    provider: String,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools_used: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct StreamToken {
    token: String,
}

#[derive(Serialize)]
struct StreamDone {
    latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<serde_json::Value>,
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Resolve provider and model from request + tenant config defaults.
/// Priority: request payload > llm_config.chat slot > tenant default_provider/model > hardcoded.
async fn resolve_provider_model(
    pool: &DbPool,
    payload: &ChatRequest,
) -> (LlmProvider, String) {
    let tenant_id = payload.tenant_id.clone().unwrap_or_else(|| "default_tenant".to_string());
    let iam = IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();

    // Use llm_config.resolve_slot for smart fallback chain
    let llm_config = tenant_config.as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();

    let default_p = tenant_config.as_ref().map(|c| c.default_provider.as_str());
    let default_m = tenant_config.as_ref().map(|c| c.default_model.as_str());
    let resolved = llm_config.resolve_slot("chat", default_p, default_m);

    let provider = payload.provider.as_deref()
        .unwrap_or(&resolved.provider)
        .parse::<LlmProvider>()
        .unwrap_or(LlmProvider::Ollama);

    let model = payload.model.clone()
        .unwrap_or(resolved.model);

    (provider, model)
}

// ─── Route Registration ────────────────────────────────────────────────────

pub fn chat_routes() -> Router<DbPool> {
    Router::new()
        .route("/chat", post(chat_handler))
        .route("/chat/stream", post(chat_stream_handler))
}

// ─── Non-Streaming Handler ─────────────────────────────────────────────────

async fn chat_handler(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<ChatRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let tenant_id = tenant_ctx.as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or_else(|| payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    let (provider, model) = resolve_provider_model(&pool, &payload).await;

    info!("💬 chat: tier={}, persona={}, provider={}, model={}, tenant={}",
          payload.tier, payload.persona, provider, model, tenant_id);

    // Load persona
    let persona = match Persona::load_by_name_cached(&payload.persona) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Persona not found: {}", e)}))
            ).into_response();
        }
    };

    match payload.tier {
        1 => {
            let agent = SimpleNpcAgent::with_options(persona, Some(&provider.to_string()), Some(&model), None);
            match agent.chat(&payload.message).await {
                Ok(response) => {
                    let action = agent.action_capture.lock().await.clone();
                    Json(ChatResponse {
                        content: response,
                        tier: 1,
                        persona: payload.persona,
                        latency_ms: start.elapsed().as_millis() as u64,
                        provider: provider.to_string(),
                        model,
                        confidence_score: None,
                        confidence_level: None,
                        sources: None,
                        tools_used: None,
                        action,
                    }).into_response()
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Agent error: {}", e)}))
                ).into_response(),
            }
        }
        2 => {
            let qdrant = QdrantService::new();
            let mut plugins: Vec<Box<dyn mimir_core_ai::rag_engine::DynamicContextPlugin>> = vec![];
            plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryMobDbTool::new(pool.clone())));
            plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryItemDbTool::new(pool.clone())));

            let agent = OracleRagAgent::with_provider(
                persona, qdrant, plugins,
                provider.clone(), Some(&model), None,
                tenant_id,
            );

            match agent.chat(&payload.message).await {
                Ok(response) => {
                    Json(ChatResponse {
                        content: response.content,
                        tier: 2,
                        persona: payload.persona,
                        latency_ms: response.latency_ms,
                        provider: provider.to_string(),
                        model,
                        confidence_score: Some(response.confidence_score),
                        confidence_level: Some(format!("{:?}", response.confidence_level)),
                        sources: Some(response.sources.iter().map(|s| {
                            serde_json::json!({
                                "source_type": s.source_type,
                                "source_id": s.source_id,
                                "relevance": s.relevance,
                                "snippet": s.snippet
                            })
                        }).collect()),
                        tools_used: Some(response.tools_used),
                        action: None,
                    }).into_response()
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Agent error: {}", e)}))
                ).into_response(),
            }
        }
        _ => (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid tier. Must be 1 or 2"}))
        ).into_response(),
    }
}

// ─── Streaming Handler ─────────────────────────────────────────────────────

async fn chat_stream_handler(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let tenant_id = tenant_ctx.as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or_else(|| payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    let (provider, model) = resolve_provider_model(&pool, &payload).await;

    info!("💬 stream: tier={}, persona={}, provider={}, model={}, tenant={}",
          payload.tier, payload.persona, provider, model, tenant_id);

    let (tx, rx) = mpsc::channel(100);
    let start = std::time::Instant::now();

    let persona_name = payload.persona.clone();
    let message = payload.message.clone();
    let tier = payload.tier;

    tokio::spawn(async move {
        // Load persona
        let persona = match Persona::load_by_name_cached(&persona_name) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(Event::default()
                    .event("error")
                    .json_data(serde_json::json!({"error": format!("Persona not found: {}", e)}))
                ).await;
                return;
            }
        };

        match tier {
            1 => {
                let agent = SimpleNpcAgent::with_options(persona, Some(&provider.to_string()), Some(&model), None);
                match agent.chat(&message).await {
                    Ok(response) => {
                        let action = agent.action_capture.lock().await.clone();
                        // Simulate streaming by chunking words
                        let words: Vec<&str> = response.split_whitespace().collect();
                        for chunk in words.chunks(3) {
                            let token = chunk.join(" ") + " ";
                            let _ = tx.send(Event::default()
                                .event("token")
                                .json_data(StreamToken { token })
                            ).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                        let _ = tx.send(Event::default()
                            .event("done")
                            .json_data(StreamDone {
                                latency_ms: start.elapsed().as_millis() as u64,
                                confidence_score: None,
                                confidence_level: None,
                                sources: None,
                                action,
                            })
                        ).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Event::default()
                            .event("error")
                            .json_data(serde_json::json!({"error": format!("Agent error: {}", e)}))
                        ).await;
                    }
                }
            }
            2 => {
                let qdrant = QdrantService::new();
                let mut plugins: Vec<Box<dyn mimir_core_ai::rag_engine::DynamicContextPlugin>> = vec![];
                plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryMobDbTool::new(pool.clone())));
                plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryItemDbTool::new(pool.clone())));

                let agent = OracleRagAgent::with_provider(
                    persona, qdrant, plugins,
                    provider.clone(), Some(&model), None,
                    tenant_id.clone(),
                );

                match agent.chat(&message).await {
                    Ok(response) => {
                        let words: Vec<&str> = response.content.split_whitespace().collect();
                        for chunk in words.chunks(3) {
                            let token = chunk.join(" ") + " ";
                            let _ = tx.send(Event::default()
                                .event("token")
                                .json_data(StreamToken { token })
                            ).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                        let _ = tx.send(Event::default()
                            .event("done")
                            .json_data(StreamDone {
                                latency_ms: response.latency_ms,
                                confidence_score: Some(response.confidence_score),
                                confidence_level: Some(format!("{:?}", response.confidence_level)),
                                sources: Some(response.sources.iter().map(|s| {
                                    serde_json::json!({
                                        "source_type": s.source_type,
                                        "source_id": s.source_id,
                                        "relevance": s.relevance,
                                        "snippet": s.snippet
                                    })
                                }).collect()),
                                action: None,
                            })
                        ).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Event::default()
                            .event("error")
                            .json_data(serde_json::json!({"error": format!("Agent error: {}", e)}))
                        ).await;
                    }
                }
            }
            _ => {
                let _ = tx.send(Event::default()
                    .event("error")
                    .json_data(serde_json::json!({"error": "Invalid tier. Must be 1 or 2"}))
                ).await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}
