//! SNOMED CT → ICD-10-TM resolver + concept search.
//!
//! Backs the POC pipeline (insurance underwriter + medical coding):
//!   clinical text ──search──▶ SNOMED concept ──resolve-icd10──▶ ICD-10-TM code(s)
//!
//!   POST /api/v1/knowledge/snomed/search        — FULLTEXT term → concepts
//!   POST /api/v1/knowledge/snomed/resolve-icd10  — concept_id (+gender,+age) → ICD-10-TM
//!
//! The map (snomed_icd10_map) is pre-split by gender/age, so the resolver just
//! filters; targets carry a role (mandatory/conditional/advisory) and a
//! needs_review flag (cannot-classify / context-dependent / TM-absent /
//! external-cause requiring post-coordination).

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use mimir_core_ai::services::db::DbPool;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use sqlx::Row;
use tracing::warn;

pub fn knowledge_snomed_routes() -> Router<DbPool> {
    Router::new()
        .route("/search", post(search))
        .route("/resolve-icd10", post(resolve_icd10))
}

type RouteError = (StatusCode, Json<JsonValue>);

fn db_error(e: sqlx::Error) -> RouteError {
    warn!("snomed query error: {e}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "query_failed", "detail": e.to_string()})),
    )
}

fn valid_concept_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_digit())
}

// ─── concept search ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SearchReq {
    text: String,
    #[serde(default = "default_limit")]
    limit: i64,
    /// Optional semantic-tag filter, e.g. "disorder" / "finding" / "procedure".
    #[serde(default)]
    semantic_tag: Option<String>,
}
fn default_limit() -> i64 {
    10
}

/// Clinical negation cues. Detection lives here (not in the resolver) so the
/// caller gets a `negated` signal alongside the matched concept — FULLTEXT
/// would otherwise match "angina" inside "without angina" and look confident.
/// We strip the cue from the match query and flag it; the caller decides not
/// to ground a negated finding (fits the advisory ground/flag pattern).
const NEG_CUES: &[&str] = &[
    "no", "not", "without", "denies", "denied", "negative", "absence", "absent", "free", "neg",
];

fn detect_negation(text: &str) -> (bool, String) {
    let mut negated = false;
    let mut kept: Vec<&str> = Vec::new();
    for tok in text.split_whitespace() {
        let norm = tok
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if NEG_CUES.contains(&norm.as_str()) {
            negated = true;
            continue; // drop the cue from the FULLTEXT query
        }
        kept.push(tok);
    }
    let cleaned = kept.join(" ");
    // If stripping left nothing, fall back to the original text.
    if cleaned.trim().is_empty() {
        (negated, text.to_string())
    } else {
        (negated, cleaned)
    }
}

async fn search(
    State(pool): State<DbPool>,
    Json(req): Json<SearchReq>,
) -> Result<Json<JsonValue>, RouteError> {
    let (negated, cleaned) = detect_negation(&req.text);
    let q = cleaned.replace('\'', "''");
    let limit = req.limit.clamp(1, 50);
    let semtag_clause = match &req.semantic_tag {
        Some(t) if !t.is_empty() => {
            format!(" AND semantic_tag = '{}'", t.replace('\'', "''"))
        }
        _ => String::new(),
    };
    // FULLTEXT filters; ranking resolves to the right concept. EXACT term match
    // wins first (so a lay synonym exactly equal to the query — "heart attack" —
    // beats a partial FSN like "Attack (finding)"). FSN preference is only a
    // tiebreaker, below exactness, so the canonical generic still wins among
    // partial matches ("asthma" → "Asthma (disorder)", not a specific variant).
    let sql = format!(
        "SELECT concept_id, term, term_type, semantic_tag \
         FROM snomed_descriptions \
         WHERE tenant_id IS NULL AND active = 1{semtag_clause} \
           AND MATCH(term) AGAINST('{q}' IN NATURAL LANGUAGE MODE) \
         ORDER BY \
           (LOWER(term) = LOWER('{q}')) DESC, \
           (LOWER(term) IN (LOWER('{q} (disorder)'), LOWER('{q} (finding)'))) DESC, \
           (LOWER(term) LIKE LOWER('{q}%')) DESC, \
           (term_type = 'fsn') DESC, \
           CHAR_LENGTH(term) ASC, \
           MATCH(term) AGAINST('{q}' IN NATURAL LANGUAGE MODE) DESC \
         LIMIT {limit}"
    );
    let rows = sqlx::query(&sql).fetch_all(&pool).await.map_err(db_error)?;
    let concepts = rows
        .iter()
        .map(|r| {
            json!({
                "concept_id": r.get::<String, _>("concept_id"),
                "term": r.get::<String, _>("term"),
                "term_type": r.get::<String, _>("term_type"),
                "semantic_tag": r.try_get::<String, _>("semantic_tag").unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "query": req.text,
        "negated": negated,
        "concepts": concepts,
    })))
}

// ─── resolve concept → ICD-10-TM ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ResolveReq {
    concept_id: String,
    /// 'M' | 'F' | None(any). Filters gender-specific map rows when provided.
    #[serde(default)]
    gender: Option<String>,
    /// neonatal|pediatric|adolescent|adult|geriatric | None(any).
    #[serde(default)]
    age_group: Option<String>,
}

async fn resolve_icd10(
    State(pool): State<DbPool>,
    Json(req): Json<ResolveReq>,
) -> Result<Json<JsonValue>, RouteError> {
    if !valid_concept_id(&req.concept_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_concept_id", "hint": "numeric SNOMED id"})),
        ));
    }
    // Gender/age rows are pre-split: a NULL row applies to anyone; a specific
    // row applies only when the caller's value matches. When the caller omits
    // the dimension, return all variants (each row carries its own gender/age).
    let mut clause = String::new();
    if let Some(g) = req.gender.as_deref() {
        if g == "M" || g == "F" {
            clause.push_str(&format!(" AND (gender IS NULL OR gender = '{g}')"));
        }
    }
    if let Some(a) = req.age_group.as_deref() {
        let a = a.replace('\'', "''");
        clause.push_str(&format!(" AND (age_group IS NULL OR age_group = '{a}')"));
    }
    let sql = format!(
        "SELECT icd10_who, icd10_tm, match_tier, target_role, gender, age_group, \
                map_advice, needs_review \
         FROM snomed_icd10_map \
         WHERE tenant_id IS NULL AND concept_id = '{cid}'{clause} \
         ORDER BY (target_role = 'mandatory') DESC, icd10_tm IS NULL, icd10_who",
        cid = req.concept_id,
    );
    // Return the concept's FSN so the caller can sanity-check which concept was
    // resolved (e.g. confirm it isn't a near-miss variant or a negated finding).
    let concept_fsn: Option<String> = sqlx::query_scalar(
        "SELECT term FROM snomed_descriptions \
         WHERE tenant_id IS NULL AND concept_id = ? AND term_type = 'fsn' LIMIT 1",
    )
    .bind(&req.concept_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    let rows = sqlx::query(&sql).fetch_all(&pool).await.map_err(db_error)?;
    if rows.is_empty() {
        return Ok(Json(json!({
            "concept_id": req.concept_id, "concept_fsn": concept_fsn,
            "targets": [], "billable": [],
            "note": "no ICD-10 map for this concept"
        })));
    }

    let mut targets = Vec::new();
    let mut billable = Vec::new();
    for r in &rows {
        let who: String = r.get("icd10_who");
        let tm: Option<String> = r.try_get("icd10_tm").ok();
        let tier: String = r.get("match_tier");
        let role: String = r.get("target_role");
        let needs_review: i8 = r.try_get("needs_review").unwrap_or(0);
        let item = json!({
            "icd10_who": who,
            "icd10_tm": tm,
            "match_tier": tier,
            "role": role,
            "gender": r.try_get::<String, _>("gender").ok(),
            "age_group": r.try_get::<String, _>("age_group").ok(),
            "advice": r.try_get::<String, _>("map_advice").unwrap_or_default(),
            "needs_review": needs_review != 0,
        });
        // Billable shortlist: resolved TM code, mandatory, no review flag.
        if let Some(code) = item.get("icd10_tm").and_then(|v| v.as_str()) {
            if role == "mandatory" && needs_review == 0 {
                billable.push(code.to_string());
            }
        }
        targets.push(item);
    }
    Ok(Json(json!({
        "concept_id": req.concept_id,
        "concept_fsn": concept_fsn,
        "gender": req.gender,
        "age_group": req.age_group,
        "targets": targets,
        "billable": billable,
    })))
}
