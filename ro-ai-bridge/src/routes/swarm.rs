use axum::{
    extract::{Path, State},
    routing::post,
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use mimir_core_ai::services::db::DbPool;

// Swarm endpoints proxy to Bifrost-RS

#[derive(Deserialize, Serialize)]
pub struct SwarmRequest {
    pub query: String,
    pub session_id: Option<String>,
}

/// Response schema from Bifrost-RS OverseerManager
#[derive(Deserialize)]
struct BifrostResponse {
    reasoning: Option<String>,
    final_answer: String,
    action_required: Option<serde_json::Value>,
}

/// Response schema returned to the dashboard / MegaCare
#[derive(Serialize, Deserialize)]
pub struct SwarmResponse {
    pub answer: String,
    pub reasoning: Option<String>,
    pub status: String,
    pub action_required: Option<serde_json::Value>,
}

pub fn swarm_routes() -> Router<DbPool> {
    Router::new().route("/swarm", post(swarm_search))
}

/// Resolve the Bifrost base URL from environment, falling back to K8s service DNS.
fn bifrost_base_url() -> String {
    std::env::var("BIFROST_URL")
        .unwrap_or_else(|_| "http://bifrost.asgard.svc:8100".to_string())
}

// Mimir acts as a proxy to Bifrost-RS for Swarm executions
async fn swarm_search(
    Path(tenant_id): Path<String>,
    Json(payload): Json<SwarmRequest>,
) -> (StatusCode, Json<SwarmResponse>) {
    let client = reqwest::Client::new();
    let bifrost_url = format!("{}/v1/agents/{}/run", bifrost_base_url(), tenant_id);

    tracing::info!(bifrost_url = %bifrost_url, tenant_id = %tenant_id, "Proxying swarm request to Bifrost");

    match client.post(&bifrost_url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<BifrostResponse>().await {
                    Ok(bifrost) => {
                        (StatusCode::OK, Json(SwarmResponse {
                            answer: bifrost.final_answer,
                            reasoning: bifrost.reasoning,
                            status: "ok".to_string(),
                            action_required: bifrost.action_required,
                        }))
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to parse Bifrost response");
                        (StatusCode::INTERNAL_SERVER_ERROR, Json(SwarmResponse {
                            answer: format!("Failed to parse JSON response from Bifrost: {}", e),
                            reasoning: None,
                            status: "error".to_string(),
                            action_required: None,
                        }))
                    }
                }
            } else {
                let status_code = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::error!(status = %status_code, body = %body, "Bifrost returned error");
                (StatusCode::BAD_GATEWAY, Json(SwarmResponse {
                    answer: format!("Bifrost returned HTTP {}: {}", status_code, body),
                    reasoning: None,
                    status: "error".to_string(),
                    action_required: None,
                }))
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to reach Bifrost");
            (StatusCode::SERVICE_UNAVAILABLE, Json(SwarmResponse {
                answer: format!("Failed to reach Bifrost at {}. Error: {}", bifrost_base_url(), e),
                reasoning: None,
                status: "error".to_string(),
                action_required: None,
            }))
        }
    }
}
