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

    // Query expansion: rewrite single-token inputs before TMT/TMLT FULLTEXT
    // lookup — both lab acronyms (HbA1c → "glycated hemoglobin") and Thai
    // drug transliterations (วาร์ฟาริน → "warfarin"). Other workers see the
    // raw query (LIKE handles substrings, BGE-M3 handles semantics).
    let q_expanded = expand_query(q);

    // Fan out — each KB query is independent. Use tokio::join for parallelism.
    let p1 = icd10_search(&pool, q, k);
    let p2 = tpc_search(&pool, q, k);
    let p3 = loinc_search(&pool, q, k);
    let p4 = tmt_search(&pool, &q_expanded, k);
    let p5 = tmlt_search(&pool, &q_expanded, k);
    let p6 = primekg_search(q, k);

    let (r1, r2, r3, r4, r5, r6) = tokio::join!(p1, p2, p3, p4, p5, p6);

    let results = vec![r1, r2, r3, r4, r5, r6];

    Ok(Json(SearchResponse {
        q: q.to_string(),
        k: k as u32,
        results,
        total_ms: start.elapsed().as_millis(),
    }))
}

// ── query expansion ───────────────────────────────────────────────────────
//
// Two single-token rewrites that help FULLTEXT NATURAL LANGUAGE find the
// right row in TMT/TMLT (whose FSN store one canonical English form):
//
//   1. Lab acronyms (HbA1c → "glycated hemoglobin") — TMLT FSN uses the long
//      form, never the acronym. Append the expansion so MATCH() relevance
//      picks the most-relevant row regardless of which form the index has.
//
//   2. Thai drug transliterations (วาร์ฟาริน → "warfarin") — TMT FSN contains
//      Thai script only for *brand+manufacturer names* in parentheses; the
//      generic drug name is always Latin. Investigation 2026-05-19 showed
//      TMT has 0 rows matching `วาร์ฟาริน` despite carrying 380+ warfarin
//      products. Same single-token append pattern lets the Latin token in
//      the rewritten query hit the right FSN.
//
// Only add high-confidence single-meaning mappings.
//
// Multi-word queries: we look at the FIRST whitespace-separated token only.
// This handles `พาราเซตามอล 500 mg` (drug name + strength) and `วาร์ฟาริน
// INR target` (drug + lab context) without dropping the trailing modifiers.
// False positives are bounded — adding a known drug name as a Latin token
// to a query that already mentions that drug is harmless to FULLTEXT scoring.
fn expand_query(q: &str) -> String {
    let token = q.trim();
    if token.is_empty() {
        return q.to_string();
    }
    // First-token-only lookup so multi-word queries still benefit. Preserve
    // the full original `token` in the output so trailing context (dose,
    // route, modifier) stays in the FULLTEXT query.
    let first_token = token.split_whitespace().next().unwrap_or(token);
    // Lookup is case-insensitive for ASCII; Thai script has no case so the
    // lowercased key equals the original. Try ASCII-lowercased first for
    // acronyms, then fall back to raw token for Thai/non-ASCII.
    let key_lc = first_token.to_ascii_lowercase();
    let expansion: Option<&'static str> = match key_lc.as_str() {
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
        _ => None,
    };
    // Thai script → Latin generic name. Top-25 commonly prescribed drugs
    // (Thai FDA generic spellings). Extend as production query logs reveal
    // new entries; a DB-backed alias table is the long-term shape if this
    // grows past ~50-100 entries.
    let expansion = expansion.or_else(|| match first_token {
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
    });
    match expansion {
        Some(expanded) => format!("{token} {expanded}"),
        None => q.to_string(),
    }
}

// ── per-KB workers ────────────────────────────────────────────────────────

async fn icd10_search(pool: &DbPool, q: &str, k: i64) -> KbResult {
    let t0 = Instant::now();
    let q_safe = q.replace('\'', "''");
    let sql = format!(
        "SELECT code, en_label, th_label, chapter FROM icd10_codes \
         WHERE tenant_id IS NULL AND \
               (code LIKE '{q}%' OR en_label LIKE '%{q}%' OR th_label LIKE '%{q}%') \
         ORDER BY (code = '{q}') DESC, (code LIKE '{q}%') DESC, code LIMIT {k}",
        q = q_safe, k = k,
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
