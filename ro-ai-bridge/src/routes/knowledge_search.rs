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
    let p1 = icd10_search(&pool, &q_replace, k);
    let p2 = tpc_search(&pool, &q_replace, k);
    let p3 = loinc_search(&pool, &q_replace, k);
    let p4 = tmt_search(&pool, &q_append, k);
    let p5 = tmlt_search(&pool, &q_append, k);
    let p6 = primekg_search(&q_replace, k);

    let (r1, r2, r3, r4, r5, r6) = tokio::join!(p1, p2, p3, p4, p5, p6);

    let results = vec![r1, r2, r3, r4, r5, r6];

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
        _ => None,
    })
}

/// Append the canonical form to the original query: `"HbA1c"` →
/// `"HbA1c glycated hemoglobin"`. For FULLTEXT MATCH() NATURAL LANGUAGE
/// where relevance score picks the most-specific matching FSN regardless
/// of which form (acronym vs long) is stored in the index.
fn expand_query(q: &str) -> String {
    let token = q.trim();
    if token.is_empty() {
        return q.to_string();
    }
    let first_token = token.split_whitespace().next().unwrap_or(token);
    match lookup_expansion(first_token) {
        Some(expanded) => format!("{token} {expanded}"),
        None => q.to_string(),
    }
}

/// Replace the first token with its canonical form: `"T2DM diet"` →
/// `"diabetes mellitus type 2 diet"`. For LIKE-based KBs (ICD/TPC/LOINC)
/// where appending the long form to a 4-char acronym never produces a
/// matching substring, and for semantic search where the natural-language
/// form gives a cleaner embedding signal than the acronym alone.
fn replace_query(q: &str) -> String {
    let token = q.trim();
    if token.is_empty() {
        return q.to_string();
    }
    let first_token = token.split_whitespace().next().unwrap_or(token);
    match lookup_expansion(first_token) {
        Some(expanded) => {
            // Slice the rest (including leading whitespace) and append.
            // Byte index = byte length of first_token (UTF-8 safe because
            // we sliced at a whitespace boundary or at end-of-string).
            let rest = token.get(first_token.len()..).unwrap_or("");
            format!("{expanded}{rest}")
        }
        None => q.to_string(),
    }
}

// ── code-pattern preprocessing ────────────────────────────────────────────
//
// ICD-10 codes are stored WITHOUT decimal dots in `icd10_codes.code`
// (e.g. `E119` not `E11.9`, `J189` not `J18.9`). M1 dataset + real users
// commonly type the dotted form. When the leading token of the query
// looks like an ICD-10 code (letter + ≥2 digits + optional `.digits`),
// also try the dot-stripped variant for the `code LIKE` clause.
//
// Pattern is conservative — only fires on standalone or leading code
// tokens; doesn't affect "metformin"-style searches.
fn icd_code_variants(q: &str) -> Vec<String> {
    let mut variants = vec![q.to_string()];
    let leading = q.split_whitespace().next().unwrap_or(q);
    let leading_upper = leading.to_uppercase();
    // Match: letter + 2-3 digits + optional ".digits"
    let bytes = leading_upper.as_bytes();
    let is_letter = !bytes.is_empty() && bytes[0].is_ascii_alphabetic();
    let rest_ok = bytes[1..].iter().all(|b| b.is_ascii_digit() || *b == b'.');
    if is_letter && rest_ok && leading_upper.contains('.') {
        let stripped = leading_upper.replace('.', "");
        variants.push(stripped);
    }
    variants
}

// ── per-KB workers ────────────────────────────────────────────────────────

async fn icd10_search(pool: &DbPool, q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_safe = q.replace('\'', "''");
    let variants = icd_code_variants(&q_safe);
    // Build the WHERE: code LIKE for every variant + en/th_label LIKE
    // for the raw query (label search doesn't benefit from dot-stripping).
    let mut code_likes: Vec<String> = variants
        .iter()
        .map(|v| format!("code LIKE '{v}%'"))
        .collect();
    code_likes.push(format!("en_label LIKE '%{q_safe}%'"));
    code_likes.push(format!("th_label LIKE '%{q_safe}%'"));
    let where_clause = code_likes.join(" OR ");
    // Use the *first* variant for the exact-match / prefix ordering
    // (raw query if no dot; stripped form if user typed dotted code).
    let order_pivot = variants.last().cloned().unwrap_or_else(|| q_safe.clone());
    let sql = format!(
        "SELECT code, en_label, th_label, chapter FROM icd10_codes \
         WHERE tenant_id IS NULL AND ({where_clause}) \
         ORDER BY (code = '{pivot}') DESC, (code LIKE '{pivot}%') DESC, code LIMIT {k}",
        pivot = order_pivot, k = k,
    );
    let items = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(rs) => rs.iter().map(|r| json!({
            "code": r.get::<String,_>("code"),
            "en_label": r.try_get::<String,_>("en_label").unwrap_or_default(),
            "th_label": r.try_get::<String,_>("th_label").unwrap_or_default(),
            "chapter": r.try_get::<String,_>("chapter").unwrap_or_default(),
        })).collect::<Vec<_>>(),
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
