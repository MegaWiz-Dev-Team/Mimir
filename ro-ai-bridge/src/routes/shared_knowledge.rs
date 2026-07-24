//! Shared Knowledge routes — read-only catalog of master/shared knowledge bases.
//!
//! Per `feedback_no_new_norse_components`, this is **not a new component** —
//! it's a Mimir surface that aggregates the existing shared master stores
//! (ICD-10-TM, PrimeKG, PubMed, LOINC, TMT, RxNorm, TMLT, SNOMED, TPC) into one
//! read-only catalog that the UI can render with rich metadata.
//!
//! Tenant model: every entry has `tenant_id=null` — these are global, not
//! per-tenant. Per-tenant ingest sources still go through `/api/v1/tenants/...`
//! and `/api/v1/sources/...`.
//!
//! Routes:
//!   GET /api/v1/knowledge/shared        — list all shared KBs with metadata

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use serde::Serialize;
use serde_json::json;

pub fn shared_knowledge_routes() -> Router<DbPool> {
    Router::new().route("/", get(list_shared_kbs))
}

/// Static, hand-authored metadata for a shared KB. Lives next to the code
/// because operators care about license terms / FHIR bindings / refresh
/// cadence — these are stable facts that don't change every release.
#[derive(Debug, Serialize)]
struct SharedKbMeta {
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
    /// Source URL or attribution.
    source_url: &'static str,
    /// Organization that owns/maintains the master data.
    maintainer: &'static str,
    /// Region / scope: "TH", "INTL", "US", etc.
    region: &'static str,
    /// Languages present in the FSN/label fields.
    languages: Vec<&'static str>,
    /// Release year of the *current* loaded vintage (e.g. 2010 for anamai,
    /// 2024 for LOINC 2.82, 2026 for TMT/TMLT releases).
    vintage_year: Option<i32>,
    /// License terms — short form for display.
    license: &'static str,
    /// FHIR resource.field this KB powers, if any.
    fhir_binding: Option<&'static str>,
    /// Release cadence the upstream publishes at.
    update_cadence: &'static str,
    /// Which Mimir sprint shipped the schema for this KB.
    schema_version: &'static str,
    /// Notes for operator (gaps, deprecations).
    notes: Option<&'static str>,
}

/// Live state for a KB — counts, status, last refresh, source_version
/// of the currently-loaded data. Derived from live queries, not hand-coded.
#[derive(Debug, Serialize)]
struct SharedKbLive {
    /// Live row/node/point counts. Missing values → store unreachable.
    counts: serde_json::Value,
    /// Latest source_version label (e.g. "anamai-moph-2010").
    source_version: Option<String>,
    /// Lifecycle: `active`, `active_fallback`, `degraded`, `pending_data`.
    status: &'static str,
    /// Timestamp of last successful ingest (UTC, ISO-8601).
    last_local_refresh: Option<String>,
}

#[derive(Debug, Serialize)]
struct SharedKbEntry {
    #[serde(flatten)]
    meta: SharedKbMeta,
    #[serde(flatten)]
    live: SharedKbLive,
}

async fn list_shared_kbs(
    State(pool): State<DbPool>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut kbs: Vec<SharedKbEntry> = Vec::new();

    // ── ICD-10-TM ──────────────────────────────────────────────────────────
    {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM icd10_codes WHERE tenant_id IS NULL",
        )
        .fetch_one(&pool).await.unwrap_or(0);
        let qdrant_count = qdrant_collection_points("icd10-th").await;
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM icd10_codes WHERE tenant_id IS NULL \
             ORDER BY updated_at DESC LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "icd10_ingest_runs").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "icd10-tm",
                name: "ICD-10-TM (Thai)",
                description: "Thai-localized ICD-10 diagnosis master. Cascade exact → naive → semantic lookup.",
                kind: "ontology",
                stores: vec!["mariadb", "qdrant"],
                source_url: "https://backenddc.anamai.moph.go.th/coverpage/d1579eb1c80b878ab62513c060681290.pdf",
                maintainer: "MoPH Bureau of Health Information (anamai)",
                region: "TH",
                languages: vec!["en", "th"],
                vintage_year: Some(2010),
                license: "Thai government document (public)",
                fhir_binding: Some("Condition.code"),
                update_cadence: "Irregular (2010 → 2017 pending B-48a license)",
                schema_version: "sprint48",
                notes: Some("Refresh target: ICD-10-TM 2017 from MoPH. Awaiting license letter."),
            },
            live: SharedKbLive {
                counts: json!({ "mariadb_codes": count, "qdrant_points": qdrant_count }),
                source_version: version,
                status: if count > 0 { "active" } else { "pending_data" },
                last_local_refresh: refresh,
            },
        });
    }

    // ── PrimeKG ────────────────────────────────────────────────────────────
    {
        let neo_count = primekg_neo4j_count().await;
        let qdrant_count = qdrant_collection_points("primekg-entities").await;
        let edges = primekg_edge_count().await;
        // authoritative version from the graph's (:Meta) node; fall back to the known label.
        let version = primekg_version().await.or_else(|| Some("primekg-v2".into()));
        let status = if neo_count > 0 && qdrant_count > 0 {
            "active"
        } else if neo_count > 0 {
            "degraded"
        } else {
            "pending_data"
        };
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "primekg",
                name: "PrimeKG (Biomedical Knowledge Graph)",
                description: "Harvard PrimeKG — 129K biomedical entities (disease/drug/gene/anatomy) + 8.1M relations. Powers GraphRAG + Hermodr MCP tools.",
                kind: "graph_ontology",
                stores: vec!["neo4j", "qdrant"],
                source_url: "https://dataverse.harvard.edu/dataset.xhtml?persistentId=doi:10.7910/DVN/IXA7BM",
                maintainer: "Harvard Dataverse (Marinka Zitnik lab)",
                region: "INTL",
                languages: vec!["en"],
                vintage_year: Some(2022),
                license: "MIT (research)",
                fhir_binding: None,
                update_cadence: "Ad-hoc (v1 2021, v2 2022, no v3 announced)",
                schema_version: "sprint48",
                notes: Some("Vector dim 1024 (BGE-M3 via Heimdall). Source: data/PrimeKG/kg.csv."),
            },
            live: SharedKbLive {
                counts: json!({
                    "neo4j_nodes":   neo_count,
                    "neo4j_edges":   edges,
                    "qdrant_points": qdrant_count,
                }),
                source_version: version,
                status,
                last_local_refresh: None,
            },
        });
    }

    // ── PubMed (abstracts) ───────────────────────────────────────────────────
    //
    // Qdrant-only literature corpus. Mixes a one-off BigQuery PMC open-access
    // bulk load with topic-targeted E-utilities backfills (each point tagged
    // `topic`). No MariaDB table / ingest_runs, so counts come straight from
    // Qdrant and there's no last_local_refresh to report.
    {
        let qdrant_count = qdrant_collection_points("pubmed-abstracts").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "pubmed",
                name: "PubMed (Abstracts)",
                description: "Biomedical literature corpus (titles + abstracts / PMC open-access full text). Semantic search via BGE-M3; powers the unified knowledge search + the research agent's lit_search. Topic-tagged (e.g. sleep_osa_cpap, cardiology).",
                kind: "literature",
                stores: vec!["qdrant"],
                source_url: "https://pubmed.ncbi.nlm.nih.gov/",
                maintainer: "NCBI / U.S. National Library of Medicine",
                region: "INTL",
                languages: vec!["en"],
                vintage_year: None,
                license: "PubMed metadata/abstracts per NLM terms; PMC subset = open-access (commercial-use tier)",
                fhir_binding: None,
                update_cadence: "Rolling — topic backfill via NCBI E-utilities (idempotent by PMID); daily incremental CronJob available",
                schema_version: "wave4b",
                notes: Some("Dense-only (BGE-M3 1024-d); no BM25 sparse yet, so served by dense search not hybrid. Point id = uuid5('pubmed:{pmid}') → re-runs never duplicate. Ingest: scripts/sync_pubmed_incremental.py (PUBMED_QUERY/PUBMED_TOPIC)."),
            },
            live: SharedKbLive {
                counts: json!({ "qdrant_points": qdrant_count }),
                source_version: Some("pmc-oa-commercial + ncbi-eutils".into()),
                status: if qdrant_count > 0 { "active" } else { "pending_data" },
                last_local_refresh: None,
            },
        });
    }

    // ── LOINC ──────────────────────────────────────────────────────────────
    {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loinc_codes WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM loinc_codes WHERE tenant_id IS NULL \
             ORDER BY updated_at DESC LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "loinc_ingest_runs").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "loinc",
                name: "LOINC (Lab/Observation Codes)",
                description: "Logical Observation Identifiers Names and Codes. Universal coding for labs, vitals, imaging, surveys.",
                kind: "ontology",
                stores: vec!["mariadb"],
                source_url: "https://loinc.org/downloads/",
                maintainer: "Regenstrief Institute (US)",
                region: "INTL",
                languages: vec!["en"],
                vintage_year: Some(2024),
                license: "Free under LOINC license (account required)",
                fhir_binding: Some("Observation.code"),
                update_cadence: "Biannual (Feb / Aug)",
                schema_version: "sprint49",
                notes: Some("Pair with TMLT for Thai display. ~98K codes (incl. deprecated/trial)."),
            },
            live: SharedKbLive {
                counts: json!({ "mariadb_codes": count }),
                source_version: version,
                status: if count > 0 { "active" } else { "pending_data" },
                last_local_refresh: refresh,
            },
        });
    }

    // ── TMT ────────────────────────────────────────────────────────────────
    {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tmt_codes WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let rel_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tmt_relationships WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM tmt_codes WHERE tenant_id IS NULL \
             ORDER BY updated_at DESC LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "tmt_ingest_runs").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "tmt",
                name: "TMT (Thai Medicines Terminology)",
                description: "Thai dm+d-style drug ontology. 8 layers SUBS→VTM→GP→GPP→GPU→TP→TPP→TPU with brand/generic linkage.",
                kind: "terminology",
                stores: vec!["mariadb"],
                source_url: "https://this.or.th/",
                maintainer: "THIS-Center / MoPH",
                region: "TH",
                languages: vec!["en", "th"],
                vintage_year: Some(2026),
                license: "Free (THIS-Center)",
                fhir_binding: Some("MedicationRequest.medicationCodeableConcept"),
                update_cadence: "Monthly releases (TMTRF<YYYYMMDD>)",
                schema_version: "sprint50",
                notes: Some("8 concept layers + 11 relationship types unified into 2 tables."),
            },
            live: SharedKbLive {
                counts: json!({
                    "mariadb_concepts":      count,
                    "mariadb_relationships": rel_count,
                }),
                source_version: version,
                status: if count > 0 { "active" } else { "pending_data" },
                last_local_refresh: refresh,
            },
        });
    }

    // ── RxNorm (drug normalizer) ─────────────────────────────────────────────
    {
        let atoms: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rxnorm_atoms WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let rel_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rxnorm_rel WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let unii: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rxnorm_unii WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM rxnorm_ingest_runs WHERE status='DONE' \
             ORDER BY created_at DESC LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "rxnorm_ingest_runs").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "rxnorm",
                name: "RxNorm (Drug Normalizer)",
                description: "US drug vocabulary — brand→ingredient crosswalk + UNII/DrugBank-id keys. Normalizes drug names to the PrimeKG-canonical form for the medication-safety pruner.",
                kind: "terminology",
                stores: vec!["mariadb"],
                source_url: "https://www.nlm.nih.gov/research/umls/rxnorm/",
                maintainer: "US NLM (National Library of Medicine)",
                region: "INTL",
                languages: vec!["en"],
                vintage_year: Some(2026),
                license: "Public domain (US NLM) — SAB=RXNORM core ships",
                fhir_binding: Some("MedicationRequest.medicationCodeableConcept"),
                update_cadence: "Monthly full release (RxNorm_full_MMDDYYYY)",
                schema_version: "sprint55",
                notes: Some("Feeds DrugDiseaseNormalizer (brand→ingredient) + rxnorm_primekg_bridge (drugbank_id→PrimeKG node). DrugBank curated content excluded; only IDs/keys used."),
            },
            live: SharedKbLive {
                counts: json!({
                    "mariadb_atoms":         atoms,
                    "mariadb_relationships": rel_count,
                    "mariadb_unii":          unii,
                }),
                source_version: version,
                status: if atoms > 0 { "active" } else { "pending_data" },
                last_local_refresh: refresh,
            },
        });
    }

    // ── TMLT ───────────────────────────────────────────────────────────────
    {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tmlt_codes WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let links: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tmlt_relationships WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM tmlt_codes WHERE tenant_id IS NULL \
             ORDER BY updated_at DESC LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "tmlt_ingest_runs").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "tmlt",
                name: "TMLT (Thai Medical Laboratory Terminology)",
                description: "Thai lab/observation companion to LOINC. ITEM + PANEL layers with Thai display names.",
                kind: "terminology",
                stores: vec!["mariadb"],
                source_url: "https://this.or.th/",
                maintainer: "THIS-Center / MoPH",
                region: "TH",
                languages: vec!["en", "th"],
                vintage_year: Some(2026),
                license: "Free (THIS-Center)",
                fhir_binding: Some("Observation.code (Thai display layer)"),
                update_cadence: "Ad-hoc releases (TMLTRF<YYYYMMDD>)",
                schema_version: "sprint51",
                notes: Some("Used with LOINC: LOINC for international wire, TMLT for Thai UI."),
            },
            live: SharedKbLive {
                counts: json!({
                    "mariadb_concepts":   count,
                    "mariadb_panel_links": links,
                }),
                source_version: version,
                status: if count > 0 { "active" } else { "pending_data" },
                last_local_refresh: refresh,
            },
        });
    }

    // ── SNOMED CT (concepts + ICD-10-TM map) ─────────────────────────────────
    {
        let descriptions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM snomed_descriptions WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let map_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM snomed_icd10_map WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let mapped_concepts: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT concept_id) FROM snomed_icd10_map \
             WHERE tenant_id IS NULL AND icd10_tm IS NOT NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM snomed_icd10_map WHERE tenant_id IS NULL LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "snomed_map_ingest_runs").await;
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "snomed",
                name: "SNOMED CT → ICD-10-TM",
                description: "SNOMED CT concept descriptions + ICD-10-TM crosswalk. Resolves clinical text → SNOMED concept → ICD-10-TM (WHO→TM bridge self-derived from icd10_codes).",
                kind: "terminology",
                stores: vec!["mariadb"],
                source_url: "https://mlds.ihtsdotools.org/",
                maintainer: "SNOMED International (Affiliate) / MoPH ExtendedMap transform",
                region: "INT",
                languages: vec!["en"],
                vintage_year: Some(2026),
                license: "SNOMED Affiliate License — restricted (commercial_use requires affiliate; see license docs)",
                fhir_binding: Some("Condition.code / ConceptMap (SNOMED→ICD-10)"),
                update_cadence: "Biannual International Edition (must upgrade ≤180d per Affiliate License clause 6.2)",
                schema_version: "sprint54",
                notes: Some("POC: resolver at /api/v1/knowledge/snomed/{search,resolve-icd10}. needs_review rows = cannot-classify / context-dependent / external-cause (post-coordination) / TM-absent."),
            },
            live: SharedKbLive {
                counts: json!({
                    "mariadb_descriptions":   descriptions,
                    "mariadb_map_rows":       map_rows,
                    "concepts_mapped_to_tm":  mapped_concepts,
                }),
                source_version: version,
                status: if map_rows > 0 { "active" } else { "pending_data" },
                last_local_refresh: refresh,
            },
        });
    }

    // ── TPC (currently served by US ICD-9-CM upstream baseline) ───────────
    //
    // Naming note (2026-05-19): we briefly used `active_fallback` here to
    // distinguish "serving via baseline ICD-9-CM" from "serving via fully
    // localized Thai TPC". That framing was misleading — TPC IS derived
    // from ICD-9-CM (US baseline + ~200 Thai additions + Thai labels), so
    // the public-domain ICD-9-CM upstream is the *baseline of the same
    // standard*, not a different inferior thing. For 95% of common
    // procedures the codes are identical. Status is now just `active`;
    // the `notes` field describes what would be added by the Thai
    // extension when license arrives.
    {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tpc_codes WHERE tenant_id IS NULL",
        ).fetch_one(&pool).await.unwrap_or(0);
        let version: Option<String> = sqlx::query_scalar(
            "SELECT source_version FROM tpc_codes WHERE tenant_id IS NULL \
             ORDER BY updated_at DESC LIMIT 1",
        ).fetch_optional(&pool).await.ok().flatten();
        let refresh = fetch_last_refresh(&pool, "tpc_ingest_runs").await;
        let is_us_baseline = version.as_deref().map(|v| v.starts_with("icd9cm-")).unwrap_or(false);
        kbs.push(SharedKbEntry {
            meta: SharedKbMeta {
                id: "tpc",
                name: "TPC (Thai Procedural Classification)",
                description: "Procedure code master. US ICD-9-CM Volume 3 (the open-source baseline that Thai TPC extends with ~200 Thai-specific codes + Thai labels). Multi-source-version PK supports merging the Thai extension on top when license arrives.",
                kind: "terminology",
                stores: vec!["mariadb"],
                source_url: if is_us_baseline {
                    "https://www.cms.gov/medicare/coding-billing/icd-10-codes"
                } else {
                    "https://bps.moph.go.th/"
                },
                maintainer: if is_us_baseline {
                    "US CMS (upstream baseline of Thai TPC)"
                } else {
                    "MoPH Bureau of Health Information"
                },
                region: if is_us_baseline { "INTL → TH (extending)" } else { "TH" },
                languages: if is_us_baseline { vec!["en"] } else { vec!["en", "th"] },
                vintage_year: if is_us_baseline { Some(2014) } else { None },
                license: if is_us_baseline {
                    "Public domain (US Government)"
                } else {
                    "MoPH license required"
                },
                fhir_binding: Some("Procedure.code"),
                update_cadence: if is_us_baseline {
                    "Frozen 2015 (US baseline; Thai extension on roadmap)"
                } else {
                    "Irregular"
                },
                schema_version: "sprint52",
                notes: Some("Currently serving the US ICD-9-CM baseline (~95% of common procedures coded identically to Thai TPC). Thai extension adds ~200 codes for traditional medicine + regional procedures + Thai display labels — pending MoPH license."),
            },
            live: SharedKbLive {
                counts: json!({ "mariadb_codes": count }),
                source_version: version,
                status: if count == 0 { "pending_data" } else { "active" },
                last_local_refresh: refresh,
            },
        });
    }

    let count = kbs.len();
    Ok(Json(json!({ "items": kbs, "count": count })))
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Most recent successful ingest timestamp, as ISO-8601 UTC string.
async fn fetch_last_refresh(pool: &DbPool, runs_table: &str) -> Option<String> {
    // Note: query_scalar cannot interpolate table names; build SQL string.
    let sql = format!(
        "SELECT DATE_FORMAT(finished_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM {runs_table} \
         WHERE status='COMPLETED' AND tenant_id IS NULL \
         ORDER BY finished_at DESC LIMIT 1"
    );
    sqlx::query_scalar::<_, String>(&sql)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn qdrant_collection_points(name: &str) -> i64 {
    let base = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let url = format!("{}/collections/{}", base.trim_end_matches('/'), name);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let Ok(resp) = client.get(&url).send().await else { return 0 };
    if !resp.status().is_success() {
        return 0;
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else { return 0 };
    body.pointer("/result/points_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

async fn primekg_neo4j_count() -> i64 {
    let cfg = Neo4jConfig::from_env();
    let Some(svc) = Neo4jService::try_new(&cfg).await else { return 0 };
    svc.count_primekg_nodes().await.unwrap_or(0)
}

async fn primekg_edge_count() -> i64 {
    let cfg = Neo4jConfig::from_env();
    let Some(svc) = Neo4jService::try_new(&cfg).await else { return 0 };
    svc.count_primekg_edges().await.unwrap_or(0)
}

/// Pinned PrimeKG version from the `(:Meta {kb:'primekg'})` node — authoritative over any
/// hardcoded label. `None` if Neo4j is unreachable or the node hasn't been seeded.
async fn primekg_version() -> Option<String> {
    let cfg = Neo4jConfig::from_env();
    let svc = Neo4jService::try_new(&cfg).await?;
    svc.primekg_meta_version().await.ok().flatten()
}
