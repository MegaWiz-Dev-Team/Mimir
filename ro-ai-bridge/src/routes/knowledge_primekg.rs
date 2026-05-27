//! Hermodr MCP — PrimeKG graph-native endpoints.
//!
//! Six POST routes that back the Hermodr PrimeKG tool catalog (Hermodr PR #5).
//! All routes are graph-native (Neo4j Cypher), complementing the semantic
//! search at `/api/v1/knowledge/search`. Tenant context comes in via JWT or
//! a `tenant_id` field in the request body (currently ignored — PrimeKG is
//! a shared/global KB, not tenant-scoped).
//!
//!   POST /api/v1/knowledge/primekg/entity              — name/type lookup
//!   POST /api/v1/knowledge/primekg/neighbors           — multi-hop expand
//!   POST /api/v1/knowledge/primekg/drug_interactions   — DRUG_DRUG edges
//!   POST /api/v1/knowledge/primekg/disease_drugs       — INDICATION + CTRA + OFFLABEL
//!   POST /api/v1/knowledge/primekg/symptom_to_disease  — reverse phenotype
//!   POST /api/v1/knowledge/primekg/path                — shortest path(s)
//!
//! All routes return `{"status": "neo4j_disabled"}` with HTTP 503 when
//! USE_NEO4J_GRAPH is unset / Neo4j is unavailable, so callers can degrade
//! gracefully.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::post,
    Json, Router,
};
use futures::stream::Stream;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{mpsc, OnceCell};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

pub fn knowledge_primekg_routes() -> Router<DbPool> {
    Router::new()
        .route("/entity", post(lookup_entity))
        .route("/neighbors", post(neighbors))
        .route("/drug_interactions", post(drug_interactions))
        .route("/disease_drugs", post(disease_drugs))
        .route("/symptom_to_disease", post(symptom_to_disease))
        .route("/path", post(path))
        // Restored 2026-05-27 — Medical Knowledge Assistant chat panel.
        // Backend was deployed in dashboard v2.3.36 (May 22) but the
        // Rust route never got committed to git, then was lost when
        // v2.3.42 rebuilt without the WIP. Both routes proxy to
        // Bifrost PrimeKG Graph Agent (id=7, tenant=asgard_medical).
        .route("/assistant", post(assistant))
        .route("/assistant/stream", post(assistant_stream))
}

// ── PrimeKG assistant (Bifrost proxy) ─────────────────────────────────────────

/// Bifrost agent id for the PrimeKG Graph Agent. Per the
/// `primekg_graph_agent` memory: "agent id=7 grounds disease-
/// relationship Qs in PrimeKG via Bifrost; needs X-Tenant-Id header".
const PRIMEKG_AGENT_ID: u32 = 7;

/// Cross-tenant target: PrimeKG agent lives on `asgard_medical`.
/// Mimir dashboard (caller) may be on `asgard_platform` or any other
/// tenant — Bifrost ACL gates the consult.
const PRIMEKG_AGENT_TENANT: &str = "asgard_medical";

fn bifrost_base_url() -> String {
    std::env::var("BIFROST_URL")
        .unwrap_or_else(|_| "http://bifrost.asgard.svc:8100".to_string())
}

#[derive(Deserialize)]
struct AssistantRequest {
    query: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Serialize)]
struct AssistantResponse {
    answer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<String>,
}

/// Bifrost agent_run response shape (mirrors swarm.rs).
#[derive(Deserialize)]
struct BifrostAgentResponse {
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    final_answer: Option<serde_json::Value>,
}

/// Pulls the human-readable text out of Bifrost's `final_answer` —
/// which may arrive as a plain string OR as a stringified JSON like
/// `{"reasoning":...,"final_answer":...}` depending on the agent's
/// MCP layer. Mirrors the Iris v2.24.1 `extract_human_text` helper.
fn extract_answer_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => {
            // Try parsing the string as JSON (handles the
            // `{"reasoning":"...","final_answer":"..."}` shape).
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s.trim()) {
                if let Some(inner) = parsed.get("final_answer").or_else(|| parsed.get("answer")) {
                    return extract_answer_text(inner);
                }
            }
            s.clone()
        }
        serde_json::Value::Object(map) => {
            for key in ["final_answer", "answer", "reply", "content"] {
                if let Some(inner) = map.get(key) {
                    return extract_answer_text(inner);
                }
            }
            serde_json::to_string(v).unwrap_or_default()
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// POST /api/v1/knowledge/primekg/assistant
/// Non-streaming variant. Frontend uses this when SSE isn't needed.
async fn assistant(
    State(_pool): State<DbPool>,
    Json(req): Json<AssistantRequest>,
) -> Result<Json<AssistantResponse>, (StatusCode, Json<JsonValue>)> {
    let bifrost_resp = call_bifrost_primekg_agent(&req).await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("PrimeKG assistant failed: {e}")})),
        )
    })?;
    let answer = bifrost_resp
        .final_answer
        .as_ref()
        .map(extract_answer_text)
        .unwrap_or_default();
    Ok(Json(AssistantResponse {
        answer,
        reasoning: bifrost_resp.reasoning,
    }))
}

/// POST /api/v1/knowledge/primekg/assistant/stream
/// SSE stream: emits 3 event types:
///   * `status` — heartbeat while waiting on Bifrost
///   * `answer` — one JSON `{"answer":"…"}` body with the final text
///   * `error`  — one JSON `{"error":"…"}` body if anything fails
///
/// The current Bifrost agent_run API is request/response (not
/// streaming-native). Until Bifrost grows a true SSE endpoint, we
/// simulate the stream: emit `status` on entry, then await Bifrost,
/// then emit one `answer` event with the full text. This still gives
/// the dashboard a progress signal during the 5–10s call.
async fn assistant_stream(
    State(_pool): State<DbPool>,
    Json(req): Json<AssistantRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(8);

    tokio::spawn(async move {
        // Heartbeat so the dashboard's onStatus callback fires
        // (clears any "loading…" placeholder).
        let _ = tx
            .send(Ok(Event::default().event("status").data("consulting")))
            .await;

        match call_bifrost_primekg_agent(&req).await {
            Ok(resp) => {
                let answer = resp
                    .final_answer
                    .as_ref()
                    .map(extract_answer_text)
                    .unwrap_or_default();
                let payload = json!({"answer": answer});
                if let Ok(event) = Event::default().event("answer").json_data(&payload) {
                    let _ = tx.send(Ok(event)).await;
                }
            }
            Err(e) => {
                let payload = json!({"error": format!("PrimeKG assistant failed: {e}")});
                if let Ok(event) = Event::default().event("error").json_data(&payload) {
                    let _ = tx.send(Ok(event)).await;
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

/// Shared Bifrost call. Both `assistant` + `assistant_stream` route
/// through here so the agent-id / tenant / timeout constants live in
/// one place.
async fn call_bifrost_primekg_agent(req: &AssistantRequest) -> Result<BifrostAgentResponse, String> {
    let url = format!(
        "{}/v1/agents/{}/run",
        bifrost_base_url(),
        PRIMEKG_AGENT_ID,
    );
    info!(
        url = %url,
        tenant = %PRIMEKG_AGENT_TENANT,
        "PrimeKG assistant → Bifrost",
    );
    let body = json!({
        "query": req.query,
        "session_id": req.session_id,
    });
    let resp = reqwest::Client::new()
        .post(&url)
        .header("X-Tenant-Id", PRIMEKG_AGENT_TENANT)
        .header("Content-Type", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Bifrost HTTP {status}: {body}"));
    }
    resp.json::<BifrostAgentResponse>()
        .await
        .map_err(|e| format!("Bifrost JSON decode: {e}"))
}

// We use HeaderMap (potentially) for future Skuggi/JWT propagation —
// declared at top-level so the unused-import warning doesn't trip.
#[allow(dead_code)]
fn _hm_marker(_h: HeaderMap) {}

// ── shared Neo4j handle (local to this module to avoid coupling with graph.rs) ─

static NEO4J: OnceCell<Option<Arc<Neo4jService>>> = OnceCell::const_new();

async fn neo4j() -> Option<Arc<Neo4jService>> {
    NEO4J
        .get_or_init(|| async {
            if std::env::var("USE_NEO4J_GRAPH").as_deref() == Ok("true") {
                let config = Neo4jConfig::from_env();
                Neo4jService::try_new(&config).await.map(Arc::new)
            } else {
                None
            }
        })
        .await
        .clone()
}

type RouteError = (StatusCode, Json<JsonValue>);

fn neo4j_disabled() -> RouteError {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"status": "neo4j_disabled",
                    "hint": "set USE_NEO4J_GRAPH=true and ensure Neo4j is reachable"})),
    )
}

fn neo4j_error(err: anyhow::Error) -> RouteError {
    warn!("primekg neo4j error: {err}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "neo4j_query_failed", "detail": err.to_string()})),
    )
}

// ─── 1. lookup_entity ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LookupEntityReq {
    name: String,
    #[serde(default)]
    entity_type: Option<String>,
    #[serde(default = "default_limit_5")]
    limit: i64,
    #[serde(default)]
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

fn default_limit_5() -> i64 { 5 }

async fn lookup_entity(
    State(_pool): State<DbPool>,
    Json(req): Json<LookupEntityReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "name required"}))));
    }
    let limit = req.limit.clamp(1, 25);
    let items = svc
        .primekg_lookup_entity(name, req.entity_type.as_deref(), limit)
        .await
        .map_err(neo4j_error)?;
    Ok(Json(json!({"items": items, "count": items.len()})))
}

// ─── 2. neighbors ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct NeighborsReq {
    entity_index: i64,
    #[serde(default)]
    relation_types: Vec<String>,
    #[serde(default = "default_hops_1")]
    hops: u32,
    #[serde(default = "default_limit_25")]
    limit: i64,
    #[serde(default)]
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

fn default_hops_1() -> u32 { 1 }
fn default_limit_25() -> i64 { 25 }

async fn neighbors(
    State(_pool): State<DbPool>,
    Json(req): Json<NeighborsReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    let limit = req.limit.clamp(1, 100);
    let items = svc
        .primekg_neighbors_filtered(req.entity_index, &req.relation_types, req.hops, limit)
        .await
        .map_err(neo4j_error)?;
    Ok(Json(json!({"items": items, "count": items.len()})))
}

// ─── 3. drug_interactions ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DrugInteractionsReq {
    drug_index: i64,
    /// PrimeKG does NOT store severity natively — this filter is accepted
    /// for Hermodr-tool-contract compatibility but does NOT prune results.
    /// Caller should post-filter on `display_relation` if needed.
    #[serde(default)]
    #[allow(dead_code)]
    severity: Option<String>,
    #[serde(default = "default_limit_25")]
    limit: i64,
    #[serde(default)]
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

async fn drug_interactions(
    State(_pool): State<DbPool>,
    Json(req): Json<DrugInteractionsReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    let limit = req.limit.clamp(1, 100);
    let items = svc
        .primekg_drug_interactions(req.drug_index, limit)
        .await
        .map_err(neo4j_error)?;
    Ok(Json(json!({
        "items": items,
        "count": items.len(),
        "severity_filter_supported": false,
        "note": "PrimeKG does not track DDI severity natively — display_relation may help heuristic post-filtering."
    })))
}

// ─── 4. disease_drugs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DiseaseDrugsReq {
    disease_index: i64,
    #[serde(default = "default_limit_25")]
    limit_per_relation: i64,
    #[serde(default)]
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

async fn disease_drugs(
    State(_pool): State<DbPool>,
    Json(req): Json<DiseaseDrugsReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    let limit = req.limit_per_relation.clamp(1, 50);
    let groups = svc
        .primekg_disease_drugs(req.disease_index, limit)
        .await
        .map_err(neo4j_error)?;
    Ok(Json(groups))
}

// ─── 5. symptom_to_disease ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SymptomToDiseaseReq {
    phenotype_names: Vec<String>,
    #[serde(default = "default_min_match_1")]
    min_match: u32,
    #[serde(default = "default_limit_20")]
    limit: i64,
    #[serde(default)]
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

fn default_min_match_1() -> u32 { 1 }
fn default_limit_20() -> i64 { 20 }

async fn symptom_to_disease(
    State(_pool): State<DbPool>,
    Json(req): Json<SymptomToDiseaseReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    if req.phenotype_names.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "phenotype_names must be non-empty"})),
        ));
    }
    let limit = req.limit.clamp(1, 100);
    let min_match = req.min_match.max(1);
    let items = svc
        .primekg_symptom_to_disease(&req.phenotype_names, min_match, limit)
        .await
        .map_err(neo4j_error)?;
    Ok(Json(json!({"items": items, "count": items.len()})))
}

// ─── 6. path ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PathReq {
    from_index: i64,
    to_index: i64,
    #[serde(default = "default_max_hops_4")]
    max_hops: u32,
    #[serde(default = "default_limit_paths_3")]
    limit_paths: i64,
    #[serde(default)]
    #[allow(dead_code)]
    tenant_id: Option<String>,
}

fn default_max_hops_4() -> u32 { 4 }
fn default_limit_paths_3() -> i64 { 3 }

async fn path(
    State(_pool): State<DbPool>,
    Json(req): Json<PathReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    let limit_paths = req.limit_paths.clamp(1, 10);
    let items = svc
        .primekg_path(req.from_index, req.to_index, req.max_hops, limit_paths)
        .await
        .map_err(neo4j_error)?;
    Ok(Json(json!({"items": items, "count": items.len()})))
}
