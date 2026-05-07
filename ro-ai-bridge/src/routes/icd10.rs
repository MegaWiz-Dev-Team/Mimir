//! Sprint 48 — ICD-10 / ICD-10-TM lookup API
//!
//! Hermodr-bound skill (currently hosted in Mimir until Hermodr standalone
//! repo reaches doc parity). Single tool surface for all 19 Eir agents.
//!
//! Endpoints:
//!   - GET /api/v1/icd10/lookup?q=...&mode=...&locale=...&limit=...
//!         — search by code/term in en/th/both, with mode cascading
//!   - GET /api/v1/icd10/code/:code
//!         — exact code lookup
//!   - GET /api/v1/icd10/sources
//!         — list available source_versions (anamai-moph-2010, future moph-tm-2017, …)
//!
//! Tenant scope:
//!   - Master codes are tenant-scoped via `tenant_id IS NULL` (shared).
//!   - Caller's tenant context still used for audit-trail (logged, not filtered).
//!   - Per-tenant supplemental codes (future) join via `tenant_id = ?` UNION.

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::FromRow;

const DEFAULT_SOURCE_VERSION: &str = "anamai-moph-2010";
const QDRANT_URL: &str = "http://qdrant.asgard-infra.svc:6333";
const QDRANT_COLLECTION: &str = "icd10-th";
const OLLAMA_URL: &str = "http://host.docker.internal:11434";
// BGE-M3 multilingual (dim=1024) — handles Thai semantic well; replaces
// nomic-embed-text (English-tuned, dim=768) per B-48f.2 upgrade. Qdrant
// collection icd10-th was rebuilt with dim=1024 vectors at the same time.
const EMBED_MODEL: &str = "bge-m3";

#[derive(Debug, Serialize, FromRow)]
pub struct IcdMatch {
    pub code: String,
    pub en_label: String,
    pub th_label: Option<String>,
    pub chapter: Option<String>,
    pub block: Option<String>,
    #[serde(rename = "billable")]
    pub billable_flag: bool,
    pub drg_id: Option<String>,
    pub locale_metadata: Option<JsonValue>,
    pub source_version: String,
}

#[derive(Debug, Deserialize)]
pub struct LookupQuery {
    /// Free-form code, label, or phrase.
    pub q: String,
    /// `auto` | `exact` | `prefix` | `naive` (default: `auto` — cascades exact → naive).
    #[serde(default = "default_mode")]
    pub mode: String,
    /// `en` | `th` | `both` (default: `both`).
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Default 10, max 50.
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Override pinned source_version. Defaults to the latest ingested.
    pub source_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CodeQuery {
    /// Override pinned source_version. Defaults to the latest ingested.
    pub source_version: Option<String>,
}

fn default_mode() -> String { "auto".to_string() }
fn default_locale() -> String { "both".to_string() }
fn default_limit() -> u32 { 10 }

#[derive(Debug, Serialize)]
pub struct LookupResponse {
    pub query: String,
    pub mode_used: String,
    pub locale: String,
    pub source_version: String,
    pub count: usize,
    pub results: Vec<IcdMatch>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SourceInfo {
    pub source_version: String,
    pub row_count: i64,
    pub last_ingested: Option<DateTime<Utc>>,
}

pub fn icd10_routes() -> Router<DbPool> {
    Router::new()
        .route("/api/v1/icd10/lookup", get(lookup))
        .route("/api/v1/icd10/code/{code}", get(get_code))
        .route("/api/v1/icd10/sources", get(list_sources))
        .layer(axum::middleware::from_fn(tenant_auth_middleware))
}

// ─── Handlers ───────────────────────────────────────────────────────────────

async fn lookup(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Query(req): Query<LookupQuery>,
) -> Result<Json<LookupResponse>, (StatusCode, String)> {
    let limit = req.limit.clamp(1, 50);
    let source_version = req.source_version.clone()
        .unwrap_or_else(|| DEFAULT_SOURCE_VERSION.to_string());
    let locale = req.locale.as_str();
    if !matches!(locale, "en" | "th" | "both") {
        return Err((StatusCode::BAD_REQUEST,
            format!("invalid locale: {locale} (want en|th|both)")));
    }

    let q = req.q.trim().to_string();
    if q.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "q is required".to_string()));
    }

    // Smart auto cascade: exact → naive → semantic (multilingual via BGE-M3).
    // - Prefix mode skipped: too restrictive for canonical labels with
    //   qualifier prefixes (e.g. "Non-insulin-dependent diabetes mellitus").
    // - Naive + ORDER BY (CHAR_LENGTH ASC, code ASC) puts root codes first.
    // - Semantic via BGE-M3 multilingual (B-48f.2 upgrade) — handles Thai
    //   queries cleanly (e.g. 'ปวดหัว' → R51 Headache via 0.94 cos), so
    //   the prior English-only guard is no longer needed.
    let modes_to_try: Vec<&str> = match req.mode.as_str() {
        "auto" => vec!["exact", "naive", "semantic"],
        "exact" | "prefix" | "naive" | "semantic" => vec![req.mode.as_str()],
        other => return Err((StatusCode::BAD_REQUEST,
            format!("invalid mode: {other} (want auto|exact|prefix|naive|semantic)"))),
    };

    let mut matched_mode = req.mode.clone();
    let mut results: Vec<IcdMatch> = Vec::new();

    for mode in &modes_to_try {
        let rows = if *mode == "semantic" {
            // Best-effort: don't fail the whole call if Qdrant/Ollama down.
            match run_semantic(&q, &source_version, limit).await {
                Ok(rs) => rs,
                Err(e) => {
                    tracing::warn!(event = "icd10_semantic_fail", err = %e);
                    vec![]
                }
            }
        } else {
            run_query(&pool, &q, mode, locale, &source_version, limit).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?
        };
        if !rows.is_empty() {
            matched_mode = mode.to_string();
            results = rows;
            break;
        }
    }

    tracing::info!(
        event = "icd10_lookup",
        tenant = %tenant.tenant_id,
        q = %q,
        mode_used = %matched_mode,
        locale = %locale,
        count = results.len(),
        source_version = %source_version,
    );

    Ok(Json(LookupResponse {
        query: q,
        mode_used: matched_mode,
        locale: locale.to_string(),
        source_version,
        count: results.len(),
        results,
    }))
}

async fn run_query(
    pool: &DbPool,
    q: &str,
    mode: &str,
    locale: &str,
    source_version: &str,
    limit: u32,
) -> Result<Vec<IcdMatch>, sqlx::Error> {
    // Build label-column WHERE fragment based on locale.
    let label_cols: &[&str] = match locale {
        "en" => &["en_label"],
        "th" => &["th_label"],
        _    => &["en_label", "th_label"],
    };

    let (code_op, label_op, bind_value): (&str, &str, String) = match mode {
        "exact"  => ("=",     "=",     q.to_string()),
        "prefix" => ("LIKE",  "LIKE",  format!("{}%", q)),
        "naive"  => ("LIKE",  "LIKE",  format!("%{}%", q)),
        _ => return Ok(vec![]),
    };

    let mut clauses: Vec<String> = vec![format!("code {} ?", code_op)];
    for col in label_cols {
        clauses.push(format!("{col} {label_op} ?"));
    }
    let where_or = clauses.join(" OR ");

    // Bind count = 1 (code) + label_cols.len() + 2 (relevance hints) + source_version
    let sql = format!(
        "SELECT code, en_label, th_label, chapter, block, billable_flag,
                drg_id, locale_metadata, source_version
           FROM icd10_codes
          WHERE ({where_or})
            AND source_version = ?
            AND tenant_id IS NULL
       ORDER BY (code = ?) DESC,
                (code LIKE ?) DESC,
                CHAR_LENGTH(code) ASC,
                code ASC
          LIMIT ?"
    );

    let mut q_builder = sqlx::query_as::<_, IcdMatch>(&sql);
    // Code op binding
    q_builder = q_builder.bind(&bind_value);
    // Label ops
    for _ in label_cols {
        q_builder = q_builder.bind(&bind_value);
    }
    // Source version
    q_builder = q_builder.bind(source_version);
    // Relevance hint binds: exact-code, prefix-code (uses raw q + q% always)
    q_builder = q_builder.bind(q);
    q_builder = q_builder.bind(format!("{}%", q));
    // Limit
    q_builder = q_builder.bind(limit as i64);

    q_builder.fetch_all(pool).await
}

async fn get_code(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(code): Path<String>,
    Query(req): Query<CodeQuery>,
) -> Result<Json<IcdMatch>, (StatusCode, String)> {
    let source_version = req.source_version.clone()
        .unwrap_or_else(|| DEFAULT_SOURCE_VERSION.to_string());

    let row = sqlx::query_as::<_, IcdMatch>(
        "SELECT code, en_label, th_label, chapter, block, billable_flag,
                drg_id, locale_metadata, source_version
           FROM icd10_codes
          WHERE code = ? AND source_version = ? AND tenant_id IS NULL"
    )
    .bind(&code)
    .bind(&source_version)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, format!("code not found: {code}")));
    };

    tracing::info!(
        event = "icd10_get_code",
        tenant = %tenant.tenant_id,
        code = %code,
        source_version = %source_version,
    );

    Ok(Json(row))
}

async fn list_sources(
    State(pool): State<DbPool>,
    Extension(_tenant): Extension<TenantContext>,
) -> Result<Json<Vec<SourceInfo>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, SourceInfo>(
        "SELECT source_version,
                COUNT(*) AS row_count,
                MAX(updated_at) AS last_ingested
           FROM icd10_codes
          WHERE tenant_id IS NULL
       GROUP BY source_version
       ORDER BY last_ingested DESC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;

    Ok(Json(rows))
}

// ─── Semantic search (Qdrant + Ollama nomic-embed-text) ─────────────────────

/// Detect Thai chars (incl. PUA range used by old MoPH PDFs).
#[allow(dead_code)]
fn has_thai(s: &str) -> bool {
    s.chars().any(|c| {
        let n = c as u32;
        (0x0E00..=0x0E7F).contains(&n) || (0xF700..=0xF71F).contains(&n)
    })
}

/// Medical-acronym expansion — fixes the BGE-M3 acronym blind spot
/// (e.g. STEMI, MI, T2DM aren't tokens BGE-M3 understands as medical
/// terms). Run before embedding so the semantic search sees the
/// expanded form. Case-insensitive token match preserves rest of query.
fn expand_acronyms(query: &str) -> String {
    const PAIRS: &[(&str, &str)] = &[
        // Cardio
        ("STEMI", "ST elevation myocardial infarction"),
        ("NSTEMI", "Non ST elevation myocardial infarction"),
        ("MI", "myocardial infarction"),
        ("AMI", "acute myocardial infarction"),
        ("CHF", "congestive heart failure"),
        ("CABG", "coronary artery bypass graft"),
        ("AFIB", "atrial fibrillation"),
        ("AF", "atrial fibrillation"),
        ("DVT", "deep vein thrombosis"),
        ("PE", "pulmonary embolism"),
        ("HTN", "hypertension"),
        // Pulm
        ("COPD", "chronic obstructive pulmonary disease"),
        ("URTI", "upper respiratory tract infection"),
        ("ARDS", "acute respiratory distress syndrome"),
        ("PNA", "pneumonia"),
        // Endo / metabolic
        ("T1DM", "type 1 diabetes mellitus"),
        ("T2DM", "type 2 diabetes mellitus"),
        ("DM", "diabetes mellitus"),
        ("DKA", "diabetic ketoacidosis"),
        // Neuro
        ("CVA", "cerebrovascular accident stroke"),
        ("TIA", "transient ischemic attack"),
        // Renal
        ("AKI", "acute kidney injury"),
        ("CKD", "chronic kidney disease"),
        ("ESRD", "end stage renal disease"),
        ("UTI", "urinary tract infection"),
        // GI / liver
        ("GERD", "gastroesophageal reflux disease"),
        ("IBD", "inflammatory bowel disease"),
        ("GIB", "gastrointestinal bleeding"),
        // Pediatrics / OB
        ("RDS", "respiratory distress syndrome"),
        ("PROM", "premature rupture of membranes"),
        // Psych
        ("MDD", "major depressive disorder"),
        ("GAD", "generalized anxiety disorder"),
        ("PTSD", "post traumatic stress disorder"),
        ("OCD", "obsessive compulsive disorder"),
    ];
    let mut out = String::with_capacity(query.len() + 32);
    let mut changed = false;
    for token in query.split_inclusive(char::is_whitespace) {
        // Strip trailing whitespace + punctuation for match.
        let trimmed = token.trim_end();
        let suffix = &token[trimmed.len()..];
        let core: String = trimmed.chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let punct = &trimmed[core.len()..];
        let upper = core.to_ascii_uppercase();
        if let Some(&(_, full)) = PAIRS.iter().find(|(k, _)| *k == upper) {
            out.push_str(full);
            out.push_str(punct);
            out.push_str(suffix);
            changed = true;
        } else {
            out.push_str(token);
        }
    }
    if changed { out } else { query.to_string() }
}

async fn run_semantic(
    query: &str,
    source_version: &str,
    limit: u32,
) -> Result<Vec<IcdMatch>, anyhow::Error> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    // Expand medical acronyms BEFORE embedding so STEMI/NSTEMI/etc. become
    // semantically meaningful for the embedder (BGE-M3 doesn't grok acronyms).
    let expanded = expand_acronyms(query);

    // 1. Embed via Ollama.
    let embed_resp: serde_json::Value = client
        .post(format!("{}/api/embeddings", OLLAMA_URL))
        .json(&json!({"model": EMBED_MODEL, "prompt": expanded}))
        .send().await?
        .error_for_status()?
        .json().await?;
    let vec_arr = embed_resp.get("embedding")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("ollama: missing embedding field"))?;
    let vector: Vec<f32> = vec_arr.iter()
        .filter_map(|v| v.as_f64().map(|x| x as f32))
        .collect();

    // 2. Qdrant search.
    let qdrant_body = json!({
        "vector": vector,
        "limit": limit,
        "with_payload": true,
        "filter": {"must": [
            {"key": "source_version", "match": {"value": source_version}}
        ]}
    });
    let qdrant_resp: serde_json::Value = client
        .post(format!("{}/collections/{}/points/search",
            QDRANT_URL, QDRANT_COLLECTION))
        .json(&qdrant_body)
        .send().await?
        .error_for_status()?
        .json().await?;
    let hits = qdrant_resp.get("result")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("qdrant: missing result field"))?;

    let mut out = Vec::with_capacity(hits.len());
    for h in hits {
        let p = h.get("payload").cloned().unwrap_or_else(|| json!({}));
        out.push(IcdMatch {
            code: p.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            en_label: p.get("en_label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            th_label: p.get("th_label").and_then(|v| v.as_str()).map(|s| s.to_string()),
            chapter: p.get("chapter").and_then(|v| v.as_str()).map(|s| s.to_string()),
            block: None,
            billable_flag: true,
            drg_id: None,
            locale_metadata: Some(json!({"semantic_score": h.get("score").cloned().unwrap_or(json!(null))})),
            source_version: p.get("source_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        });
    }
    Ok(out)
}
