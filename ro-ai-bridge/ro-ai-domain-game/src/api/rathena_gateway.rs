use axum::{
    extract::{Json, State},
    routing::{Router, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use mimir_core_ai::services::db::DbPool;

pub fn rathena_routes() -> Router<DbPool> {
    Router::new().route("/chat", post(handle_chat))
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: String,
    pub player_char_id: i32,
    pub player_name: String,
    pub message: String,
    pub map_name: String,
    pub persona_name: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub success: bool,
    pub message: String,
    pub action: Option<Value>,
}

use crate::simple_npc::SimpleNpcAgent;
use mimir_core_ai::models::persona::Persona;

async fn handle_chat(
    State(_pool): State<DbPool>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    info!("Received chat from {}: {}", req.player_name, req.message);

    // Load persona configuration
    let persona = match Persona::load_by_name_cached(&req.persona_name) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to load persona '{}': {}", req.persona_name, e);
            return Json(ChatResponse {
                success: false,
                message: format!("I seem to have lost my memory. ({})", e),
                action: None,
            });
        }
    };

    // Initialize agent (using simple completion NPC logic here)
    let agent = if let Some(model_id) = persona.model_id.clone() {
        SimpleNpcAgent::with_model(persona, &model_id)
    } else {
        SimpleNpcAgent::new(persona)
    };

    // Chat with agent
    match agent.chat(&req.message).await {
        Ok(reply) => {
            let action = agent.action_capture.lock().await.clone();
            Json(ChatResponse {
                success: true,
                message: reply,
                action,
            })
        }
        Err(e) => {
            error!("Agent error: {}", e);
            Json(ChatResponse {
                success: false,
                message: "Sorry, I'm having trouble thinking right now.".to_string(),
                action: None,
            })
        }
    }
}
