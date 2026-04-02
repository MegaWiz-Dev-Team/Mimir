use axum::{
    extract::{Path, State},
    routing::post,
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use mimir_core_ai::services::{db::DbPool, llm_router::LlmRouter, qdrant::QdrantService};

use crate::swarm::overseer::{ActionApproval, OverseerManager};

#[derive(Deserialize)]
pub struct SwarmRequest {
    pub query: String,
    pub session_id: Option<String>,
}

#[derive(Serialize)]
pub struct SwarmResponse {
    pub answer: String,
    pub status: String,
    pub action_required: Option<ActionApproval>,
}

pub fn swarm_routes() -> Router<DbPool> {
    Router::new().route("/swarm", post(swarm_search))
}

async fn swarm_search(
    State(pool): State<DbPool>,
    Path(tenant_id): Path<String>,
    Json(payload): Json<SwarmRequest>,
) -> (StatusCode, Json<SwarmResponse>) {
    let qdrant = Arc::new(QdrantService::new());
    
    // Fallback/standard resolving of dynamic config
    let mut router = match LlmRouter::new(pool.clone(), &tenant_id).await {
        Ok(r) => r,
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SwarmResponse { answer: format!("Router config failed: {}", e), status: "error".to_string(), action_required: None })
        )
    };
    let router = Arc::new(router);

    let manager = OverseerManager::new(pool, qdrant, router);

    match manager.run_swarm(&tenant_id, &payload.query, payload.session_id.as_deref()).await {
        Ok(response) => {
            let status_code = if response.action_required.is_some() { StatusCode::PRECONDITION_REQUIRED } else { StatusCode::OK };
            let status_string = if response.action_required.is_some() { "WAITING_FOR_APPROVAL".to_string() } else { "success".to_string() };
            
            (
                status_code,
                Json(SwarmResponse {
                    answer: response.final_answer,
                    status: status_string,
                    action_required: response.action_required,
                })
            )
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SwarmResponse {
                answer: format!("Swarm execution failed: {}", e),
                status: "error".to_string(),
                action_required: None,
            })
        ),
    }
}
