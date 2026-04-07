use axum::{
    extract::{Path, State},
    routing::post,
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use mimir_core_ai::services::db::DbPool;

// Swarm endpoints proxy to Bifrost

#[derive(Deserialize, Serialize)]
pub struct SwarmRequest {
    pub query: String,
    pub session_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SwarmResponse {
    pub answer: String,
    pub status: String,
    // pub action_required: Option<ActionApproval>,
}

pub fn swarm_routes() -> Router<DbPool> {
    Router::new().route("/swarm", post(swarm_search))
}

// Mimir acts as a proxy to Bifrost-RS for Swarm executions
async fn swarm_search(
    Path(tenant_id): Path<String>,
    Json(payload): Json<SwarmRequest>,
) -> (StatusCode, Json<SwarmResponse>) {
    let client = reqwest::Client::new();
    let bifrost_url = format!("http://localhost:8100/v1/agents/{}/run", tenant_id);
    
    match client.post(&bifrost_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                // Return Bifrost response exactly as-is assuming schema matches SwarmResponse
                if let Ok(bifrost_json) = resp.json::<SwarmResponse>().await {
                    (StatusCode::OK, Json(bifrost_json))
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(SwarmResponse {
                        answer: "Failed to parse JSON response from Bifrost".to_string(),
                        status: "error".to_string(),
                        // action_required: None,
                    }))
                }
            } else {
                (StatusCode::BAD_GATEWAY, Json(SwarmResponse {
                    answer: format!("Bifrost returned an error: HTTP {}", resp.status()),
                    status: "error".to_string(),
                    // action_required: None,
                }))
            }
        },
        Err(e) => {
            (StatusCode::SERVICE_UNAVAILABLE, Json(SwarmResponse {
                answer: format!("Failed to reach Bifrost. Is it running on port 8100? Error: {}", e),
                status: "error".to_string(),
            }))
        }
    }
}
