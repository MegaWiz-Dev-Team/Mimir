//! Medication safety check — structured clinical guardrail endpoint.
//!
//!   POST /api/v1/medical/medication-safety/check
//!     { "proposed_drug": "warfarin",
//!       "current_drugs": ["aspirin"],
//!       "conditions": ["asthma"] }
//!
//! Runs `PrimeKgPruner` (normalize → resolve → PrimeKG edge check) and returns a
//! decision. This is the STRUCTURED entry point for the clinical safety pruner:
//! agents call it deliberately (e.g. via the Hermodr MCP catalog) instead of
//! parsing free-text chat, which has no structured patient context.
//!
//! Returns 503 `neo4j_disabled` when `USE_NEO4J_GRAPH` is unset / Neo4j is
//! unreachable, so callers degrade gracefully — mirrors the primekg routes.
//!
//! NOTE: PrimeKG DRUG_DRUG carries no severity, so findings are UNRANKED. A
//! DDInter-sourced severity gate (eval-only, firewalled from the product KG) is
//! layered on later to cut over-flag. `unresolved` names are surfaced, never
//! silently passed — a name we can't map to a PrimeKG node is not "safe".

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use ro_ai_domain_medical::safety_pruner::{Decision, PatientContext, PrimeKgPruner};
use ro_ai_domain_medical::severity::Severity;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::warn;

const SEVERITY_NOTE: &str =
    "Severity from a curated ONC-15/FDA rules gate (license-clean). Unlisted \
     interactions default to 'moderate' (exists, severity not established) — \
     never 'safe'. Use min_severity to filter.";

pub fn medication_safety_routes() -> Router<DbPool> {
    Router::new().route("/check", post(check))
}

// Module-local Neo4j handle, gated on USE_NEO4J_GRAPH — same pattern as the
// primekg routes so behaviour/degradation is consistent.
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
    warn!("medication-safety neo4j error: {err}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "neo4j_query_failed", "detail": err.to_string()})),
    )
}

#[derive(Debug, Deserialize)]
struct CheckReq {
    proposed_drug: String,
    #[serde(default)]
    current_drugs: Vec<String>,
    #[serde(default)]
    conditions: Vec<String>,
    /// Optional: only return findings at or above this severity
    /// (minor|moderate|major|contraindicated). Default: return all.
    #[serde(default)]
    min_severity: Option<String>,
}

async fn check(
    State(_pool): State<DbPool>,
    Json(req): Json<CheckReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let svc = neo4j().await.ok_or_else(neo4j_disabled)?;
    let proposed = req.proposed_drug.trim();
    if proposed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "proposed_drug required"})),
        ));
    }

    let pruner = PrimeKgPruner::new(svc.as_ref());
    let ctx = PatientContext {
        current_drugs: req.current_drugs,
        conditions: req.conditions,
    };
    let decision = pruner.check(proposed, &ctx).await.map_err(neo4j_error)?;

    let body = match decision {
        Decision::Pass => json!({ "decision": "pass" }),
        Decision::Flag(mut findings) => {
            if let Some(min) = req.min_severity.as_deref().and_then(Severity::parse) {
                findings.retain(|f| f.severity >= min);
            }
            if findings.is_empty() {
                json!({ "decision": "pass", "note": "no findings at or above min_severity" })
            } else {
                let worst = findings.iter().map(|f| f.severity).max().map(|s| s.label());
                let items = serde_json::to_value(&findings).unwrap_or_else(|_| json!([]));
                json!({
                    "decision": "flag",
                    "worst_severity": worst,
                    "findings": items,
                    "severity_note": SEVERITY_NOTE,
                })
            }
        }
        Decision::Unresolved(names) => json!({
            "decision": "unresolved",
            "unresolved": names,
            "note": "could not map these names to PrimeKG nodes — NOT verified; surface to clinician",
        }),
    };
    Ok(Json(body))
}
