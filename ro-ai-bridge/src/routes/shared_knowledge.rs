//! Shared Knowledge routes — read-only catalog of master/shared knowledge bases.
//!
//! Per `feedback_no_new_norse_components`, this is **not a new component** —
//! it's a Mimir surface that aggregates the existing shared master stores
//! (ICD-10-TM in MariaDB+Qdrant, PrimeKG in Neo4j+Qdrant, LOINC in MariaDB)
//! into one read-only catalog that the UI can render.
//!
//! Tenant model: every entry has `tenant_id=null` — these are global, not
//! per-tenant. Per-tenant ingest sources still go through `/api/v1/tenants/...`
//! and `/api/v1/sources/...`.
//!
//! Routes:
//!   GET /api/v1/knowledge/shared        — list all shared KBs with stats
//!
//! Future:
//!   GET /api/v1/knowledge/shared/:id    — details + sample entries
//!   POST /api/v1/knowledge/shared/:id/refresh — trigger re-ingest/re-embed

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use serde::Serialize;
use serde_json::json;

pub fn shared_knowledge_routes() -> Router<DbPool> {
    Router::new().route("/", get(list_shared_kbs))
}

#[derive(Debug, Serialize)]
struct SharedKb {
    /// Stable identifier used in UI URLs.
    id: &'static str,
    /// Human-readable display name.
    name: &'static str,
    /// One-line description.
    description: &'static str,
    /// Logical kind: `ontology`, `graph_ontology`, `terminology`.
    kind: &'static str,
    /// Where this KB lives: `mariadb`, `qdrant`, `neo4j`, or combinations.
    stores: Vec<&'static str>,
    /// Live row/node/point counts. Missing values → store unreachable.
    counts: serde_json::Value,
    /// Source URL or attribution.
    source: &'static str,
    /// Latest source_version label (e.g. "anamai-moph-2010"). Null if not ingested.
    source_version: Option<String>,
    /// Lifecycle: `active`, `pending_data`, `degraded`.
    status: &'static str,
    /// Free-form notes for the operator (deprecations, refresh cadence).
    notes: Option<&'static str>,
}

async fn list_shared_kbs(
    State(pool): State<DbPool>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut kbs: Vec<SharedKb> = Vec::new();

    // ── ICD-10-TM (MariaDB master + Qdrant icd10-th) ──────────────────────
    let icd10_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM icd10_codes WHERE tenant_id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    let icd10_version: Option<String> = sqlx::query_scalar(
        "SELECT source_version FROM icd10_codes WHERE tenant_id IS NULL \
         ORDER BY updated_at DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();
    let icd10_qdrant_count = qdrant_collection_points("icd10-th").await;
    kbs.push(SharedKb {
        id: "icd10-tm",
        name: "ICD-10-TM (Thai)",
        description: "Thai-localized ICD-10 master from MoPH anamai 2010. \
                      Powers FHIR Condition.code + diagnosis cascade lookup.",
        kind: "ontology",
        stores: vec!["mariadb", "qdrant"],
        counts: json!({
            "mariadb_codes":  icd10_count,
            "qdrant_points":  icd10_qdrant_count,
        }),
        source: "https://backenddc.anamai.moph.go.th/coverpage/...",
        source_version: icd10_version,
        status: if icd10_count > 0 { "active" } else { "pending_data" },
        notes: Some("Refresh path: MoPH ICD-10-TM 2017 license response \
                    (B-48a, on hold per user direction 2026-05-18)."),
    });

    // ── PrimeKG (Neo4j source-of-truth + Qdrant primekg-entities) ──────────
    let pkg_neo4j_count = primekg_neo4j_count().await;
    let pkg_qdrant_count = qdrant_collection_points("primekg-entities").await;
    let pkg_edges = primekg_edge_count().await;
    let pkg_status = if pkg_neo4j_count > 0 && pkg_qdrant_count > 0 {
        "active"
    } else if pkg_neo4j_count > 0 && pkg_qdrant_count == 0 {
        "degraded"
    } else {
        "pending_data"
    };
    kbs.push(SharedKb {
        id: "primekg",
        name: "PrimeKG (Biomedical Knowledge Graph)",
        description: "Harvard PrimeKG v2 — 129K biomedical entities \
                      (disease/drug/gene/anatomy) + 8.1M relations. \
                      Powers Hermodr MCP graph tools + GraphRAG.",
        kind: "graph_ontology",
        stores: vec!["neo4j", "qdrant"],
        counts: json!({
            "neo4j_nodes": pkg_neo4j_count,
            "neo4j_edges": pkg_edges,
            "qdrant_points": pkg_qdrant_count,
        }),
        source: "https://dataverse.harvard.edu/dataset.xhtml?persistentId=doi:10.7910/DVN/IXA7BM",
        source_version: Some("primekg-v2".into()),
        status: pkg_status,
        notes: Some("Vector dim 1024 (BGE-M3 via Heimdall). \
                    Re-import via Mimir/scripts/primekg_import.sh."),
    });

    // ── LOINC (MariaDB master, FHIR Observation.code binding) ──────────────
    let loinc_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM loinc_codes WHERE tenant_id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    let loinc_version: Option<String> = sqlx::query_scalar(
        "SELECT source_version FROM loinc_codes WHERE tenant_id IS NULL \
         ORDER BY updated_at DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();
    kbs.push(SharedKb {
        id: "loinc",
        name: "LOINC (Lab/Observation Codes)",
        description: "Logical Observation Identifiers Names and Codes. \
                      Powers FHIR Observation.code (labs, vitals, imaging).",
        kind: "ontology",
        stores: vec!["mariadb"],
        counts: json!({ "mariadb_codes": loinc_count }),
        source: "https://loinc.org/downloads/",
        source_version: loinc_version,
        status: if loinc_count > 0 { "active" } else { "pending_data" },
        notes: Some("Free under LOINC license; manual account download. \
                    Schema ready (sprint49); see W2.3a runbook for ingest."),
    });

    // ── TMT (Thai Medicines Terminology, dm+d-style 8-layer ontology) ──────
    let tmt_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tmt_codes WHERE tenant_id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    let tmt_rel_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tmt_relationships WHERE tenant_id IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    let tmt_version: Option<String> = sqlx::query_scalar(
        "SELECT source_version FROM tmt_codes WHERE tenant_id IS NULL \
         ORDER BY updated_at DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();
    kbs.push(SharedKb {
        id: "tmt",
        name: "TMT (Thai Medicines Terminology)",
        description: "Thai dm+d-style drug ontology from THIS-Center / MoPH. \
                      8 concept layers (SUBS→VTM→GP→GPP→GPU→TP→TPP→TPU). \
                      Powers FHIR MedicationRequest.medicationCodeableConcept.",
        kind: "terminology",
        stores: vec!["mariadb"],
        counts: json!({
            "mariadb_concepts":      tmt_count,
            "mariadb_relationships": tmt_rel_count,
        }),
        source: "https://this.or.th/",
        source_version: tmt_version,
        status: if tmt_count > 0 { "active" } else { "pending_data" },
        notes: Some("Free download from THIS-Center (this.or.th). \
                    Ingest via scripts/tmt_ingest.py (W2.3b)."),
    });

    // ── TPC (Thai Procedural Classification) — license-blocked stub ────────
    kbs.push(SharedKb {
        id: "tpc",
        name: "TPC (Thai Procedural Classification)",
        description: "Thai procedure code master. \
                      Powers FHIR Procedure.code under Thai profile.",
        kind: "terminology",
        stores: vec![],
        counts: json!({}),
        source: "MoPH (license required)",
        source_version: None,
        status: "pending_data",
        notes: Some("Same license block as TMT (W2.3c)."),
    });

    Ok(Json(json!({ "items": kbs, "count": kbs.len() })))
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Best-effort GET of a Qdrant collection's points_count. Returns 0 on any
/// failure — this endpoint is informational, never blocking.
async fn qdrant_collection_points(name: &str) -> i64 {
    let base = std::env::var("QDRANT_URL").unwrap_or_else(|_| {
        "http://localhost:6333".to_string()
    });
    let url = format!("{}/collections/{}", base.trim_end_matches('/'), name);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();
    let Ok(client) = client else { return 0 };
    let Ok(resp) = client.get(&url).send().await else { return 0 };
    if !resp.status().is_success() {
        return 0;
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else { return 0 };
    body.pointer("/result/points_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

/// Best-effort PrimeKG node count via Neo4j. Returns 0 if Neo4j unreachable.
async fn primekg_neo4j_count() -> i64 {
    let cfg = Neo4jConfig::from_env();
    let Some(svc) = Neo4jService::try_new(&cfg).await else { return 0 };
    svc.count_primekg_nodes().await.unwrap_or(0)
}

/// Best-effort PrimeKG edge count. We expose this for the catalog row;
/// the actual graph queries go through the dedicated graph routes.
async fn primekg_edge_count() -> i64 {
    let cfg = Neo4jConfig::from_env();
    let Some(svc) = Neo4jService::try_new(&cfg).await else { return 0 };
    svc.count_primekg_edges().await.unwrap_or(0)
}
