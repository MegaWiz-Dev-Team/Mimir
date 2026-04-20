use axum::{
    extract::Path,
    routing::post,
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use mimir_core_ai::services::db::DbPool;

// Swarm endpoints proxy to Bifrost-RS

#[derive(Deserialize, Serialize)]
pub struct SwarmRequest {
    pub agent_id: i64,
    pub query: String,
    pub session_id: Option<String>,
}

/// Response schema from Bifrost-RS OverseerManager
#[derive(Deserialize)]
struct BifrostResponse {
    reasoning: Option<String>,
    final_answer: String,
    action_required: Option<serde_json::Value>,
    trace_id: Option<String>,
    steps: Option<Vec<serde_json::Value>>,
}

/// Response schema returned to the dashboard / MegaCare
#[derive(Serialize, Deserialize)]
pub struct SwarmResponse {
    pub answer: String,
    pub reasoning: Option<String>,
    pub status: String,
    pub action_required: Option<serde_json::Value>,
    pub trace_id: Option<String>,
    pub steps: Option<Vec<serde_json::Value>>,
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
    headers: axum::http::HeaderMap,
    Path(_tenant_id_url): Path<String>, // Keep backward compatibility on route path
    Json(payload): Json<SwarmRequest>,
) -> (StatusCode, Json<SwarmResponse>) {
    let tenant_id = crate::routes::tenant::extract_tenant_id(&headers).to_string();
    let client = reqwest::Client::new();
    let bifrost_url = format!("{}/v1/agents/{}/run", bifrost_base_url(), payload.agent_id);

    tracing::info!(bifrost_url = %bifrost_url, tenant_id = %tenant_id, agent_id = %payload.agent_id, "Proxying swarm request to Bifrost");

    match client.post(&bifrost_url)
        .header("X-Tenant-Id", &tenant_id)
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
                            trace_id: bifrost.trace_id,
                            steps: bifrost.steps,
                        }))
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to parse Bifrost response");
                        (StatusCode::INTERNAL_SERVER_ERROR, Json(SwarmResponse {
                            answer: format!("Failed to parse JSON response from Bifrost: {}", e),
                            reasoning: None,
                            status: "error".to_string(),
                            action_required: None,
                            trace_id: None,
                            steps: None,
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
                    trace_id: None,
                    steps: None,
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
                trace_id: None,
                steps: None,
            }))
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// TC_BIFROST_01: SwarmRequest parses agent_id payload correctly
    #[test]
    fn test_swarm_request_payload_deserialization() {
        let json_payload = r#"{
            "agent_id": 9,
            "query": "Hello",
            "session_id": "test_session_123"
        }"#;

        let req: SwarmRequest = serde_json::from_str(json_payload).expect("Failed to parse SwarmRequest");
        assert_eq!(req.agent_id, 9, "Agent ID should be extracted properly from payload");
        assert_eq!(req.query, "Hello");
        assert_eq!(req.session_id, Some("test_session_123".to_string()));
    }
}
