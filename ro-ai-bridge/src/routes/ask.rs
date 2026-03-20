//! Ask Route — Simple RAG Q&A endpoint
//!
//! - POST /api/ask — Ask a question, get an answer with sources

use axum::{
    routing::post,
    Router, Json, extract::{State, Extension},
    response::IntoResponse,
    http::{StatusCode, HeaderMap},
};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::middleware::tenant::TenantContext;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::qdrant::QdrantService;
use mimir_core_ai::services::iam::IamService;
use mimir_core_ai::rag_engine::{OracleRagAgent, LlmProvider};
use mimir_core_ai::models::persona::Persona;
use serde::{Deserialize, Serialize};
use tracing::info;

// ─── Request / Response Types ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AskRequest {
    pub question: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Serialize)]
pub struct AskResponse {
    pub answer: String,
    pub confidence: f32,
    pub latency_ms: u64,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<serde_json::Value>>,
}

// ─── Route Registration ────────────────────────────────────────────────────

pub fn ask_routes() -> Router<DbPool> {
    Router::new()
        .route("/api/ask", post(ask_handler))
}

// ─── Handler ───────────────────────────────────────────────────────────────

async fn ask_handler(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<AskRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let tenant_id = tenant_ctx.as_ref()
        .map(|ctx| ctx.tenant_id.clone())
        .or_else(|| payload.tenant_id.clone())
        .unwrap_or_else(|| extract_tenant_id(&headers).to_string());

    // Resolve provider & model
    let iam = IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();

    let llm_config = tenant_config.as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();

    let default_p = tenant_config.as_ref().map(|c| c.default_provider.as_str());
    let default_m = tenant_config.as_ref().map(|c| c.default_model.as_str());
    let resolved = llm_config.resolve_slot("chat", default_p, default_m);

    let provider = payload.provider.as_deref()
        .map(|p| p.parse::<LlmProvider>().unwrap_or_default())
        .unwrap_or_default();

    let model = payload.model.clone()
        .unwrap_or(resolved.model);

    info!(
        event = "ask",
        question = %payload.question,
        provider = %provider,
        model = %model,
        tenant = %tenant_id,
        "🧠 /api/ask query"
    );

    // Load default persona (Oracle for RAG)
    let persona = match Persona::load_by_name_cached("oracle") {
        Ok(p) => p,
        Err(_) => {
            Persona::load_by_name_cached("default").unwrap_or_else(|_| {
                Persona {
                    name: "oracle".to_string(),
                    display_name: "RAG Query Agent".to_string(),
                    tier: 2,
                    model_id: None,
                    avatar_url: None,
                    system_prompt: "You are a helpful knowledge assistant. Answer questions accurately based on the provided context.".to_string(),
                    greeting: None,
                    allowed_actions: vec![],
                    personality_traits: vec![],
                }
            })
        }
    };

    // Build RAG agent (tier 2 — retrieval-augmented)
    let qdrant = QdrantService::new();
    let plugins: Vec<Box<dyn mimir_core_ai::rag_engine::DynamicContextPlugin>> = vec![];

    let agent = OracleRagAgent::with_provider(
        persona, qdrant, plugins,
        provider.clone(), Some(&model), None,
        tenant_id,
    );

    match agent.chat(&payload.question).await {
        Ok(response) => {
            let sources: Vec<serde_json::Value> = response.sources.iter().map(|s| {
                serde_json::json!({
                    "source_type": s.source_type,
                    "source_id": s.source_id,
                    "relevance": s.relevance,
                    "snippet": s.snippet
                })
            }).collect();

            (StatusCode::OK, Json(AskResponse {
                answer: response.content,
                confidence: response.confidence_score,
                latency_ms: response.latency_ms,
                provider: provider.to_string(),
                model,
                sources: if sources.is_empty() { None } else { Some(sources) },
            })).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "❌ /api/ask failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("RAG query failed: {}", e),
                "question": payload.question,
            }))).into_response()
        }
    }
}
