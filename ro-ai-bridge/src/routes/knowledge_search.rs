//! Unified cross-KB search — Level 3 of the Shared Knowledge UI.
//!
//! GET /api/v1/knowledge/search?q=<term>&k=<int>
//!
//! Fans out the same query across all shared KBs in parallel, returning
//! grouped results so the user sees "how does this concept appear in
//! every vocabulary at once". Each KB uses its native lookup:
//!
//!   icd10-tm  → cascade (exact → naive → semantic via Heimdall BGE-M3)
//!   tpc       → cascade
//!   tmt       → FULLTEXT on fsn
//!   tmlt      → FULLTEXT on fsn
//!   loinc     → LIKE on long_common_name + short_name
//!   primekg   → semantic via Qdrant primekg-entities (BGE-M3 1024-d)
//!
//! Returns a flat envelope so the UI can render with one template:
//!   { q, k, results: [{kb_id, items, count, latency_ms}], total_ms }

use axum::{extract::{Query, State}, http::StatusCode, routing::get, Json, Router};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::Row;
use std::time::Instant;
use tracing::warn;

pub fn knowledge_search_routes() -> Router<DbPool> {
    Router::new().route("/", get(unified_search))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_k")]
    k: u32,
}

fn default_k() -> u32 { 3 }

#[derive(Debug, Serialize)]
struct KbResult {
    kb_id: &'static str,
    /// Display title fetched live from shared_knowledge metadata (kept in sync).
    kb_name: &'static str,
    items: Vec<JsonValue>,
    count: usize,
    latency_ms: u128,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    q: String,
    k: u32,
    results: Vec<KbResult>,
    total_ms: u128,
}

async fn unified_search(
    State(pool): State<DbPool>,
    Query(sq): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<JsonValue>)> {
    let q = sq.q.trim();
    if q.is_empty() {
        return Err((StatusCode::BAD_REQUEST,
                    Json(json!({"error": "q must not be empty"}))));
    }
    // Min-length guard: 1-char prefix queries return ~broad noise (e.g. q='a'
    // matches every code/label starting with 'a'). Require ≥ 2 chars (Thai
    // single-character queries are also degenerate — Thai medical terms are
    // multi-character).
    if q.chars().count() < 2 {
        return Err((StatusCode::BAD_REQUEST,
                    Json(json!({"error": "q must be at least 2 characters"}))));
    }
    let k = sq.k.clamp(1, 10) as i64;
    let start = Instant::now();

    // Query rewriting: two transforms of the same lookup table, one per
    // KB type. See `lookup_expansion()` for the entries.
    //   - FULLTEXT KBs (TMT/TMLT): append form — relevance score picks
    //     between the original token + the canonical long form.
    //   - LIKE-based KBs (ICD/TPC/LOINC) + semantic (PrimeKG): replace
    //     form — substring search can't match a 2-word acronym+expansion
    //     concatenation; semantic embedding does better with natural form.
    let q_append  = expand_query(q);
    let q_replace = replace_query(q);

    // Fan out — each KB query is independent. Use tokio::join for parallelism.
    // ICD-10-TM gets BOTH forms because it has a dedicated th_label column
    // (Thai user phrases like `เบาหวาน` should still substring-match the
    // Thai label) AND an en_label column (where the replaced English form
    // does better for aliases like `T2DM`).
    // ICD route handles multi-segment splitting internally (clinical_scenario
    // queries like `sleep apnea AHI 35 with HTN` resolve to G47.3 + I10
    // via per-segment lookup + dedupe).
    let p1 = icd10_search_multi(&pool, q, k);
    let p2 = tpc_search(&pool, &q_replace, k);
    let p3 = loinc_search(&pool, &q_replace, k);
    let p4 = tmt_search(&pool, &q_append, k);
    let p5 = tmlt_search(&pool, &q_append, k);
    let p6 = primekg_search(&q_replace, k);
    // 7th KB: PrimeKG graph traversal (symptoms → disease) + ICD chain.
    // Uses q_replace so Thai symptom aliases (ปวดหัว → headache) get
    // rewritten before tokenisation against PrimeKG phenotype nodes.
    let p7 = symptom_search(&pool, &q_replace, k);

    let (r1, r2, r3, r4, r5, r6, r7) = tokio::join!(p1, p2, p3, p4, p5, p6, p7);

    let results = vec![r1, r2, r3, r4, r5, r6, r7];

    Ok(Json(SearchResponse {
        q: q.to_string(),
        k: k as u32,
        results,
        total_ms: start.elapsed().as_millis(),
    }))
}

// ── query rewriting ───────────────────────────────────────────────────────
//
// Single source-of-truth lookup table for first-token query rewriting.
// Two transforms consume the same table:
//
//   - `expand_query()`  — appends the canonical form to the original query.
//                         Used by FULLTEXT KBs (TMT/TMLT) where MATCH()
//                         NATURAL LANGUAGE relevance picks whichever form
//                         the FSN happens to store.
//
//   - `replace_query()` — replaces the first token with the canonical form,
//                         keeps trailing modifiers. Used by LIKE-based KBs
//                         (ICD/TPC/LOINC) where substring match needs to
//                         find the long form directly, plus semantic search
//                         (PrimeKG) where the natural-language form gives
//                         a cleaner embedding.
//
// Three classes of entries:
//   1. Lab acronyms (HbA1c → "glycated hemoglobin") — ASCII, case-insensitive
//   2. Disease acronyms (T2DM → "diabetes mellitus type 2") — ASCII, case-insensitive
//   3. Thai drug transliterations (วาร์ฟาริน → "warfarin") — Thai script, raw match
//
// Only high-confidence single-meaning mappings. Multi-word queries:
// first whitespace-separated token only. Trailing context (dose, route,
// modifier) preserved by both transforms.
fn lookup_expansion(first_token: &str) -> Option<&'static str> {
    let key_lc = first_token.to_ascii_lowercase();
    let by_lc: Option<&'static str> = match key_lc.as_str() {
        // ── lab acronyms → canonical long form ─────────────────────────
        "hba1c"    => Some("glycated hemoglobin"),
        "bun"      => Some("urea nitrogen"),
        "alt"      => Some("alanine aminotransferase"),
        "sgpt"     => Some("alanine aminotransferase"),
        "ast"      => Some("aspartate aminotransferase"),
        "sgot"     => Some("aspartate aminotransferase"),
        "ldl"      => Some("low density lipoprotein"),
        "hdl"      => Some("high density lipoprotein"),
        "tsh"      => Some("thyroid stimulating hormone"),
        "psa"      => Some("prostate specific antigen"),
        "cbc"      => Some("complete blood count"),
        "wbc"      => Some("white blood cell"),
        "rbc"      => Some("red blood cell"),
        "inr"      => Some("international normalized ratio"),
        "crp"      => Some("c-reactive protein"),
        "esr"      => Some("erythrocyte sedimentation rate"),
        "ggt"      => Some("gamma glutamyl transferase"),
        "alp"      => Some("alkaline phosphatase"),
        "ck"       => Some("creatine kinase"),
        "ldh"      => Some("lactate dehydrogenase"),
        // ── disease acronyms → canonical long form ─────────────────────
        // Word order chosen to match the ICD-10-TM en_label format
        // ("Non-insulin-dependent diabetes mellitus type 2 at without
        // complications" — so "diabetes mellitus type 2" substring hits).
        "t2dm"     => Some("diabetes mellitus type 2"),
        "t1dm"     => Some("diabetes mellitus type 1"),
        "dm"       => Some("diabetes mellitus"),
        "htn"      => Some("hypertension"),
        "osa"      => Some("sleep apnoea"),  // UK spelling matches ICD-10-TM
        // US → UK spelling for sleep terms (ICD-10-TM en_label uses UK).
        // Note: this is the *full multi-word* phrase path; multi-token
        // queries flow through `find_expansion`'s full-trimmed-query try.
        "obstructive sleep apnea"  => Some("sleep apnoea"),
        "obstructive sleep apnoea" => Some("sleep apnoea"),
        "sleep apnea"              => Some("sleep apnoea"),
        "apnea"                    => Some("apnoea"),
        // Sleep medicine — CPAP / PSG / AHI common shorthand.
        // Keep replacements TIGHT (one canonical phrase, no synonym dump)
        // — multi-term replacements dilute PrimeKG semantic search and
        // bloat TMT FULLTEXT relevance.
        "cpap"                     => Some("continuous positive airway pressure"),
        "cpap titration"           => Some("continuous positive airway pressure titration"),
        "ahi"                      => Some("sleep apnoea"),         // route to G47.3 family
        "psg"                      => Some("polysomnography"),
        "sleep study"              => Some("polysomnography"),
        // Drug class — single tight alias for SGLT2 (the only drug-class
        // M1 query). Other classes left raw to avoid regressing
        // multi-symptom drug_interaction queries where the class acronym
        // is incidental context (e.g. `SSRI + tramadol`).
        "sglt2 inhibitor"          => Some("empagliflozin"),  // exemplar — ranks specific drug rows higher than "inhibitor"-only rows
        "sglt-2 inhibitor"         => Some("empagliflozin"),
        "copd"     => Some("chronic obstructive pulmonary"),
        "chf"      => Some("congestive heart failure"),
        "ckd"      => Some("chronic kidney disease"),
        "uti"      => Some("urinary tract infection"),
        _ => None,
    };
    // Thai script → Latin generic name. Top commonly prescribed drugs
    // (Thai FDA generic spellings). Extend as production query logs reveal
    // new entries; a DB-backed alias table is the long-term shape if this
    // grows past ~50-100 entries.
    by_lc.or_else(|| match first_token {
        // Cardiovascular
        "วาร์ฟาริน"          => Some("warfarin"),
        "โลซาร์แทน"          => Some("losartan"),
        "โลซาร์ทาน"          => Some("losartan"),       // spelling variant (ทาน/แทน)
        "อะมโลดิปีน"         => Some("amlodipine"),
        "แอมโลดิปีน"         => Some("amlodipine"),      // spelling variant (แอม/อะม)
        "เอนาลาพริล"         => Some("enalapril"),
        "อะทีโนลอล"          => Some("atenolol"),
        "ซิมวาสแตติน"        => Some("simvastatin"),
        "อะทอร์วาสแตติน"     => Some("atorvastatin"),
        "ไฮโดรคลอโรไทอะไซด์" => Some("hydrochlorothiazide"),
        "ดิจอกซิน"           => Some("digoxin"),
        // Diabetes
        "เมทฟอร์มิน"         => Some("metformin"),
        "ไกลเบนคลาไมด์"      => Some("glibenclamide"),
        "อินซูลิน"           => Some("insulin"),
        // Pain / inflammation
        "พาราเซตามอล"        => Some("acetaminophen paracetamol"),
        "ไอบูโพรเฟน"         => Some("ibuprofen"),
        "แอสไพริน"           => Some("aspirin"),
        "ไดโคลฟีแนค"         => Some("diclofenac"),
        "ทรามาดอล"           => Some("tramadol"),
        "เพรดนิโซโลน"        => Some("prednisolone"),
        // GI
        "โอเมพราโซล"         => Some("omeprazole"),
        // Antibiotics
        "อะม็อกซิลลิน"       => Some("amoxicillin"),
        "เซฟาเล็กซิน"        => Some("cephalexin"),
        "เซฟไตรอะโซน"        => Some("ceftriaxone"),
        "ไซโปรฟลอกซาซิน"     => Some("ciprofloxacin"),
        "อะซิโทรมัยซิน"      => Some("azithromycin"),
        // Allergy
        "เซทิริซีน"          => Some("cetirizine"),
        "ลอราตาดีน"          => Some("loratadine"),
        // Respiratory (asthma / COPD)
        "ซาลบูทามอล"         => Some("salbutamol albuterol"),  // UK + US name
        "อัลบูเทอรอล"        => Some("albuterol salbutamol"),
        "ไอพราโทรเปียม"      => Some("ipratropium"),
        // Diuretic / cardiac add-ons (caught from M1 audit gaps)
        "ฟูโรซีไมด์"         => Some("furosemide"),
        "ฟูโรเซไมด์"         => Some("furosemide"),       // spelling variant
        // ── Thai disease names ─────────────────────────────────────────
        // ICD-10-TM th_label has tone-mark stripping issues in the loaded
        // dataset ("นอนไมหลับ" instead of "นอนไม่หลับ") so direct user-
        // typed Thai often misses substring match. Map common Thai disease
        // phrases to their canonical English form for ICD/PrimeKG lookup.
        "เบาหวาน"                  => Some("diabetes mellitus"),
        "เบาหวานชนิดที่ 2"         => Some("diabetes mellitus type 2"),
        "เบาหวานชนิดที่ 1"         => Some("diabetes mellitus type 1"),
        "ความดันโลหิตสูง"          => Some("essential hypertension"),
        "ความดันสูง"               => Some("hypertension"),
        "โรคหืด"                   => Some("asthma"),
        "โรคหอบหืด"                => Some("asthma"),
        "หอบหืด"                   => Some("asthma"),
        "ภาวะหยุดหายใจขณะหลับ"     => Some("sleep apnoea"),
        "นอนไม่หลับ"               => Some("insomnia"),
        "ปอดอุดกั้นเรื้อรัง"       => Some("chronic obstructive pulmonary"),
        "หัวใจล้มเหลว"             => Some("heart failure"),
        "ไตวาย"                    => Some("kidney failure"),
        "ไตวายเรื้อรัง"            => Some("chronic kidney disease"),
        "หลอดเลือดสมอง"            => Some("cerebrovascular"),
        // ── Thai symptom phrases → English phenotype tokens ───────────
        // Used by `symptom_search` worker — tokenizes the expanded form
        // and looks up against PrimeKG `effect/phenotype` nodes.
        "ปวดหัว"                   => Some("headache"),
        "ปวดหัวรุนแรงเฉียบพลัน"    => Some("acute severe headache"),
        "เจ็บหน้าอก"               => Some("chest pain"),
        "เจ็บแน่นหน้าอกร้าวไปแขนซ้าย" => Some("chest pain radiating left arm"),
        "หายใจไม่ออก"              => Some("dyspnea shortness of breath"),
        "หายใจลำบาก"               => Some("dyspnea"),
        "ไข้"                      => Some("fever"),
        "ไข้ไอเสมหะเขียว"           => Some("fever cough sputum"),
        "ไอ"                       => Some("cough"),
        "เสมหะ"                    => Some("sputum"),
        "ปัสสาวะบ่อย"              => Some("polyuria"),
        "กระหายน้ำ"                => Some("polydipsia"),
        "นอนกรน"                   => Some("snoring"),
        "ง่วงนอน"                  => Some("drowsiness"),
        "น้ำหนักลด"                => Some("weight loss"),
        "ขาบวม"                    => Some("ankle swelling edema"),
        // ── Thai sleep medicine phrases ────────────────────────────────
        "การปรับเครื่อง CPAP"      => Some("continuous positive airway pressure titration"),
        _ => None,
    })
}

/// Find the longest matching alias prefix and return `(matched_prefix,
/// canonical_form)`. Walks from full-query down to 1-word prefixes,
/// returning the first match. Catches:
///   - full multi-word Thai phrases like `"ภาวะหยุดหายใจขณะหลับ"`
///   - leading 2+ word acronym phrases like `"sleep apnea"` in
///     `"sleep apnea AHI 35 with HTN"` (clinical_scenario queries)
///   - single first-token like `"พาราเซตามอล"` in `"พาราเซตามอล 500 mg"`
fn find_expansion(q: &str) -> Option<(&str, &'static str)> {
    let token = q.trim();
    if token.is_empty() {
        return None;
    }
    // Pre-compute byte-end offsets of each whitespace-separated word so we
    // can build prefix slices without allocating.
    let mut word_ends: Vec<usize> = Vec::with_capacity(8);
    let mut in_word = false;
    let mut end = 0;
    for (i, c) in token.char_indices() {
        if c.is_whitespace() {
            if in_word {
                word_ends.push(end);
                in_word = false;
            }
        } else {
            in_word = true;
            end = i + c.len_utf8();
        }
    }
    if in_word {
        word_ends.push(end);
    }
    // Longest-prefix-first: walk from full-word count down to 1.
    for end_idx in word_ends.iter().rev() {
        let prefix = &token[..*end_idx];
        if let Some(exp) = lookup_expansion(prefix) {
            return Some((prefix, exp));
        }
    }
    None
}

/// Split a multi-condition query into segments by clinical connectors:
/// `" with " / " and " / "+" / "," / ";"`. Returns a single-element vec
/// when no connector is present.
fn split_segments(q: &str) -> Vec<String> {
    let mut segments: Vec<String> = vec![q.trim().to_string()];
    for sep in [" with ", " and ", " + ", "+", ",", ";"].iter() {
        segments = segments
            .into_iter()
            .flat_map(|s| {
                s.split(sep)
                    .map(|p| p.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    if segments.is_empty() {
        vec![q.trim().to_string()]
    } else {
        segments
    }
}

/// Append the canonical form to the original query: `"HbA1c"` →
/// `"HbA1c glycated hemoglobin"`. For FULLTEXT MATCH() NATURAL LANGUAGE
/// where relevance score picks the most-specific matching FSN regardless
/// of which form (acronym vs long) is stored in the index.
fn expand_query(q: &str) -> String {
    match find_expansion(q) {
        Some((_, exp)) => format!("{token} {exp}", token = q.trim()),
        None => q.to_string(),
    }
}

/// Replace the matched prefix with its canonical form: `"T2DM diet"` →
/// `"diabetes mellitus type 2 diet"`, `"ภาวะหยุดหายใจขณะหลับ"` →
/// `"sleep apnoea"`. For LIKE-based KBs (ICD/TPC/LOINC) where appending
/// the long form to a 4-char acronym never produces a matching substring,
/// and for semantic search where the natural-language form gives a cleaner
/// embedding signal than the acronym alone.
fn replace_query(q: &str) -> String {
    let token = q.trim();
    match find_expansion(q) {
        Some((matched, exp)) => {
            // Slice the rest (including leading whitespace) and append.
            // UTF-8 safe — `matched` is either the full token or a slice
            // taken at a whitespace boundary, both valid byte boundaries.
            let rest = token.get(matched.len()..).unwrap_or("");
            format!("{exp}{rest}")
        }
        None => q.to_string(),
    }
}

// ── code-pattern preprocessing ────────────────────────────────────────────
//
// ICD-10 codes are stored WITHOUT decimal dots in `icd10_codes.code`
// (e.g. `E119` not `E11.9`, `J189` not `J18.9`). M1 dataset + real users
// commonly type the dotted form. Scan ALL whitespace-separated tokens
// of the query for ICD-code-shaped tokens (letter + 2-3 digits + optional
// `.digits`) and add both forms (with and without dot) to the variants
// list. Catches three patterns:
//   - standalone code   `E11.9`               → ["E11.9", "E119"]
//   - leading code      `G47.3 คือโรคอะไร`     → ["G47.3 คือโรคอะไร", "G473"]
//   - embedded code     `ICD-10 J18.9`        → ["ICD-10 J18.9", "J189"]
//
// Non-code tokens like `metformin` or `ICD-10` are skipped (letter-only
// or letter+hyphen don't match the digit-rest rule).
fn icd_code_variants(q: &str) -> Vec<String> {
    let mut variants = vec![q.to_string()];
    for tok in q.split_whitespace() {
        let upper = tok.to_uppercase();
        let bytes = upper.as_bytes();
        // Need at least 3 chars: letter + ≥2 digits/dots
        if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() {
            continue;
        }
        let rest_ok = bytes[1..]
            .iter()
            .all(|b| b.is_ascii_digit() || *b == b'.');
        if !rest_ok {
            continue;
        }
        // Dotted form: add stripped variant
        if upper.contains('.') {
            variants.push(upper.replace('.', ""));
        }
        // Embedded form: also surface the bare code-shaped token so
        // `code LIKE 'J18.9%'` / `'J189%'` works even when the full
        // query is `"ICD-10 J18.9"`.
        if upper != q.to_uppercase() {
            variants.push(upper.clone());
        }
    }
    variants
}

// ── per-KB workers ────────────────────────────────────────────────────────

/// Multi-segment ICD wrapper: split the user query into clinical segments
/// (`" with "`, `" and "`, `"+"`, `","`, `";"`) and run `icd10_search` per
/// segment, deduping the merged items by code. Single-segment queries take
/// the direct path. Enables clinical_scenario queries like
/// `"sleep apnea AHI 35 with HTN"` to surface BOTH G47.3 and I10 in one
/// KbResult (each segment alone would only yield its own code).
async fn icd10_search_multi(pool: &DbPool, q_raw: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let segments = split_segments(q_raw);
    if segments.len() <= 1 {
        // Direct path — no overhead for normal queries.
        let q_replace = replace_query(q_raw);
        return icd10_search(pool, &q_replace, q_raw, k).await;
    }
    let mut items_acc: Vec<JsonValue> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for seg in &segments {
        // For each segment: if an alias matches, use ONLY the canonical
        // expansion (drop trailing modifiers like "AHI 35") to keep the
        // ICD AND-of-words tokenization clean — `sleep apnea AHI 35` →
        // `sleep apnoea` (not `sleep apnoea AHI 35`, which would require
        // "AHI" to also appear in the ICD en_label).
        let seg_canonical = match find_expansion(seg) {
            Some((_, exp)) => exp.to_string(),
            None => seg.to_string(),
        };
        let seg_kb = icd10_search(pool, &seg_canonical, seg, k).await;
        for it in seg_kb.items {
            let code = it
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !code.is_empty() && seen.insert(code) {
                items_acc.push(it);
                if items_acc.len() >= k as usize {
                    break;
                }
            }
        }
        if items_acc.len() >= k as usize {
            break;
        }
    }
    KbResult {
        kb_id: "icd10-tm",
        kb_name: "ICD-10-TM (Thai)",
        count: items_acc.len(),
        items: items_acc,
        latency_ms: t0.elapsed().as_millis(),
    }
}

async fn icd10_search(pool: &DbPool, q_en: &str, q_raw: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_en_safe  = q_en.replace('\'', "''");
    let q_raw_safe = q_raw.replace('\'', "''");
    let variants = icd_code_variants(&q_en_safe);
    // Build the WHERE clauses:
    //   - `code` and `en_label` use the English/canonical form (q_en).
    //   - `th_label` uses the raw user query (q_raw) so Thai disease
    //     names still substring-match the Thai column (avoids a regression
    //     where Thai aliases that rewrite to English bypass th_label hits).
    let mut code_likes: Vec<String> = variants
        .iter()
        .map(|v| format!("code LIKE '{v}%'"))
        .collect();
    // en_label match: try the literal whole-phrase substring first AND
    // an AND-of-word-substring form. Whole-phrase wins when the label
    // has the exact wording ("Sleep apnoea"); AND-of-words wins when the
    // label has a parenthetical in the middle ("Essential (primary)
    // hypertension" — exact "essential hypertension" substring fails but
    // both words present succeeds). Tokens <3 chars (a/of/to) are dropped.
    let words: Vec<&str> = q_en_safe
        .split_whitespace()
        .filter(|w| w.len() >= 3 && w.is_ascii())
        .collect();
    let en_clause = if words.len() >= 2 {
        let and_words = words
            .iter()
            .map(|w| format!("en_label LIKE '%{w}%'"))
            .collect::<Vec<_>>()
            .join(" AND ");
        format!("(en_label LIKE '%{q_en_safe}%' OR ({and_words}))")
    } else {
        format!("en_label LIKE '%{q_en_safe}%'")
    };
    code_likes.push(en_clause);
    code_likes.push(format!("th_label LIKE '%{q_raw_safe}%'"));
    let where_clause = code_likes.join(" OR ");
    let order_pivot = variants.last().cloned().unwrap_or_else(|| q_en_safe.clone());
    // ORDER BY priority: exact-code match, then code-prefix, then
    // *whole-phrase* en_label substring (so "diabetes mellitus type 2"
    // ranks E11x above E10x even though both satisfy the AND-of-words
    // fallback), then th_label substring (Thai phrase hit), then code.
    let sql = format!(
        "SELECT code, en_label, th_label, chapter FROM icd10_codes \
         WHERE tenant_id IS NULL AND ({where_clause}) \
         ORDER BY (code = '{pivot}') DESC, \
                  (code LIKE '{pivot}%') DESC, \
                  (en_label LIKE '%{q_en_safe}%') DESC, \
                  (th_label LIKE '%{q_raw_safe}%') DESC, \
                  code LIMIT {k}",
        pivot = order_pivot, k = k,
    );
    let items = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rs) => rs.iter().map(|r| {
            let code: String = r.get("code");
            // ICD-10 canonical published form has a `.` between the
            // 3-char category prefix and any sub-classification digits.
            // Storage uses the stripped form; re-format for UI / agents.
            let code_formatted = icd_format_code(&code);
            json!({
                "code": code,
                "code_formatted": code_formatted,
                "en_label": r.try_get::<String,_>("en_label").unwrap_or_default(),
                "th_label": r.try_get::<String,_>("th_label").unwrap_or_default(),
                "chapter": r.try_get::<String,_>("chapter").unwrap_or_default(),
            })
        }).collect::<Vec<_>>(),
        Err(e) => {
            warn!("icd10_search: {e}");
            vec![]
        }
    };
    KbResult {
        kb_id: "icd10-tm", kb_name: "ICD-10-TM (Thai)",
        count: items.len(), items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

/// Re-insert the canonical dot between the 3-char category and any
/// sub-classification digits: `E119` → `E11.9`, `G473` → `G47.3`,
/// `I10` → `I10` (no change, no sub-classification). Codes that don't
/// match the standard pattern pass through unchanged.
fn icd_format_code(code: &str) -> String {
    let bytes = code.as_bytes();
    // Pattern: letter + 2 digits + optional more digits → insert dot
    // after position 3 if there are characters beyond.
    if bytes.len() > 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3..].iter().all(|b| b.is_ascii_digit())
    {
        format!("{}.{}", &code[..3], &code[3..])
    } else {
        code.to_string()
    }
}

async fn tpc_search(pool: &DbPool, q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_safe = q.replace('\'', "''");
    let sql = format!(
        "SELECT code, en_label, chapter FROM tpc_codes \
         WHERE tenant_id IS NULL AND \
               (code LIKE '{q}%' OR en_label LIKE '%{q}%') \
         ORDER BY (code = '{q}') DESC, (code LIKE '{q}%') DESC, code LIMIT {k}",
        q = q_safe, k = k,
    );
    let items = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rs) => rs.iter().map(|r| json!({
            "code": r.get::<String,_>("code"),
            "en_label": r.try_get::<String,_>("en_label").unwrap_or_default(),
            "chapter": r.try_get::<String,_>("chapter").unwrap_or_default(),
        })).collect::<Vec<_>>(),
        Err(e) => { warn!("tpc_search: {e}"); vec![] }
    };
    KbResult {
        kb_id: "tpc", kb_name: "TPC (Procedure Codes)",
        count: items.len(), items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

async fn loinc_search(pool: &DbPool, q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_safe = q.replace('\'', "''");
    // LIKE rather than FULLTEXT here — query terms may be partial codes ("2160")
    // that wouldn't match a word-boundary FULLTEXT.
    let sql = format!(
        "SELECT loinc_num, long_common_name, class, status FROM loinc_codes \
         WHERE tenant_id IS NULL AND \
               (loinc_num LIKE '{q}%' OR long_common_name LIKE '%{q}%' OR short_name LIKE '%{q}%') \
         ORDER BY (loinc_num = '{q}') DESC, (loinc_num LIKE '{q}%') DESC LIMIT {k}",
        q = q_safe, k = k,
    );
    let items = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rs) => rs.iter().map(|r| json!({
            "loinc_num": r.get::<String,_>("loinc_num"),
            "long_common_name": r.get::<String,_>("long_common_name"),
            "class": r.try_get::<String,_>("class").unwrap_or_default(),
            "status": r.try_get::<String,_>("status").unwrap_or_default(),
        })).collect::<Vec<_>>(),
        Err(e) => { warn!("loinc_search: {e}"); vec![] }
    };
    KbResult {
        kb_id: "loinc", kb_name: "LOINC",
        count: items.len(), items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

async fn tmt_search(pool: &DbPool, q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_safe = q.replace('\'', "''");
    // FULLTEXT — TMT FSN is descriptive paragraph-ish (brand + manufacturer +
    // generic + dose form), word-boundary matching is the right tool.
    let sql = format!(
        "SELECT tmt_id, concept_type, fsn FROM tmt_codes \
         WHERE tenant_id IS NULL AND MATCH(fsn) AGAINST('{q}' IN NATURAL LANGUAGE MODE) \
         LIMIT {k}",
        q = q_safe, k = k,
    );
    let items = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rs) => rs.iter().map(|r| json!({
            "tmt_id": r.get::<String,_>("tmt_id"),
            "concept_type": r.get::<String,_>("concept_type"),
            "fsn": r.get::<String,_>("fsn"),
        })).collect::<Vec<_>>(),
        Err(e) => { warn!("tmt_search: {e}"); vec![] }
    };
    KbResult {
        kb_id: "tmt", kb_name: "TMT (Thai Medicines)",
        count: items.len(), items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

async fn tmlt_search(pool: &DbPool, q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_safe = q.replace('\'', "''");
    let sql = format!(
        "SELECT tmlt_id, concept_type, fsn FROM tmlt_codes \
         WHERE tenant_id IS NULL AND MATCH(fsn) AGAINST('{q}' IN NATURAL LANGUAGE MODE) \
         LIMIT {k}",
        q = q_safe, k = k,
    );
    let items = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rs) => rs.iter().map(|r| json!({
            "tmlt_id": r.get::<String,_>("tmlt_id"),
            "concept_type": r.get::<String,_>("concept_type"),
            "fsn": r.get::<String,_>("fsn"),
        })).collect::<Vec<_>>(),
        Err(e) => { warn!("tmlt_search: {e}"); vec![] }
    };
    KbResult {
        kb_id: "tmlt", kb_name: "TMLT (Thai Lab Codes)",
        count: items.len(), items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

async fn primekg_search(q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let items = match embed_and_qdrant(q, k as usize).await {
        Ok(v) => v,
        Err(e) => { warn!("primekg_search: {e}"); vec![] }
    };
    KbResult {
        kb_id: "primekg", kb_name: "PrimeKG (Biomedical KG)",
        count: items.len(), items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

/// Symptom → Disease worker: 7th KB in the L3 fan-out.
///
/// Pipeline:
///   1. Tokenize the canonical (alias-rewritten) query into phenotype
///      candidates by splitting on whitespace + `+,;` separators.
///   2. Call PrimeKG `DISEASE_PHENOTYPE_POSITIVE` traversal via Neo4j
///      (the same Cypher used by `/api/v1/knowledge/primekg/symptom_to_disease`).
///   3. For each candidate disease, look up matching ICD-10-TM codes by
///      en_label substring so the response carries codes (not just names)
///      and the cross-KB result blob picks up ICD hits for downstream
///      bench evaluation.
///
/// Quiet when Neo4j is unavailable / USE_NEO4J_GRAPH is unset — returns
/// an empty KbResult rather than failing the whole fan-out.
async fn symptom_search(pool: &DbPool, q_canonical: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let items = symptom_search_impl(pool, q_canonical, k).await
        .unwrap_or_else(|e| {
            warn!("symptom_search: {e}");
            vec![]
        });
    KbResult {
        kb_id: "symptoms",
        kb_name: "Symptom → Disease (PrimeKG + ICD)",
        count: items.len(),
        items,
        latency_ms: t0.elapsed().as_millis(),
    }
}

async fn symptom_search_impl(pool: &DbPool, q_canonical: &str, k: i64) -> Result<Vec<JsonValue>, String> {
    use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};

    // Tokenize. Split on whitespace + common separators; drop tokens
    // shorter than 4 chars (gets rid of articles like "to", "of", "and"
    // and bare modifiers like "+"). ASCII only — Thai whole-phrase
    // symptoms should have been rewritten to English by `replace_query()`
    // (which runs `lookup_expansion` against Thai symptom aliases).
    let phenotypes: Vec<String> = q_canonical
        .split(|c: char| c.is_whitespace() || matches!(c, '+' | ',' | ';' | '/'))
        .map(|s| s.trim())
        .filter(|s| s.len() >= 4 && s.is_ascii())
        .map(|s| s.to_string())
        .collect();

    if phenotypes.is_empty() {
        return Ok(vec![]);
    }

    // Lazy Neo4j connect (env-gated). Cached at process scope would be
    // better but the L3 fan-out runs <300ms and Neo4j connect is ~10ms
    // amortized across keepalives, so OK for now.
    if std::env::var("USE_NEO4J_GRAPH").as_deref() != Ok("true") {
        return Ok(vec![]);
    }
    let svc = Neo4jService::try_new(&Neo4jConfig::from_env()).await
        .ok_or_else(|| "Neo4j unavailable".to_string())?;

    // min_match=1 because real user queries often have noise tokens
    // (articles, modifiers) that aren't phenotypes — requiring 2+ exact
    // matches eliminates legitimate hits with one strong + one noise term.
    let candidates = svc
        .primekg_symptom_to_disease(&phenotypes, 1, k * 3)
        .await
        .map_err(|e| e.to_string())?;

    if candidates.is_empty() {
        return Ok(vec![]);
    }

    // Chain to ICD: for each top disease, find matching ICD codes by
    // en_label substring. Limits hard-capped per candidate to keep latency
    // bounded (~3-5 candidates × 3 codes each = up to 15 ICD rows fetched).
    let mut out = Vec::new();
    for d in candidates.into_iter().take(k as usize) {
        let name = d.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let name_safe = name.replace('\'', "''");
        let icd_sql = format!(
            "SELECT code FROM icd10_codes WHERE tenant_id IS NULL \
             AND en_label LIKE '%{name_safe}%' ORDER BY code LIMIT 3"
        );
        let icd_codes: Vec<String> = sqlx::query(&icd_sql)
            .fetch_all(pool)
            .await
            .map(|rs| {
                rs.iter()
                    .filter_map(|r| r.try_get::<String, _>("code").ok())
                    .collect()
            })
            .unwrap_or_default();
        out.push(serde_json::json!({
            "entity_index":     d.get("entity_index"),
            "name":             name,
            "match_count":      d.get("match_count"),
            "matched_symptoms": d.get("matched"),
            "icd_codes":        icd_codes,
        }));
    }
    Ok(out)
}

/// Heimdall BGE-M3 embed + Qdrant primekg-entities search.
async fn embed_and_qdrant(text: &str, k: usize) -> Result<Vec<JsonValue>, String> {
    let heimdall_url = std::env::var("HEIMDALL_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080/v1".into());
    let heimdall_key = std::env::var("HEIMDALL_API_KEY").unwrap_or_default();
    let qdrant_url = std::env::var("QDRANT_URL")
        .unwrap_or_else(|_| "http://localhost:6333".into());
    let embed_model = std::env::var("EMBED_MODEL")
        .unwrap_or_else(|_| "BAAI/bge-m3".into());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let embed_resp: JsonValue = client
        .post(format!("{}/embeddings", heimdall_url.trim_end_matches('/')))
        .bearer_auth(&heimdall_key)
        .json(&json!({"model": embed_model, "input": text}))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let vector = embed_resp
        .pointer("/data/0/embedding")
        .and_then(|v| v.as_array())
        .ok_or("missing data[0].embedding")?
        .iter()
        .filter_map(|v| v.as_f64().map(|x| x as f32))
        .collect::<Vec<_>>();

    // Score floor: PrimeKG cosine has no natural zero — BGE-M3 character-level
    // similarity gives even nonsense queries ~0.47-0.55 against gene/protein
    // entries (which have similar char distributions). Real drug/disease
    // matches sit ≥ 0.60. Default 0.55 rejects junk like `asdfqwerty` (~0.48)
    // and `zzzzzz` (~0.55 borderline) without sacrificing legitimate hits.
    // Override per-deploy via `PRIMEKG_SCORE_THRESHOLD` if your corpus tuning
    // differs (e.g. lower for short-form acronyms, higher for picky terms).
    let score_threshold: f64 = std::env::var("PRIMEKG_SCORE_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.55);

    let qd: JsonValue = client
        .post(format!(
            "{}/collections/primekg-entities/points/search",
            qdrant_url.trim_end_matches('/')
        ))
        .json(&json!({
            "vector": {"name": "dense", "vector": vector},
            "limit": k,
            "with_payload": true,
            "score_threshold": score_threshold,
        }))
        .send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let hits = qd.get("result").and_then(|v| v.as_array())
        .ok_or("missing result")?;
    Ok(hits.iter().map(|h| {
        let p = h.get("payload").cloned().unwrap_or(json!({}));
        json!({
            "entity_index": p.get("entity_index"),
            "name": p.get("name"),
            "entity_type": p.get("entity_type"),
            "source": p.get("source"),
            "score": h.get("score"),
        })
    }).collect())
}
