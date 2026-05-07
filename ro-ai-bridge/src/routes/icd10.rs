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
use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

const DEFAULT_SOURCE_VERSION: &str = "anamai-moph-2010";

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
    /// `auto` | `exact` | `prefix` | `naive` (default: `auto` — cascades exact → prefix → naive).
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
    pub last_ingested: Option<chrono::NaiveDateTime>,
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

    // Smart auto cascade: exact → naive (skip prefix).
    // Prefix mode is too restrictive — misses canonical labels with qualifier
    // prefixes (e.g. "Non-insulin-dependent diabetes mellitus" doesn't start
    // with "Diabetes mellitus"). Naive + smart ranking (CHAR_LENGTH ASC,
    // code ASC) puts root codes first and correctly orders E11 before O24.
    let modes_to_try: Vec<&str> = match req.mode.as_str() {
        "auto" => vec!["exact", "naive"],
        "exact" | "prefix" | "naive" => vec![req.mode.as_str()],
        other => return Err((StatusCode::BAD_REQUEST,
            format!("invalid mode: {other} (want auto|exact|prefix|naive)"))),
    };

    let mut matched_mode = req.mode.clone();
    let mut results: Vec<IcdMatch> = Vec::new();

    for mode in &modes_to_try {
        let rows = run_query(&pool, &q, mode, locale, &source_version, limit).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;
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
    Query(req): Query<LookupQuery>,
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
