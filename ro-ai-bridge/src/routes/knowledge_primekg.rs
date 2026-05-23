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
    http::StatusCode,
    routing::post,
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::warn;

pub fn knowledge_primekg_routes() -> Router<DbPool> {
    Router::new()
        .route("/entity", post(lookup_entity))
        .route("/neighbors", post(neighbors))
        .route("/drug_interactions", post(drug_interactions))
        .route("/disease_drugs", post(disease_drugs))
        .route("/symptom_to_disease", post(symptom_to_disease))
        .route("/path", post(path))
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
    // Fuzzy fallback: substring search has no spell-correction, so a
    // single-char typo ("Amarosis Fugax") misses entirely. When we get 0
    // exact hits, ask the same backend for the closest Jaro-Winkler
    // matches (~0.85+) so callers can render a "Did you mean...?" prompt
    // instead of a dead end. Always returned as a separate field so
    // medical UI surfaces never silently substitute one disease for
    // another — the caller decides whether to act on the suggestion.
    let did_you_mean = if items.is_empty() {
        svc.primekg_fuzzy_suggest(name, req.entity_type.as_deref(), 5)
            .await
            .map_err(neo4j_error)?
    } else {
        Vec::new()
    };
    Ok(Json(json!({
        "items": items,
        "count": items.len(),
        "did_you_mean": did_you_mean,
    })))
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
