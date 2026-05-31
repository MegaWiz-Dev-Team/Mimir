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

use super::icd10::expand_acronyms;

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
        // Lets the chat panel re-center the 3D graph on whatever disease
        // the user's question names (e.g. OSA), not just the seeded node.
        .route("/resolve_query", post(resolve_query))
        // Relations for a known entity_index — current-topic follow-ups.
        .route("/relations", post(relations))
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
    let bifrost_answer = bifrost_resp
        .final_answer
        .as_ref()
        .map(extract_answer_text)
        .unwrap_or_default();

    // Bifrost agent id=7 currently emits "I will search…" prose without
    // actually executing PrimeKG tools (see iris_swarm_chat_bifrost_gaps).
    // When detected, fall back to a direct Neo4j lookup so the user gets
    // real graph evidence instead of a dangling-action stub.
    let (answer, used_fallback) = if is_dangling_action(&bifrost_answer) {
        match primekg_fallback(&req.query).await {
            Some(text) => (text, true),
            None => (bifrost_answer, false),
        }
    } else {
        (bifrost_answer, false)
    };

    if used_fallback {
        info!("PrimeKG assistant: served via Neo4j fallback (Bifrost emitted dangling action)");
    }
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
                let bifrost_answer = resp
                    .final_answer
                    .as_ref()
                    .map(extract_answer_text)
                    .unwrap_or_default();
                let answer = if is_dangling_action(&bifrost_answer) {
                    // Heartbeat so dashboard knows we're still working
                    // during the (potentially slow) Neo4j fallback.
                    let _ = tx
                        .send(Ok(Event::default()
                            .event("status")
                            .data("primekg_lookup")))
                        .await;
                    match primekg_fallback(&req.query).await {
                        Some(text) => {
                            info!("PrimeKG stream: served via Neo4j fallback");
                            text
                        }
                        None => bifrost_answer,
                    }
                } else {
                    bifrost_answer
                };
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

// ── PrimeKG Bifrost-gap mitigation (2026-05-27) ──────────────────────────────
//
// Bifrost agent id=7 currently returns reasoning prose like "I will begin by
// searching for the entity 'OSA'..." without actually invoking PrimeKG tools.
// Until the agent_configs are wired to true MCP tool execution
// (see iris_swarm_chat_bifrost_gaps memory), detect that pattern and serve
// the question from a direct Neo4j fallback so the user gets real graph
// evidence instead of a stub.

/// Heuristic: does this look like a "I will do X next" stub instead of a
/// finished answer? Counts forward-looking markers; 2+ → fallback regardless
/// of length, since some agents pad the stub with metadata like "Confidence:
/// High (Process initiated)" that defeats a naive substance check.
fn is_dangling_action(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    let lower = t.to_lowercase();

    // Strong markers — any single occurrence is enough.
    let strong = [
        "tool execution is complete",
        "process initiated",
        "will provide the final list",
        "once the tool",
        "once the id is confirmed",
        "currently looking up",
        "searching for the entity",
    ];
    if strong.iter().any(|m| lower.contains(m)) {
        return true;
    }

    // Action markers — 2+ occurrences indicate a multi-step plan with no
    // execution. We tolerate one because real answers sometimes say
    // "Let me explain…" in a substantive paragraph.
    let actions = [
        "i will begin",
        "i will search",
        "i will look",
        "i will use",
        "i will perform",
        "i will start",
        "i will first",
        "i will then",
        "i will provide",
        "i need to",
        "let me search",
        "let me look",
        "let me first",
        "step 1:",
        "**step 1",
        "step 2:",
        "**step 2",
        "first, i will",
        "first, let me",
        "next, i will",
        "next, let me",
        "once confirmed",
        "once the",
    ];
    let action_count = actions.iter().filter(|m| lower.contains(*m)).count();
    if action_count >= 2 {
        return true;
    }

    // Short + one action marker still counts (the original case).
    if t.len() <= 400 && action_count >= 1 {
        return true;
    }
    false
}

/// Best-effort entity extraction from a free-text query.
/// - Expands medical abbreviations (OSA → obstructive sleep apnea)
/// - Pulls ASCII word runs (Thai/punctuation question prefix dropped)
/// - Returns candidate phrases longest-first for trial-and-error lookup
fn entity_candidates(query: &str) -> Vec<String> {
    let expanded = expand_acronyms(query);
    // Collect runs of ASCII alphabetic + hyphen, joined with single spaces.
    let mut current = String::new();
    let mut runs: Vec<String> = Vec::new();
    for ch in expanded.chars() {
        if ch.is_ascii_alphabetic() || ch == '-' {
            current.push(ch);
        } else if ch.is_ascii_whitespace() && !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    // Filter out trivial fillers + question words.
    const STOP: &[&str] = &[
        "what", "is", "the", "of", "for", "to", "and", "or", "in", "on",
        "are", "from", "about", "by", "an", "a", "with",
    ];
    let words: Vec<String> = runs
        .into_iter()
        .filter(|w| w.len() > 1 && !STOP.contains(&w.to_lowercase().as_str()))
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    // Build n-gram candidates from longest contiguous span down to single words.
    let mut out = Vec::new();
    for window in (1..=words.len()).rev() {
        for start in 0..=words.len() - window {
            let phrase = words[start..start + window].join(" ");
            if !out.contains(&phrase) {
                out.push(phrase);
            }
        }
    }
    out
}

/// PrimeKG relation types are stored UPPERCASE on the graph. Each tier is
/// tried in order; we serve the first non-empty result so the user always
/// sees the closest disease-context match (e.g. another disease) before
/// secondary information (symptoms, contraindicated drugs).
const RELATION_TIERS: &[&[&str]] = &[
    // Tier 1: disease ↔ disease (what the user usually means by "related").
    &["DISEASE_DISEASE"],
    // Tier 2: clinical phenotypes (symptoms, presenting features).
    &["DISEASE_PHENOTYPE_POSITIVE", "DISEASE_PHENOTYPE_NEGATIVE"],
    // Tier 3: pharmacology (drugs that treat or are contraindicated).
    &["INDICATION", "OFF-LABEL USE", "CONTRAINDICATION"],
    // Tier 4: any remaining association (genes/proteins, exposures).
    &[],
];

/// Resolve the most likely PrimeKG entity named in a free-text question.
/// Reuses `entity_candidates` (acronym expansion + longest-first n-gram
/// trials, e.g. OSA → obstructive sleep apnea), preferring `disease` type
/// for medical relationship questions. Returns `(entity_index, name, type)`
/// or `None` if Neo4j is disabled / nothing matched.
async fn resolve_query_entity(query: &str) -> Option<(i64, String, String)> {
    let svc = neo4j().await?;
    let candidates = entity_candidates(query);
    if candidates.is_empty() {
        return None;
    }
    // Try each candidate phrase longest-first until we find an entity.
    let mut chosen: Option<serde_json::Value> = None;
    for cand in &candidates {
        // Prefer disease type for medical relationship questions.
        let hits = svc
            .primekg_lookup_entity(cand, Some("disease"), 1)
            .await
            .unwrap_or_default();
        if let Some(top) = hits.into_iter().next() {
            chosen = Some(top);
            break;
        }
        // Fall back to any type (drugs, phenotypes, etc.).
        let hits = svc.primekg_lookup_entity(cand, None, 1).await.unwrap_or_default();
        if let Some(top) = hits.into_iter().next() {
            chosen = Some(top);
            break;
        }
    }
    let top = chosen?;
    let entity_index = top.get("entity_index").and_then(|v| v.as_i64())?;
    let entity_name = top
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("entity")
        .to_string();
    let entity_type = top
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("entity")
        .to_string();
    Some((entity_index, entity_name, entity_type))
}

#[derive(Deserialize)]
struct ResolveQueryReq {
    query: String,
}

/// Cap on CONTRAINDICATION edges returned for the evidence card. PrimeKG's
/// edge distribution is heavily skewed (a disease can have hundreds of
/// CONTRAINDICATION edges); a flat limit would alphabetically truncate the
/// rare-but-salient INDICATION / phenotype / disease-disease edges — the
/// v2.3.45 Solriamfetol/OSA bug. Keep ALL non-CONTRAINDICATION edges and
/// cap CONTRAINDICATION so it can't crowd out clinically-important relations.
const CONTRA_CAP: usize = 12;

/// Balanced first-hop relations for the chat's Graph Evidence card.
/// Flattens each neighbor to `{entity_index, name, type, relation}` where
/// `relation` is the first-hop edge type. Returns `[]` on any failure
/// (the card is best-effort; the LLM prose still renders).
async fn balanced_relations(entity_index: i64) -> Vec<JsonValue> {
    let Some(svc) = neo4j().await else {
        return Vec::new();
    };
    // Fetch wide across ALL relation types (1 hop), then balance here.
    let raw = svc
        .primekg_neighbors_filtered(entity_index, &[], 1, 250)
        .await
        .unwrap_or_default();
    let mut contra = 0usize;
    let mut out = Vec::new();
    for item in raw {
        let relation = item
            .get("path_relations")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if relation == "CONTRAINDICATION" {
            contra += 1;
            if contra > CONTRA_CAP {
                continue;
            }
        }
        out.push(json!({
            "entity_index": item.get("entity_index").cloned().unwrap_or(JsonValue::Null),
            "name": item.get("name").cloned().unwrap_or(JsonValue::Null),
            "type": item.get("type").cloned().unwrap_or(JsonValue::Null),
            "relation": relation,
        }));
    }
    out
}

#[derive(Deserialize)]
struct RelationsReq {
    entity_index: i64,
}

/// POST /api/v1/knowledge/primekg/relations
/// Balanced first-hop relations for a KNOWN entity_index. Used when the
/// chat's topic comes from the selected graph node (a follow-up question
/// that names no disease, e.g. "ความสัมพันธ์กับยาอะไรบ้าง") — so the
/// evidence card AND the prompt-grounding still work for the current topic.
async fn relations(
    State(_pool): State<DbPool>,
    Json(req): Json<RelationsReq>,
) -> Json<JsonValue> {
    Json(json!({ "relations": balanced_relations(req.entity_index).await }))
}

/// POST /api/v1/knowledge/primekg/resolve_query
/// Extracts the disease/entity a free-text question is about, resolves it
/// to a PrimeKG node, and returns its balanced first-hop relations — so the
/// chat panel can both drive the 3D graph onto it AND render a deterministic
/// "Graph Evidence" card (clickable related entities grouped by relation,
/// straight from the graph — not LLM text). Returns `{}` when nothing
/// resolved (e.g. a Thai-only follow-up like "รักษายังไง" with no entity
/// named — the caller then keeps the current topic).
async fn resolve_query(
    State(_pool): State<DbPool>,
    Json(req): Json<ResolveQueryReq>,
) -> Json<JsonValue> {
    match resolve_query_entity(&req.query).await {
        Some((entity_index, name, entity_type)) => {
            let relations = balanced_relations(entity_index).await;
            Json(json!({
                "entity_index": entity_index,
                "name": name,
                "type": entity_type,
                "relations": relations,
            }))
        }
        None => Json(json!({})),
    }
}

/// Direct Neo4j fallback: lookup_entity → neighbors → text answer.
/// Returns `None` if Neo4j is disabled, no entity matched, or no neighbors found.
async fn primekg_fallback(query: &str) -> Option<String> {
    let svc = neo4j().await?;
    let (entity_index, entity_name, entity_type) = resolve_query_entity(query).await?;

    // Cascade through relation tiers until one returns results.
    let mut neighbors: Vec<serde_json::Value> = Vec::new();
    let mut tier_used: &str = "";
    for tier in RELATION_TIERS {
        let rels: Vec<String> = tier.iter().map(|s| s.to_string()).collect();
        let hits = svc
            .primekg_neighbors_filtered(entity_index, &rels, 1, 20)
            .await
            .ok()?;
        if !hits.is_empty() {
            neighbors = hits;
            tier_used = describe_tier(tier);
            break;
        }
    }
    if neighbors.is_empty() {
        return Some(format!(
            "**{entity_name}** ({entity_type}) was found in PrimeKG, but no graph neighbors were recorded for this entity.\n\n*Source: PrimeKG (direct Neo4j lookup, entity_index {entity_index})*"
        ));
    }

    let mut lines = String::new();
    lines.push_str(&format!(
        "**{tier_used} for {entity_name}** ({entity_type})\n*from PrimeKG knowledge graph — showing top {} results*\n\n",
        neighbors.len()
    ));
    for item in &neighbors {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let typ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
        let path_rels: Vec<String> = item
            .get("path_relations")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| r.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let rel = path_rels.join(" → ");
        if rel.is_empty() {
            lines.push_str(&format!("- **{name}** ({typ})\n"));
        } else {
            lines.push_str(&format!("- **{name}** ({typ}) — `{rel}`\n"));
        }
    }
    lines.push_str("\n*Direct PrimeKG graph lookup. For clinical decisions, consult a healthcare professional.*");
    Some(lines)
}

fn describe_tier(tier: &[&str]) -> &'static str {
    if tier.is_empty() {
        return "Graph neighbors";
    }
    if tier.contains(&"DISEASE_DISEASE") {
        return "Related diseases";
    }
    if tier.iter().any(|t| t.starts_with("DISEASE_PHENOTYPE")) {
        return "Associated symptoms / phenotypes";
    }
    if tier.contains(&"INDICATION") || tier.contains(&"CONTRAINDICATION") {
        return "Pharmacology (indications & contraindications)";
    }
    "Graph neighbors"
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    #[test]
    fn dangling_action_short_prose_triggers() {
        assert!(is_dangling_action(
            "I will begin by searching for the entity 'OSA'.\n\n**Step 1:** Searching..."
        ));
        assert!(is_dangling_action("Let me search the database for diabetes."));
    }

    #[test]
    fn substantive_answer_does_not_trigger() {
        assert!(!is_dangling_action(
            "Diabetes mellitus is related to:\n- Cardiovascular disease\n- Diabetic retinopathy"
        ));
        let long = "Diabetes mellitus. ".repeat(50);
        assert!(!is_dangling_action(&long));
    }

    #[test]
    fn dangling_long_padded_answer_still_triggers() {
        // The exact failure mode we saw on the stream test: agent emits
        // long prose with Step 1/Step 2 plan + "Confidence: High (Process
        // initiated)" trailer instead of executing the tools.
        let txt = "To provide an accurate list of diabetes complications, I will \
                   first identify the entity and then search for related diseases.\n\n\
                   **Step 1: Identifying Entity**\nI am currently looking up the entity ID.\n\n\
                   **Step 2: Finding Related Diseases**\nOnce the ID is confirmed, I will use \
                   primekg_neighbors to list complications.\n\n\
                   - **Confidence:** High (Process initiated)\n\
                   - **Limitations:** Results depend on graph mapping.";
        assert!(is_dangling_action(txt));
    }

    #[test]
    fn empty_treated_as_dangling() {
        assert!(is_dangling_action(""));
        assert!(is_dangling_action("   \n  "));
    }

    #[test]
    fn entity_candidates_expand_osa() {
        let cands = entity_candidates("OSA เกี่ยวกับโรคอะไรบ้าง");
        assert!(
            cands.iter().any(|c| c == "obstructive sleep apnea"),
            "expected expansion to surface; got {cands:?}"
        );
    }

    #[test]
    fn entity_candidates_skip_stopwords() {
        let cands = entity_candidates("what is diabetes");
        assert!(cands.iter().any(|c| c == "diabetes"));
        assert!(!cands.iter().any(|c| c == "is" || c == "what"));
    }

    #[test]
    fn entity_candidates_longest_first() {
        let cands = entity_candidates("hypertension complications");
        // 2-word phrase should appear before single words.
        let two_word = cands.iter().position(|c| c == "hypertension complications");
        let single = cands.iter().position(|c| c == "hypertension");
        assert!(two_word.is_some() && single.is_some());
        assert!(two_word.unwrap() < single.unwrap());
    }
}

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
