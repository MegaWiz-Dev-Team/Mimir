use axum::{
    extract::{Path, State},
    http::{StatusCode, HeaderMap},
    routing::{delete, get, post},
    Json, Router,
};
use mimir_core_ai::models::a2a::{
    CreateA2aRoutingRuleRequest, A2aRoutingRule, A2aDispatchRequest, A2aDispatchResponse,
    A2aChainStatus,
};
use mimir_core_ai::services::a2a::A2aService;
use mimir_core_ai::services::db::DbPool;
use serde_json::json;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;


pub fn a2a_routes() -> Router<DbPool> {
    Router::new()
        .route("/rules", get(list_routing_rules).post(create_routing_rule))
        .route("/rules/{rule_id}", delete(delete_routing_rule))
        .route("/dispatch", post(dispatch_message))
        .route("/chains/{chain_id}", get(get_chain_status))
}

/// GET /api/v1/a2a/rules
/// List all A2A routing rules visible to the current tenant
async fn list_routing_rules(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<A2aRoutingRule>>, StatusCode> {
    let tenant_id = headers
        .get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-Tenant-Id header on list_routing_rules");
            StatusCode::BAD_REQUEST
        })?;

    let service = A2aService::new(pool);
    match service.list_routing_rules(tenant_id).await {
        Ok(rules) => Ok(Json(rules)),
        Err(e) => {
            tracing::error!("Failed to list A2A rules: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/v1/a2a/rules
/// Create a new A2A routing rule
async fn create_routing_rule(
    State(pool): State<DbPool>,
    Json(req): Json<CreateA2aRoutingRuleRequest>,
) -> Result<(StatusCode, Json<A2aRoutingRule>), (StatusCode, Json<serde_json::Value>)> {
    let service = A2aService::new(pool);
    match service.create_routing_rule(req).await {
        Ok(rule) => {
            info!("Created A2A routing rule: {}", rule.id);
            Ok((StatusCode::CREATED, Json(rule)))
        }
        Err(e) => {
            tracing::error!("Failed to create A2A rule: {}", e);
            if e.to_string().contains("Duplicate entry") {
                Err((
                    StatusCode::CONFLICT,
                    Json(json!({"error": "Routing rule already exists for this source→target pair"})),
                ))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to create routing rule"})),
                ))
            }
        }
    }
}

/// DELETE /api/v1/a2a/rules/:rule_id
/// Delete an A2A routing rule
async fn delete_routing_rule(
    State(pool): State<DbPool>,
    Path(rule_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let service = A2aService::new(pool);
    match service.delete_routing_rule(&rule_id).await {
        Ok(_) => {
            info!("Deleted A2A routing rule: {}", rule_id);
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            if e.to_string().contains("not found") {
                Err(StatusCode::NOT_FOUND)
            } else {
                tracing::error!("Failed to delete A2A rule: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// POST /api/v1/a2a/dispatch
/// Dispatch a message from one agent to another (cross-tenant)
///
/// Flow:
/// 1. Validate routing rule exists for source→target agents
/// 2. Resolve chain (start new or continue existing)
/// 3. Generate dispatch ID
/// 4. Log dispatch (audit trail)
/// 5. Apply Skuggi PII redaction if enabled
/// 6. Forward message to target agent via Bifrost
/// 7. Return dispatch response with chain tracking info
async fn dispatch_message(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(req): Json<A2aDispatchRequest>,
) -> Result<(StatusCode, Json<A2aDispatchResponse>), (StatusCode, Json<serde_json::Value>)> {
    let source_tenant = headers
        .get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-Tenant-Id header on dispatch");
            (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing X-Tenant-Id header"})))
        })?;

    let service = A2aService::new(pool.clone());

    // 0. VALIDATE: Check routing rule exists first (before any logging)
    let routing_rule = match service.find_routing_rule_by_agents(
        source_tenant,
        &req.source_agent_id,
        &req.target_agent_id,
    ).await {
        Ok(Some(rule)) => rule,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "No routing rule found for this source→target pair"})),
            ))
        }
        Err(e) => {
            tracing::error!("Failed to lookup routing rule: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to validate route"})),
            ))
        }
    };

    let target_tenant = &routing_rule.target_tenant_id;
    let dispatch_id = Uuid::new_v4().to_string();

    // Phase 4: Resolve chain
    let (chain_id, chain_step) = if req.chain_id.is_none() {
        // Start new chain
        let resolution = A2aService::new_chain_resolution(&req, source_tenant);
        let chain_id = resolution.chain_id.clone();
        let chain_step = resolution.chain_step;
        if let Err(e) = service.start_chain(&chain_id, source_tenant, &req.source_agent_id, target_tenant, &req.target_agent_id).await {
            tracing::error!("Failed to start chain: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to start chain"})),
            ));
        }
        (chain_id, chain_step)
    } else {
        // Continue existing chain
        let chain_id = req.chain_id.as_ref().unwrap().clone();
        if let Err(e) = service.continue_chain(&chain_id, target_tenant, &req.target_agent_id).await {
            tracing::error!("Chain validation failed: {}", e);
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": format!("Chain validation failed: {}", e)})),
            ));
        }
        // Get updated chain status to find the new step
        match service.get_chain(&chain_id).await {
            Ok(chain_status) => (chain_id, chain_status.current_step),
            Err(e) => {
                tracing::error!("Failed to get chain status: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to determine chain step"})),
                ));
            }
        }
    };

    info!(
        "A2A dispatch: {} (tenant={}) → {} (tenant={}) [chain={}, step={}]",
        req.source_agent_id, source_tenant, req.target_agent_id, target_tenant,
        chain_id, chain_step
    );

    // 1. Log the dispatch attempt with chain tracking
    if let Err(e) = service.log_dispatch(
        &dispatch_id,
        &req,
        source_tenant,
        target_tenant,
        &req.message,
        &chain_id,
        chain_step,
    ).await {
        tracing::error!("Failed to log dispatch: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to log dispatch"})),
        ));
    }

    // 2. Apply Skuggi PII redaction if enabled
    let (message_to_dispatch, redacted_fields) = if req.require_pii_redaction {
        use skuggi_core::redact_text;

        let redaction_result = redact_text(&req.message);

        let mut redacted_fields = std::collections::HashMap::new();

        // Log each redaction and aggregate counts
        for detection in &redaction_result.detections {
            redacted_fields.insert(detection.category.to_string(), detection.count as i32);

            // Log redaction event (don't log actual PII, just the fact it was redacted)
            let _ = service.log_redaction(
                &dispatch_id,
                &detection.category,
                detection.count as i32,
                0.99, // Skuggi regex patterns are high confidence
            ).await.map_err(|e| {
                tracing::error!("Failed to log redaction: {}", e);
                // Don't fail dispatch if redaction logging fails, just log the error
            });
        }

        let redacted_fields = if redacted_fields.is_empty() {
            None
        } else {
            Some(redacted_fields)
        };

        info!(redacted_count = redaction_result.detections.len(), "PII redacted from dispatch message");
        (redaction_result.redacted_text, redacted_fields)
    } else {
        (req.message.clone(), None)
    };

    // 2. Call Bifrost to actually dispatch to target agent
    let bifrost_url = bifrost_base_url();
    let client = reqwest::Client::new();
    let bifrost_call_url = format!("{}/v1/agents/dispatch", bifrost_url);

    let bifrost_payload = json!({
        "source_agent_id": req.source_agent_id,
        "target_agent_id": req.target_agent_id,
        "message": message_to_dispatch,
        "dispatch_id": dispatch_id,
        "require_pii_redaction": req.require_pii_redaction,
        "chain_id": chain_id,
        "chain_step": chain_step,
        "chain_context": req.context,
    });

    match client
        .post(&bifrost_call_url)
        .header("X-Tenant-Id", target_tenant)
        .header("X-Source-Tenant-Id", source_tenant)
        .json(&bifrost_payload)
        .timeout(Duration::from_secs(120))
        .send()
        .await
    {
        Ok(resp) => {
            let status_code = resp.status();

            if status_code.is_success() {
                // Update dispatch status to delivered
                let _ = service.update_dispatch_status(&dispatch_id, "delivered", None).await;

                let response = A2aDispatchResponse {
                    dispatch_id,
                    status: "delivered".to_string(),
                    message: "Message dispatched to target agent".to_string(),
                    redaction_applied: req.require_pii_redaction,
                    redacted_fields,
                    chain_id: chain_id.clone(),
                    chain_step,
                };
                Ok((StatusCode::ACCEPTED, Json(response)))
            } else {
                tracing::error!(status = %status_code, "Bifrost dispatch failed");
                let _ = service.update_dispatch_status(&dispatch_id, "failed", Some("Bifrost returned error")).await;
                let _ = service.finalize_chain(&chain_id, "failed").await;

                let response = A2aDispatchResponse {
                    dispatch_id,
                    status: "failed".to_string(),
                    message: format!("Bifrost returned {}", status_code),
                    redaction_applied: req.require_pii_redaction,
                    redacted_fields,
                    chain_id: chain_id.clone(),
                    chain_step,
                };
                Err((
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::to_value(&response).unwrap_or(json!({"error": "Dispatch failed"}))),
                ))
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to call Bifrost");
            let _ = service.update_dispatch_status(&dispatch_id, "failed", Some(&e.to_string())).await;
            let _ = service.finalize_chain(&chain_id, "failed").await;

            let response = A2aDispatchResponse {
                dispatch_id,
                status: "failed".to_string(),
                message: "Failed to reach target service".to_string(),
                redaction_applied: req.require_pii_redaction,
                redacted_fields,
                chain_id,
                chain_step,
            };
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::to_value(&response).unwrap_or(json!({"error": "Service unavailable"}))),
            ))
        }
    }
}

/// GET /api/v1/a2a/chains/{chain_id}
/// Get chain status and all dispatches in the chain
async fn get_chain_status(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(chain_id): Path<String>,
) -> Result<Json<A2aChainStatus>, (StatusCode, Json<serde_json::Value>)> {
    let _tenant_id = headers
        .get("X-Tenant-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-Tenant-Id header on get_chain_status");
            (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing X-Tenant-Id header"})))
        })?;

    let service = A2aService::new(pool);
    match service.get_chain(&chain_id).await {
        Ok(chain_status) => {
            info!("Retrieved chain status: {}", chain_id);
            Ok(Json(chain_status))
        }
        Err(e) => {
            if e.to_string().contains("not found") {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "Chain not found"})),
                ))
            } else {
                tracing::error!("Failed to get chain status: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to retrieve chain status"})),
                ))
            }
        }
    }
}

/// Get Bifrost base URL from environment or use K8s service DNS
fn bifrost_base_url() -> String {
    std::env::var("BIFROST_URL")
        .unwrap_or_else(|_| "http://bifrost.asgard.svc:8100".to_string())
}
