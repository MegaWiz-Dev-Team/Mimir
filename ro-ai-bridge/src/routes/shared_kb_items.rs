//! Per-KB item browser — Level 2 of the Shared Knowledge UI surface.
//!
//! GET /api/v1/knowledge/shared/{kb_id}/items?q=...&page=N&per_page=N&filter_<field>=v
//!
//! Returns paginated rows from the relational master tables, with a
//! `columns` metadata array so the UI can render any KB with the same
//! generic table component.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::Row;
use std::collections::HashMap;

pub fn shared_kb_items_routes() -> Router<DbPool> {
    Router::new().route("/{kb_id}/items", get(list_items))
}

#[derive(Debug, Deserialize)]
struct ItemsQuery {
    /// Free-text search (FULLTEXT on label/fsn columns where available).
    q: Option<String>,
    /// Pagination — 1-based page number.
    page: Option<u32>,
    /// Items per page (capped at 100).
    per_page: Option<u32>,
    /// Per-KB filters captured here verbatim, dispatched in handler.
    #[serde(flatten)]
    filters: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct Column {
    name: &'static str,
    label: &'static str,
    /// "string" | "code" | "boolean" | "enum"
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct ItemsResponse {
    kb_id: String,
    columns: Vec<Column>,
    items: Vec<JsonValue>,
    total: i64,
    page: u32,
    per_page: u32,
    /// Available filter values per filterable column (small enums only).
    filters: JsonValue,
}

async fn list_items(
    State(pool): State<DbPool>,
    Path(kb_id): Path<String>,
    Query(qp): Query<ItemsQuery>,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let page = qp.page.unwrap_or(1).max(1);
    let per_page = qp.per_page.unwrap_or(20).min(100).max(1);
    let offset = ((page - 1) * per_page) as i64;
    let limit = per_page as i64;

    match kb_id.as_str() {
        "icd10-tm" => icd10_items(&pool, &qp, limit, offset, page, per_page).await,
        "tpc"      => tpc_items(&pool, &qp, limit, offset, page, per_page).await,
        "loinc"    => loinc_items(&pool, &qp, limit, offset, page, per_page).await,
        "tmt"      => tmt_items(&pool, &qp, limit, offset, page, per_page).await,
        "tmlt"     => tmlt_items(&pool, &qp, limit, offset, page, per_page).await,
        "snomed"   => snomed_items(&pool, &qp, limit, offset, page, per_page).await,
        "snomed-icd10cm" => snomed_extmap_items(&pool, &qp, limit, offset, page, per_page, "icd10cm", "snomed-icd10cm", "ICD-10-CM").await,
        "snomed-icd10who" => snomed_extmap_items(&pool, &qp, limit, offset, page, per_page, "icd10who", "snomed-icd10who", "ICD-10 (WHO)").await,
        "medical-abbrev" => abbrev_items(&pool, &qp, limit, offset, page, per_page).await,
        "primekg"  => Err((
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({
                "error": "PrimeKG browser not yet implemented",
                "hint": "PrimeKG is in Neo4j; use GET /api/v1/graph/primekg/entity/{idx}/neighbors for now, or open Neo4j browser at http://localhost:7474"
            })),
        )),
        _ => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Unknown kb_id: {kb_id}")})),
        )),
    }
}

// ── ICD-10-TM ──────────────────────────────────────────────────────────────

async fn icd10_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL");
    if let Some(c) = qp.filters.get("filter_chapter") {
        sql_where.push_str(&format!(" AND chapter = '{}'", sql_safe(c)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (code LIKE '%{q}%' OR en_label LIKE '%{q}%' OR th_label LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM icd10_codes WHERE {sql_where}"
    ))
    .fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT code, en_label, th_label, chapter, billable_flag \
         FROM icd10_codes WHERE {sql_where} ORDER BY code LIMIT {limit} OFFSET {offset}"
    ))
    .fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "code": r.get::<String, _>("code"),
        "en_label": r.get::<String, _>("en_label"),
        "th_label": r.try_get::<String, _>("th_label").unwrap_or_default(),
        "chapter": r.try_get::<String, _>("chapter").unwrap_or_default(),
        "billable": r.get::<bool, _>("billable_flag"),
    })).collect();

    let chapters: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT chapter FROM icd10_codes WHERE tenant_id IS NULL AND chapter IS NOT NULL ORDER BY chapter"
    ).fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ItemsResponse {
        kb_id: "icd10-tm".into(),
        columns: vec![
            Column { name: "code",     label: "Code",        kind: "code" },
            Column { name: "en_label", label: "English",     kind: "string" },
            Column { name: "th_label", label: "Thai",        kind: "string" },
            Column { name: "chapter",  label: "Chapter",     kind: "enum" },
            Column { name: "billable", label: "Billable",    kind: "boolean" },
        ],
        items, total, page, per_page,
        filters: json!({ "chapter": chapters }),
    }))
}

// ── TPC (same shape as icd10) ──────────────────────────────────────────────

async fn tpc_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL");
    if let Some(c) = qp.filters.get("filter_chapter") {
        sql_where.push_str(&format!(" AND chapter = '{}'", sql_safe(c)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (code LIKE '%{q}%' OR en_label LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM tpc_codes WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT code, en_label, chapter, billable_flag \
         FROM tpc_codes WHERE {sql_where} ORDER BY code LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "code": r.get::<String, _>("code"),
        "en_label": r.get::<String, _>("en_label"),
        "chapter": r.try_get::<String, _>("chapter").unwrap_or_default(),
        "billable": r.get::<bool, _>("billable_flag"),
    })).collect();

    let chapters: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT chapter FROM tpc_codes WHERE tenant_id IS NULL AND chapter IS NOT NULL ORDER BY chapter"
    ).fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ItemsResponse {
        kb_id: "tpc".into(),
        columns: vec![
            Column { name: "code",     label: "Code",       kind: "code" },
            Column { name: "en_label", label: "English",    kind: "string" },
            Column { name: "chapter",  label: "Chapter",    kind: "enum" },
            Column { name: "billable", label: "Billable",   kind: "boolean" },
        ],
        items, total, page, per_page,
        filters: json!({ "chapter": chapters }),
    }))
}

// ── Medical Abbreviation Glossary ───────────────────────────────────────────
async fn abbrev_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL");
    if let Some(c) = qp.filters.get("filter_category") {
        sql_where.push_str(&format!(" AND category = '{}'", sql_safe(c)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (abbrev LIKE '%{q}%' OR full_term_en LIKE '%{q}%' OR full_term_th LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM medical_abbrev WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT abbrev, full_term_en, full_term_th, category, icd10tm \
         FROM medical_abbrev WHERE {sql_where} ORDER BY abbrev LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "abbrev": r.get::<String, _>("abbrev"),
        "full_term_en": r.try_get::<String, _>("full_term_en").unwrap_or_default(),
        "full_term_th": r.try_get::<String, _>("full_term_th").unwrap_or_default(),
        "category": r.try_get::<String, _>("category").unwrap_or_default(),
        "icd10tm": r.try_get::<String, _>("icd10tm").unwrap_or_default(),
    })).collect();

    let categories: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT category FROM medical_abbrev WHERE tenant_id IS NULL AND category IS NOT NULL ORDER BY category"
    ).fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ItemsResponse {
        kb_id: "medical-abbrev".into(),
        columns: vec![
            Column { name: "abbrev",       label: "Abbrev",     kind: "code" },
            Column { name: "full_term_en", label: "English",    kind: "string" },
            Column { name: "full_term_th", label: "ไทย",        kind: "string" },
            Column { name: "category",     label: "Category",   kind: "enum" },
            Column { name: "icd10tm",      label: "ICD-10-TM",  kind: "code" },
        ],
        items, total, page, per_page,
        filters: json!({ "category": categories }),
    }))
}

// ── LOINC ──────────────────────────────────────────────────────────────────

async fn loinc_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL");
    if let Some(c) = qp.filters.get("filter_class") {
        sql_where.push_str(&format!(" AND class = '{}'", sql_safe(c)));
    }
    if let Some(s) = qp.filters.get("filter_status") {
        sql_where.push_str(&format!(" AND status = '{}'", sql_safe(s)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (loinc_num LIKE '%{q}%' OR long_common_name LIKE '%{q}%' OR short_name LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM loinc_codes WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT loinc_num, long_common_name, short_name, class, status \
         FROM loinc_codes WHERE {sql_where} ORDER BY loinc_num LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "loinc_num": r.get::<String, _>("loinc_num"),
        "long_common_name": r.get::<String, _>("long_common_name"),
        "short_name": r.try_get::<String, _>("short_name").unwrap_or_default(),
        "class": r.try_get::<String, _>("class").unwrap_or_default(),
        "status": r.try_get::<String, _>("status").unwrap_or_default(),
    })).collect();

    let classes: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT class FROM loinc_codes WHERE tenant_id IS NULL AND class IS NOT NULL ORDER BY class"
    ).fetch_all(pool).await.unwrap_or_default();
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT status FROM loinc_codes WHERE tenant_id IS NULL AND status IS NOT NULL ORDER BY status"
    ).fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ItemsResponse {
        kb_id: "loinc".into(),
        columns: vec![
            Column { name: "loinc_num",        label: "LOINC #",   kind: "code" },
            Column { name: "long_common_name", label: "Long Name", kind: "string" },
            Column { name: "short_name",       label: "Short",     kind: "string" },
            Column { name: "class",            label: "Class",     kind: "enum" },
            Column { name: "status",           label: "Status",    kind: "enum" },
        ],
        items, total, page, per_page,
        filters: json!({ "class": classes, "status": statuses }),
    }))
}

// ── TMT ────────────────────────────────────────────────────────────────────

async fn tmt_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL");
    if let Some(ct) = qp.filters.get("filter_concept_type") {
        sql_where.push_str(&format!(" AND concept_type = '{}'", sql_safe(ct)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (tmt_id LIKE '%{q}%' OR fsn LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM tmt_codes WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT tmt_id, concept_type, fsn, manufacturer \
         FROM tmt_codes WHERE {sql_where} ORDER BY tmt_id LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "tmt_id": r.get::<String, _>("tmt_id"),
        "concept_type": r.get::<String, _>("concept_type"),
        "fsn": r.get::<String, _>("fsn"),
        "manufacturer": r.try_get::<String, _>("manufacturer").unwrap_or_default(),
    })).collect();

    Ok(Json(ItemsResponse {
        kb_id: "tmt".into(),
        columns: vec![
            Column { name: "tmt_id",       label: "TMT ID",       kind: "code" },
            Column { name: "concept_type", label: "Layer",        kind: "enum" },
            Column { name: "fsn",          label: "Name",         kind: "string" },
            Column { name: "manufacturer", label: "Manufacturer", kind: "string" },
        ],
        items, total, page, per_page,
        filters: json!({
            "concept_type": ["SUBS","VTM","GP","GPP","GPU","TP","TPP","TPU"]
        }),
    }))
}

// ── TMLT ───────────────────────────────────────────────────────────────────

async fn tmlt_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL");
    if let Some(ct) = qp.filters.get("filter_concept_type") {
        sql_where.push_str(&format!(" AND concept_type = '{}'", sql_safe(ct)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (tmlt_id LIKE '%{q}%' OR fsn LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM tmlt_codes WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT tmlt_id, concept_type, fsn \
         FROM tmlt_codes WHERE {sql_where} ORDER BY tmlt_id LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "tmlt_id": r.get::<String, _>("tmlt_id"),
        "concept_type": r.get::<String, _>("concept_type"),
        "fsn": r.get::<String, _>("fsn"),
    })).collect();

    Ok(Json(ItemsResponse {
        kb_id: "tmlt".into(),
        columns: vec![
            Column { name: "tmlt_id",      label: "TMLT ID", kind: "code" },
            Column { name: "concept_type", label: "Layer",   kind: "enum" },
            Column { name: "fsn",          label: "Name",    kind: "string" },
        ],
        items, total, page, per_page,
        filters: json!({ "concept_type": ["ITEM", "PANEL"] }),
    }))
}

// ── SNOMED CT (concept descriptions) ─────────────────────────────────────────
//
// Browses `snomed_descriptions` — the text→concept search surface. Each row is
// one description (a concept has many: 1 FSN + synonyms), so the browser lists
// descriptions, not concepts. The map → ICD-10-TM lives behind the POST resolver
// at /api/v1/knowledge/snomed/{search,resolve-icd10}, not here.

async fn snomed_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    let mut sql_where = String::from("tenant_id IS NULL AND active = 1");
    if let Some(st) = qp.filters.get("filter_semantic_tag") {
        sql_where.push_str(&format!(" AND semantic_tag = '{}'", sql_safe(st)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (concept_id LIKE '%{q}%' OR term LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM snomed_descriptions WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT concept_id, term, term_type, semantic_tag \
         FROM snomed_descriptions WHERE {sql_where} \
         ORDER BY concept_id, (term_type = 'fsn') DESC, term LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "concept_id": r.get::<String, _>("concept_id"),
        "term": r.get::<String, _>("term"),
        "term_type": r.get::<String, _>("term_type"),
        "semantic_tag": r.try_get::<String, _>("semantic_tag").unwrap_or_default(),
    })).collect();

    let semantic_tags: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT semantic_tag FROM snomed_descriptions \
         WHERE tenant_id IS NULL AND semantic_tag IS NOT NULL ORDER BY semantic_tag"
    ).fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ItemsResponse {
        kb_id: "snomed".into(),
        columns: vec![
            Column { name: "concept_id",   label: "Concept ID", kind: "code" },
            Column { name: "term",         label: "Term",       kind: "string" },
            Column { name: "term_type",    label: "Type",       kind: "enum" },
            Column { name: "semantic_tag", label: "Semantic Tag", kind: "enum" },
        ],
        items, total, page, per_page,
        filters: json!({ "semantic_tag": semantic_tags }),
    }))
}

// ── SNOMED CT → ICD-10 (rule-based ExtendedMap, US-CM + WHO) ─────────────────
//
// Browses `snomed_icd10_extmap` filtered by target_system — one row per candidate
// (concept, group, priority). The source SNOMED concept FSN is pulled from
// snomed_descriptions via a correlated subquery (LEFT JOIN would multiply rows
// when a concept has synonyms). Resolution order at query time is mapGroup →
// mapPriority; we surface them so an operator can read the rule chain.

async fn snomed_extmap_items(
    pool: &DbPool, qp: &ItemsQuery, limit: i64, offset: i64,
    page: u32, per_page: u32,
    target_system: &'static str, kb_id: &'static str, code_label: &'static str,
) -> Result<Json<ItemsResponse>, (StatusCode, Json<JsonValue>)> {
    // target_system is a trusted internal literal ('icd10cm'|'icd10who').
    let mut sql_where = format!("m.tenant_id IS NULL AND m.target_system = '{target_system}'");
    if let Some(c) = qp.filters.get("filter_map_category") {
        sql_where.push_str(&format!(" AND m.map_category = '{}'", sql_safe(c)));
    }
    if let Some(r) = qp.filters.get("filter_needs_review") {
        // "1" → review only, "0" → properly-classified only.
        sql_where.push_str(&format!(" AND m.needs_review = '{}'", sql_safe(r)));
    }
    if let Some(q) = qp.search() {
        sql_where.push_str(&format!(
            " AND (m.concept_id LIKE '%{q}%' OR m.icd10_code LIKE '%{q}%' OR m.map_advice LIKE '%{q}%')",
            q = sql_safe(&q)
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM snomed_icd10_extmap m WHERE {sql_where}"
    )).fetch_one(pool).await.map_err(db_err)?;
    let rows = sqlx::query(&format!(
        "SELECT m.concept_id, m.icd10_code, m.map_group, m.map_priority, \
                m.map_rule, m.map_advice, m.map_category, m.needs_review, \
                (SELECT d.term FROM snomed_descriptions d \
                 WHERE d.concept_id = m.concept_id AND d.term_type = 'fsn' AND d.active = 1 \
                 LIMIT 1) AS term \
         FROM snomed_icd10_extmap m WHERE {sql_where} \
         ORDER BY m.concept_id, m.map_group, m.map_priority LIMIT {limit} OFFSET {offset}"
    )).fetch_all(pool).await.map_err(db_err)?;

    let items = rows.iter().map(|r| json!({
        "concept_id":   r.get::<String, _>("concept_id"),
        "term":         r.try_get::<String, _>("term").unwrap_or_default(),
        "icd10_code":   r.try_get::<String, _>("icd10_code").unwrap_or_default(),
        "map_group":    r.try_get::<i32, _>("map_group").unwrap_or(1),
        "map_priority": r.try_get::<i32, _>("map_priority").unwrap_or(1),
        "map_rule":     r.try_get::<String, _>("map_rule").unwrap_or_default(),
        "map_advice":   r.try_get::<String, _>("map_advice").unwrap_or_default(),
        "map_category": r.try_get::<String, _>("map_category").unwrap_or_default(),
        "needs_review": r.get::<bool, _>("needs_review"),
    })).collect();

    let categories: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT DISTINCT map_category FROM snomed_icd10_extmap \
         WHERE tenant_id IS NULL AND target_system = '{target_system}' \
           AND map_category IS NOT NULL ORDER BY map_category"
    )).fetch_all(pool).await.unwrap_or_default();

    Ok(Json(ItemsResponse {
        kb_id: kb_id.into(),
        columns: vec![
            Column { name: "concept_id",   label: "SNOMED Concept", kind: "code" },
            Column { name: "term",         label: "FSN",            kind: "string" },
            Column { name: "icd10_code",   label: code_label,       kind: "code" },
            Column { name: "map_group",    label: "Grp",            kind: "string" },
            Column { name: "map_priority", label: "Prio",           kind: "string" },
            Column { name: "map_rule",     label: "Rule",           kind: "string" },
            Column { name: "map_advice",   label: "Advice",         kind: "string" },
            Column { name: "map_category", label: "Category",       kind: "enum" },
            Column { name: "needs_review", label: "Review",         kind: "boolean" },
        ],
        items, total, page, per_page,
        filters: json!({ "map_category": categories }),
    }))
}

// ── helpers ────────────────────────────────────────────────────────────────

impl ItemsQuery {
    /// Trimmed, non-empty search string if `q` was provided.
    fn search(&self) -> Option<String> {
        self.q.as_ref().and_then(|s| {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        })
    }
}

fn sql_safe(s: &str) -> String {
    s.replace('\'', "''").replace('\\', "\\\\")
}

fn db_err(e: sqlx::Error) -> (StatusCode, Json<JsonValue>) {
    (StatusCode::INTERNAL_SERVER_ERROR,
     Json(json!({"error": format!("db: {e}")})))
}
