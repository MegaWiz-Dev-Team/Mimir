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

mod chat;
mod crud;
mod generate;
mod templates;

// Re-export public types
pub use crud::{
    AgentChatRequest, AgentChatResponse, AgentConfig, ConversationListQuery, ConversationSession,
    CreateAgentRequest, ListAgentsQuery, UpdateAgentRequest, AGENT_SELECT_COLS,
};
pub use templates::AgentTemplate;

use axum::{
    routing::{get, post},
    Router,
};
use mimir_core_ai::services::db::DbPool;

pub fn agents_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(crud::list_agents).post(crud::create_agent))
        .route("/templates", get(templates::list_templates))
        .route(
            "/{id}",
            get(crud::get_agent)
                .put(crud::update_agent)
                .delete(crud::delete_agent),
        )
        .route("/{id}/publish", post(crud::publish_agent))
        .route("/{id}/chat", post(chat::agent_chat))
        .route("/{id}/conversations", get(chat::list_agent_conversations))
        .route("/generate", post(generate::generate_agent))
}
