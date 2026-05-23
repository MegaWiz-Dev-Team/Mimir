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
mod router;
mod templates;

// Re-export public types
pub use crud::{
    AgentChatRequest, AgentChatResponse, AgentConfig, ConversationListQuery, ConversationSession,
    CreateAgentRequest, ListAgentsQuery, UpdateAgentRequest, AGENT_SELECT_COLS,
};
pub use templates::AgentTemplate;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use mimir_core_ai::middleware::flexible_tenant::flexible_tenant_middleware;
use mimir_core_ai::services::db::DbPool;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};

/// Build the public read sub-router for list + detail. JWT-first auth with
/// X-Tenant-Id fallback (WARN), plus a 60 req/min/IP rate limit. Mounted
/// alongside the admin routes via `.merge()` so admin endpoints keep their
/// existing X-Tenant-Id-only contract.
fn public_read_routes() -> Router<DbPool> {
    // SmartIp picks X-Forwarded-For → X-Real-IP → Forwarded → peer IP, so
    // this works both behind k8s ingress and in tests.
    let governor_conf = std::sync::Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(60)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("governor config"),
    );

    Router::new()
        .route("/", get(crud::list_agents))
        .route("/{id_or_name}", get(crud::get_agent))
        .route_layer(middleware::from_fn(flexible_tenant_middleware))
        .layer(GovernorLayer::new(governor_conf))
}

/// Build the admin sub-router for write + interactive endpoints. Auth is
/// still the legacy `X-Tenant-Id`-trust pattern; these routes are reached
/// from Mimir Studio over an authenticated session, not from external
/// clients. Splitting them out keeps the public read endpoints' JWT path
/// from breaking Studio's existing flow.
fn admin_routes() -> Router<DbPool> {
    Router::new()
        .route("/", post(crud::create_agent))
        .route("/templates", get(templates::list_templates))
        .route(
            // Same path shape as the public read route — axum requires
            // matching positional path patterns when two routers merge, so
            // we reuse `{id_or_name}` here. update_agent/delete_agent still
            // declare `Path<i64>` so non-numeric inputs return 400 by
            // axum's extractor (admin clients always pass numeric IDs).
            "/{id_or_name}",
            axum::routing::put(crud::update_agent).delete(crud::delete_agent),
        )
        .route("/{id}/publish", post(crud::publish_agent))
        .route("/{id}/chat", post(chat::agent_chat))
        .route("/{id}/conversations", get(chat::list_agent_conversations))
        .route("/generate", post(generate::generate_agent))
        // Sprint 38 B-27: specialty router — POST /agents/route → returns selected specialist
        .route("/route", post(router::route_question))
}

pub fn agents_routes() -> Router<DbPool> {
    public_read_routes().merge(admin_routes())
}
