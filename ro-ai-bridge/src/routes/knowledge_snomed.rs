//! SNOMED CT → ICD-10-TM / ICD-10-CM resolver + concept search.
//!
//! Backs the POC pipeline (insurance underwriter + medical coding):
//!   clinical text ──search──▶ SNOMED concept ──resolve-icd10──▶ ICD-10-TM code(s)
//!
//!   POST /api/v1/knowledge/snomed/search          — FULLTEXT term → concepts
//!   POST /api/v1/knowledge/snomed/resolve-icd10    — concept_id (+gender,+age) → ICD-10-TM (Thai)
//!   POST /api/v1/knowledge/snomed/resolve-icd10cm  — concept_id (+gender) → ICD-10-CM (US, rule-based)
//!   POST /api/v1/knowledge/snomed/dose-form        — tmt_id → FHIR doseForm CodeableConcept
//!
//! The Thai map (snomed_icd10_map) is pre-split by gender/age, so resolve-icd10
//! just filters. The US map (snomed_icd10cm_map, Sprint 60) is the official NLM
//! ExtendedMap: rule-based, so resolve-icd10cm walks each mapGroup in mapPriority
//! order and picks the first row whose mapRule is satisfied (gender evaluated
//! here; other IFA gates surfaced as `conditional` for the caller to evaluate).

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use mimir_core_ai::services::db::DbPool;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use sqlx::Row;
use std::collections::BTreeMap;
use tracing::warn;

pub fn knowledge_snomed_routes() -> Router<DbPool> {
    Router::new()
        .route("/search", post(search))
        .route("/resolve-icd10", post(resolve_icd10))
        .route("/resolve-icd10cm", post(resolve_icd10cm))
        .route("/dose-form", post(dose_form))
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

/// TMT ids are the medicine identifiers in the dose-link table (`varchar(20)`).
/// They're numeric in practice, but accept alphanumerics defensively per spec.
fn valid_tmt_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_alphanumeric())
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
    /// Optional refset gate (Sprint 58): "ips" (International Patient Summary) or
    /// "gpfp" (GP/FP primary-care reasons-for-encounter). Members are boosted to
    /// the top and each result carries an `in_refset` flag; with `refset_only`
    /// they are the *only* results. Lets the patient-summary builder (B1) prefer
    /// IPS-interoperable concepts and primary-care flows (B2) narrow to GP/FP.
    #[serde(default)]
    refset: Option<String>,
    #[serde(default)]
    refset_only: bool,
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
    // Refset gate. refset_key is matched against a fixed allowlist (never
    // interpolated from raw input), so the membership subquery is injection-safe.
    let refset_key = match req.refset.as_deref() {
        Some("ips") => Some("ips"),
        Some("gpfp") => Some("gpfp"),
        _ => None,
    };
    let (refset_select, refset_boost, refset_filter) = if let Some(rk) = refset_key {
        let member = format!(
            "EXISTS(SELECT 1 FROM snomed_refset_members m \
               WHERE m.refset_key = '{rk}' AND m.concept_id = snomed_descriptions.concept_id \
               AND m.active = 1)"
        );
        let select = format!(", ({member}) AS in_refset");
        let boost = format!("({member}) DESC, ");
        let filter = if req.refset_only {
            format!(" AND {member}")
        } else {
            String::new()
        };
        (select, boost, filter)
    } else {
        (String::new(), String::new(), String::new())
    };
    // FULLTEXT filters; ranking resolves to the right concept. EXACT term match
    // wins first (so a lay synonym exactly equal to the query — "heart attack" —
    // beats a partial FSN like "Attack (finding)"). FSN preference is only a
    // tiebreaker, below exactness, so the canonical generic still wins among
    // partial matches ("asthma" → "Asthma (disorder)", not a specific variant).
    // MATCH relevance MUST outrank CHAR_LENGTH — otherwise queries whose
    // apostrophe/hyphen/punctuation makes the LIKE-prefix tier miss fall through
    // to "shortest term wins" and pick a junk same-token match (e.g.
    // "Coats disease" → "Lyme disease" instead of "Coats' disease").
    let sql = format!(
        "SELECT concept_id, term, term_type, semantic_tag{refset_select} \
         FROM snomed_descriptions \
         WHERE tenant_id IS NULL AND active = 1{semtag_clause}{refset_filter} \
           AND MATCH(term) AGAINST('{q}' IN NATURAL LANGUAGE MODE) \
         ORDER BY \
           {refset_boost}\
           (LOWER(term) = LOWER('{q}')) DESC, \
           (LOWER(term) IN (LOWER('{q} (disorder)'), LOWER('{q} (finding)'))) DESC, \
           (LOWER(term) LIKE LOWER('{q}%')) DESC, \
           (term_type = 'fsn') DESC, \
           MATCH(term) AGAINST('{q}' IN NATURAL LANGUAGE MODE) DESC, \
           CHAR_LENGTH(term) ASC \
         LIMIT {limit}"
    );
    let rows = sqlx::query(&sql).fetch_all(&pool).await.map_err(db_error)?;
    let want_refset = refset_key.is_some();
    let concepts = rows
        .iter()
        .map(|r| {
            let mut o = json!({
                "concept_id": r.get::<String, _>("concept_id"),
                "term": r.get::<String, _>("term"),
                "term_type": r.get::<String, _>("term_type"),
                "semantic_tag": r.try_get::<String, _>("semantic_tag").unwrap_or_default(),
            });
            if want_refset {
                let in_rs: i64 = r.try_get("in_refset").unwrap_or(0);
                o["in_refset"] = json!(in_rs != 0);
            }
            o
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "query": req.text,
        "negated": negated,
        "refset": refset_key,
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

// ─── resolve concept → ICD-10-CM (US official rule-based ExtendedMap) ─────────

#[derive(Debug, Deserialize)]
struct ResolveCmReq {
    concept_id: String,
    /// 'M' | 'F' | None. Evaluates gender-gated mapRules (Male=248153007,
    /// Female=248152002). Other IFA rules (age/finding) can't be evaluated here.
    #[serde(default)]
    gender: Option<String>,
}

const SNOMED_MALE: &str = "248153007";
const SNOMED_FEMALE: &str = "248152002";

/// What kind of condition a mapRule expresses, after coarse parsing.
#[derive(PartialEq)]
enum RuleKind {
    /// "TRUE" / "OTHERWISE TRUE" / empty — unconditional, always fires.
    Always,
    Male,
    Female,
    /// Any other "IFA …" gate (age threshold, finding present) — needs the caller.
    Conditional,
}

fn classify_rule(rule: &str) -> RuleKind {
    let u = rule.trim().to_uppercase();
    if u.is_empty() || u == "TRUE" || u.ends_with("OTHERWISE TRUE") {
        return RuleKind::Always;
    }
    if rule.contains(SNOMED_MALE) || u.contains("| MALE") {
        return RuleKind::Male;
    }
    if rule.contains(SNOMED_FEMALE) || u.contains("| FEMALE") {
        return RuleKind::Female;
    }
    RuleKind::Conditional
}

/// Resolve a SNOMED concept to ICD-10-CM via the official NLM rule-based map.
///
/// The map partitions candidates into mapGroups; within a group they are tried
/// in mapPriority order and the FIRST rule that matches wins (an "OTHERWISE TRUE"
/// row usually closes a conditional group as the catch-all). We evaluate gender
/// gates deterministically; any other IFA gate we cannot evaluate is left for the
/// caller (the group's selection falls through to its catch-all if one exists,
/// otherwise the group is reported unresolved with its conditional candidates).
async fn resolve_icd10cm(
    State(pool): State<DbPool>,
    Json(req): Json<ResolveCmReq>,
) -> Result<Json<JsonValue>, RouteError> {
    if !valid_concept_id(&req.concept_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_concept_id", "hint": "numeric SNOMED id"})),
        ));
    }
    let gender = match req.gender.as_deref() {
        Some("M") => Some("M"),
        Some("F") => Some("F"),
        _ => None,
    };

    let concept_fsn: Option<String> = sqlx::query_scalar(
        "SELECT term FROM snomed_descriptions \
         WHERE tenant_id IS NULL AND concept_id = ? AND term_type = 'fsn' LIMIT 1",
    )
    .bind(&req.concept_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    let rows = sqlx::query(
        "SELECT map_group, map_priority, map_rule, map_advice, icd10cm_code, \
                map_category, needs_review \
         FROM snomed_icd10cm_map \
         WHERE tenant_id IS NULL AND active = 1 AND concept_id = ? \
         ORDER BY map_group, map_priority",
    )
    .bind(&req.concept_id)
    .fetch_all(&pool)
    .await
    .map_err(db_error)?;

    if rows.is_empty() {
        return Ok(Json(json!({
            "concept_id": req.concept_id, "concept_fsn": concept_fsn,
            "gender": gender, "groups": [], "billable": [],
            "note": "no ICD-10-CM map for this concept"
        })));
    }

    // One owned candidate per map row, so grouping doesn't borrow the sqlx rows.
    struct Cand {
        group: i32,
        priority: i32,
        code: Option<String>,
        rule: String,
        advice: String,
        category: Option<String>,
        needs_review: bool,
    }
    // Preserve group order; SQL already sorts by (group, priority).
    let mut grouped: BTreeMap<i32, Vec<Cand>> = BTreeMap::new();
    for r in &rows {
        let c = Cand {
            group: r.try_get("map_group").unwrap_or(1),
            priority: r.try_get("map_priority").unwrap_or(1),
            code: r.try_get::<String, _>("icd10cm_code").ok().filter(|s| !s.is_empty()),
            rule: r.try_get("map_rule").unwrap_or_default(),
            advice: r.try_get("map_advice").unwrap_or_default(),
            category: r.try_get("map_category").ok(),
            needs_review: r.try_get::<i8, _>("needs_review").unwrap_or(0) != 0,
        };
        grouped.entry(c.group).or_default().push(c);
    }

    let mut groups_out = Vec::new();
    let mut billable: Vec<String> = Vec::new();

    for (group, cands) in &grouped {
        let mut candidates = Vec::new();
        let mut selected: Option<JsonValue> = None;
        for c in cands {
            let kind = classify_rule(&c.rule);
            let kind_str = match kind {
                RuleKind::Always => "always",
                RuleKind::Male => "gender_male",
                RuleKind::Female => "gender_female",
                RuleKind::Conditional => "conditional",
            };
            candidates.push(json!({
                "map_priority": c.priority,
                "icd10cm_code": c.code,
                "map_rule": c.rule,
                "rule_kind": kind_str,
                "advice": c.advice,
                "map_category": c.category,
                "needs_review": c.needs_review,
            }));
            // First satisfied rule in priority order wins this group.
            let fires = match kind {
                RuleKind::Always => true,
                RuleKind::Male => gender == Some("M"),
                RuleKind::Female => gender == Some("F"),
                RuleKind::Conditional => false,
            };
            if selected.is_none() && fires {
                if let Some(code) = &c.code {
                    selected = Some(json!({
                        "icd10cm_code": code,
                        "advice": c.advice,
                        "map_category": c.category,
                        "needs_review": c.needs_review,
                        "via_rule": c.rule,
                    }));
                    if !c.needs_review {
                        billable.push(code.clone());
                    }
                }
            }
        }
        let reason = if selected.is_some() {
            "selected"
        } else if candidates.iter().any(|c| c["rule_kind"] == "conditional") {
            "unresolved: group gated by a condition this endpoint can't evaluate (age/finding) — caller must decide"
        } else {
            "no candidate matched the supplied context"
        };
        groups_out.push(json!({
            "map_group": group,
            "selected": selected,
            "reason": reason,
            "candidates": candidates,
        }));
    }

    Ok(Json(json!({
        "concept_id": req.concept_id,
        "concept_fsn": concept_fsn,
        "gender": gender,
        "groups": groups_out,
        "billable": billable,
    })))
}

// ─── resolve TMT med → FHIR doseForm CodeableConcept ──────────────────────────

const SYS_SNOMED: &str = "http://snomed.info/sct";
const SYS_EDQM: &str = "https://standardterms.edqm.eu"; // EDQM Standard Terms code system

#[derive(Debug, Deserialize)]
struct DoseFormReq {
    tmt_id: String,
}

/// Resolve a TMT medicine id to a FHIR R5 `Medication.doseForm` CodeableConcept.
///
/// Chain (Sprint 58): tmt_id ──snomed_tmt_dose_link──▶ SNOMED dose-form concept
/// ──snomed_edqm_dose_map──▶ EDQM code. Only `needs_review=0` links are trusted
/// (exact/normalized); token_subset links are needs_review=1 and deliberately NOT
/// auto-coded — they need human confirmation — so this returns `doseForm: null`
/// (with `trusted: false`) rather than asserting a possibly-wrong subtype.
///
/// Port of `scripts/fhir_dose_form.py::resolve_dose_form`.
async fn dose_form(
    State(pool): State<DbPool>,
    Json(req): Json<DoseFormReq>,
) -> Result<Json<JsonValue>, RouteError> {
    if !valid_tmt_id(&req.tmt_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_tmt_id", "hint": "alphanumeric TMT id"})),
        ));
    }
    // SNOMED coding always present; EDQM added when the concept carries a map.
    // needs_review=0 only — untrusted (token_subset) links resolve to null.
    let row = sqlx::query(
        "SELECT l.snomed_concept_id AS concept, \
           (SELECT term FROM snomed_descriptions d \
              WHERE d.concept_id = l.snomed_concept_id AND d.term_type = 'fsn' LIMIT 1) AS fsn, \
           (SELECT e.edqm_code FROM snomed_edqm_dose_map e \
              WHERE e.snomed_concept_id = l.snomed_concept_id ORDER BY e.edqm_code LIMIT 1) AS edqm \
         FROM snomed_tmt_dose_link l \
         WHERE l.tmt_id = ? AND l.needs_review = 0 LIMIT 1",
    )
    .bind(&req.tmt_id)
    .fetch_optional(&pool)
    .await
    .map_err(db_error)?;

    let concept = row.as_ref().and_then(|r| r.try_get::<String, _>("concept").ok());
    let concept = match concept {
        Some(c) if !c.is_empty() => c,
        _ => {
            return Ok(Json(
                json!({"tmt_id": req.tmt_id, "doseForm": JsonValue::Null, "trusted": false}),
            ));
        }
    };
    let row = row.expect("row present when concept resolved");
    let fsn: Option<String> = row.try_get("fsn").ok();
    let edqm: Option<String> = row.try_get("edqm").ok();

    // Display strips the "(dose form)" semantic tag from the FSN.
    let display = fsn
        .as_deref()
        .map(|f| f.replace("(dose form)", "").trim().to_string())
        .filter(|s| !s.is_empty());

    let mut snomed_coding = json!({"system": SYS_SNOMED, "code": concept});
    if let Some(d) = &display {
        snomed_coding["display"] = json!(d);
    }
    let mut codings = vec![snomed_coding];
    if let Some(e) = edqm.filter(|e| !e.is_empty()) {
        codings.push(json!({"system": SYS_EDQM, "code": e}));
    }
    let mut cc = json!({"coding": codings});
    if let Some(d) = &display {
        cc["text"] = json!(d);
    }

    Ok(Json(json!({
        "tmt_id": req.tmt_id,
        "doseForm": cc,
        "trusted": true,
    })))
}
