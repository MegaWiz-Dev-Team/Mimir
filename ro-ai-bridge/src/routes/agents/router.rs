//! Sprint 38 — Specialty Router (B-27)
//!
//! POST /api/v1/agents/route — given a question + tenant, classify the
//! medical specialty and return the agent_id of the matching specialist.
//! Falls through to the tenant's `generic` agent when classification is
//! low-confidence or no specialist for the chosen specialty exists.
//!
//! Wire this in front of the regular /agents/:id/chat. The dashboard
//! "Chat with Eir" entrypoint can call this first to pick the right
//! specialist, then call /chat on the returned agent_id.

use axum::{extract::State, http::StatusCode, Extension, Json};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;

use crate::routes::tenant::extract_tenant_id;

#[derive(Debug, Deserialize)]
pub struct RouteRequest {
    pub question: String,
    /// Optional override for which router agent to use (default: first
    /// `is_router=1` agent for the tenant). Useful when a tenant has more
    /// than one router (e.g. specialty router + triage router).
    #[serde(default)]
    pub router_agent_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub specialty: String,
    pub confidence: f32,
    pub reasoning: String,
    /// Selected agent for the dispatch — call /agents/{id}/chat to continue.
    pub selected_agent_id: i64,
    pub selected_agent_name: String,
    pub selected_model_id: String,
    /// True when the router fell through to the `generic` agent because
    /// either confidence was low or no specialist existed for the picked
    /// specialty in this tenant.
    pub fell_through_to_generic: bool,
    pub router_latency_ms: i64,
}

pub async fn route_question(
    headers: axum::http::HeaderMap,
    State(pool): State<DbPool>,
    Json(req): Json<RouteRequest>,
) -> Result<(StatusCode, Json<RouteResponse>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Find the tenant's router agent
    let router_row: Option<(i64, String, String, Option<Value>)> = match req.router_agent_id {
        Some(rid) => {
            sqlx::query_as("SELECT id, model_id, system_prompt, routes_to_specialties
                            FROM agent_configs WHERE id = ? AND tenant_id = ?")
                .bind(rid).bind(&tenant_id).fetch_optional(&pool).await
        }
        None => {
            sqlx::query_as("SELECT id, model_id, system_prompt, routes_to_specialties
                            FROM agent_configs
                            WHERE tenant_id = ? AND is_router = 1
                            ORDER BY id LIMIT 1")
                .bind(&tenant_id).fetch_optional(&pool).await
        }
    }.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({"error": format!("router lookup: {}", e)}))))?;

    let (_router_id, router_model, system_prompt, _routes) = router_row.ok_or((
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "no router agent for this tenant; create one with is_router=1"}))
    ))?;

    // Classify via cheap Gemini call. The router's system_prompt embeds the
    // taxonomy + JSON output contract; we just hand it the user's question.
    let t0 = std::time::Instant::now();
    let prompt = format!("{}\n\nQuestion: {}", system_prompt, req.question);
    let cfg = mimir_core_ai::services::gemini_helper::GeminiCallConfig {
        temperature: 0.0, max_output_tokens: 256, force_json: true, timeout_secs: 15,
    };
    let raw = match mimir_core_ai::services::gemini_helper::call_text(
        &router_model, &prompt, &cfg).await {
        Ok(r) => r,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR,
                              Json(serde_json::json!({"error": format!("router LLM: {}", e)})))),
    };

    let parsed: Value = serde_json::from_str(&raw.text).unwrap_or_else(|_| {
        serde_json::json!({"specialty":"generic","confidence":0.0,
                           "reasoning":"failed to parse router output, falling back"})
    });
    let specialty_picked = parsed.get("specialty").and_then(|v| v.as_str())
        .unwrap_or("generic").to_string();
    let confidence = parsed.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let reasoning = parsed.get("reasoning").and_then(|v| v.as_str())
        .unwrap_or("(no reasoning)").to_string();

    // Look up the specialist agent for this tenant + specialty.
    // If not found, fall through to `generic` — that's the contract.
    const CONFIDENCE_THRESHOLD: f32 = 0.5;
    let target_specialty = if confidence >= CONFIDENCE_THRESHOLD {
        specialty_picked.clone()
    } else {
        "generic".to_string()
    };

    let specialist: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT id, name, model_id FROM agent_configs
         WHERE tenant_id = ? AND specialty = ? AND is_router = 0
         ORDER BY id LIMIT 1"
    )
    .bind(&tenant_id).bind(&target_specialty)
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR,
                  Json(serde_json::json!({"error": format!("specialist lookup: {}", e)}))))?;

    let (selected_id, selected_name, selected_model, fell_through) = match specialist {
        Some((id, n, m)) => (id, n, m, target_specialty == "generic" && specialty_picked != "generic"),
        None => {
            // Specialty named but no agent — fall through to generic
            let row = sqlx::query("SELECT id, name, model_id FROM agent_configs
                                   WHERE tenant_id = ? AND specialty = 'generic' AND is_router = 0
                                   ORDER BY id LIMIT 1")
                .bind(&tenant_id).fetch_optional(&pool).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR,
                              Json(serde_json::json!({"error": format!("generic lookup: {}", e)}))))?;
            match row {
                Some(r) => (r.get::<i64, _>("id"), r.get::<String, _>("name"),
                            r.get::<String, _>("model_id"), true),
                None => return Err((StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "no generic fallback agent for this tenant"})))),
            }
        }
    };

    Ok((StatusCode::OK, Json(RouteResponse {
        specialty: specialty_picked,
        confidence, reasoning,
        selected_agent_id: selected_id,
        selected_agent_name: selected_name,
        selected_model_id: selected_model,
        fell_through_to_generic: fell_through,
        router_latency_ms: t0.elapsed().as_millis() as i64,
    })))
}
