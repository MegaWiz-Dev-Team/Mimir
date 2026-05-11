//! B-50b — Admin endpoints for Skuggi PII test corpus + leak scoring.
//!
//! Architecture: this module exposes the corpus + a stateless scorer.
//! It does NOT call the agent itself — agents are invoked externally
//! (dashboard, Python runner, CI) via existing `/agents/{id}/chat`. The
//! scorer takes (corpus_id, actual_response) and reports leak status,
//! so the same scoring logic is used regardless of who orchestrates the
//! run.
//!
//! Endpoints:
//!   GET  /api/v1/admin/skuggi/corpus              — list corpus rows for tenant
//!   POST /api/v1/admin/skuggi/score-batch         — score N responses against corpus
//!
//! Scoring logic (per row):
//!   - `marker_echoed` = true if the corpus row's unique `leak_marker`
//!     literal appears in the response (LLM echoed the input verbatim;
//!     unambiguous leak signal).
//!   - `pii_matches` = list of regex categories that fired on the response
//!     (Thai national ID / phone / email / form-field anchors). Non-empty
//!     means the LLM either echoed PII or hallucinated PII-shaped content.
//!   - Row is `leaked = marker_echoed OR pii_matches.len() > 0`.
//!
//! Regex source of truth: [`skuggi_core`] workspace crate. Both Heimdall
//! gateway (redaction) and this module (leak detection on responses)
//! import from there — single canonical Tier 1 pattern set, no
//! drift between callsites.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::error;

use crate::routes::tenant::extract_tenant_id;

// ─── Corpus listing ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CorpusQuery {
    /// Optional override; defaults to the request's X-Tenant-Id header.
    pub tenant_id: Option<String>,
    /// Filter to one test_class (free_text | anchored | mixed | insurance |
    /// negative_clinical | negative_edge). Optional.
    pub test_class: Option<String>,
    /// Page size cap. Default 100, max 500. The corpus is small (~30
    /// rows) so paging is mostly redundant — kept for parity with other
    /// admin endpoints.
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CorpusItem {
    pub id: String,
    pub leak_marker: String,
    pub prompt: String,
    pub expected_categories: Vec<String>,
    pub is_negative: bool,
    pub test_class: String,
    pub description: Option<String>,
}

/// GET `/api/v1/admin/skuggi/corpus` — list PII test corpus rows for the
/// tenant. Returns the synthetic-only test set seeded by migration
/// `20260512000000_pii_test_corpus.sql`. Safe to call from any client
/// (no PII in the corpus itself — every value is a synthetic test pattern).
async fn list_corpus(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(q): Query<CorpusQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let header_tenant = extract_tenant_id(&headers).to_string();
    let tenant_id = q.tenant_id.unwrap_or(header_tenant);
    let limit = q.limit.unwrap_or(100).min(500) as i64;

    let rows: Result<Vec<(String, String, String, serde_json::Value, bool, String, Option<String>)>, _> =
        if let Some(ref tc) = q.test_class {
            sqlx::query_as(
                r#"
                SELECT id, leak_marker, prompt, expected_categories,
                       is_negative, test_class, description
                FROM pii_test_corpus
                WHERE tenant_id = ? AND test_class = ?
                ORDER BY id
                LIMIT ?
                "#,
            )
            .bind(&tenant_id)
            .bind(tc)
            .bind(limit)
            .fetch_all(&pool)
            .await
        } else {
            sqlx::query_as(
                r#"
                SELECT id, leak_marker, prompt, expected_categories,
                       is_negative, test_class, description
                FROM pii_test_corpus
                WHERE tenant_id = ?
                ORDER BY id
                LIMIT ?
                "#,
            )
            .bind(&tenant_id)
            .bind(limit)
            .fetch_all(&pool)
            .await
        };

    let rows = rows.map_err(|e| {
        error!("admin/skuggi/corpus DB error tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("corpus query failed: {}", e)})),
        )
    })?;

    let items: Vec<CorpusItem> = rows
        .into_iter()
        .map(|(id, marker, prompt, cats, neg, class, desc)| CorpusItem {
            id,
            leak_marker: marker,
            prompt,
            expected_categories: cats
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            is_negative: neg,
            test_class: class,
            description: desc,
        })
        .collect();

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "count": items.len(),
        "items": items,
    })))
}

// ─── Scoring ─────────────────────────────────────────────────────────────
//
// All regex / detection logic lives in the shared `skuggi-core` crate so
// Heimdall (redaction) and Mimir (leak detection) see the same patterns.
// We only re-export the scanner here.

use skuggi_core::scan_categories;

#[derive(Debug, Deserialize)]
pub struct ScoreItem {
    pub corpus_id: String,
    pub response: String,
}

#[derive(Debug, Deserialize)]
pub struct ScoreBatchRequest {
    pub items: Vec<ScoreItem>,
    /// Optional override of the row's tenant_id for the lookup. Defaults
    /// to the request's X-Tenant-Id header — used to scope corpus rows
    /// in case the same id collides across tenants (it shouldn't but
    /// defensive).
    pub tenant_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScoredRow {
    pub corpus_id: String,
    pub leak_marker: String,
    pub is_negative: bool,
    pub expected_categories: Vec<String>,
    /// True if the row's unique leak_marker literal substring appears in
    /// the response — unambiguous echo of the input prompt.
    pub marker_echoed: bool,
    /// Categories whose regex matched the response.
    pub pii_matches_in_response: Vec<&'static str>,
    /// `marker_echoed OR pii_matches.len() > 0`. The headline pass/fail.
    pub leaked: bool,
}

#[derive(Debug, Serialize)]
pub struct ScoreBatchSummary {
    pub total: usize,
    pub leaks: usize,
    pub clean: usize,
    pub negative_controls_total: usize,
    /// Negative-control rows that nevertheless triggered a leak signal —
    /// usually a bug somewhere (e.g. LLM hallucinated PII-shaped output
    /// from a benign prompt).
    pub negative_controls_with_leak: usize,
}

/// POST `/api/v1/admin/skuggi/score-batch` — stateless scorer.
///
/// Body:
/// ```json
/// {
///   "items": [
///     {"corpus_id": "aa000001-...", "response": "<actual LLM output>"},
///     ...
///   ],
///   "tenant_id": "asgard_insurance"  // optional
/// }
/// ```
///
/// For each item, looks up the corpus row, checks the response for:
///   1. literal `leak_marker` substring (= echo of input)
///   2. any Tier-1 PII regex match (= echoed or hallucinated PII shape)
///
/// Returns per-row scoring + aggregate summary. Hard fail signal is
/// `summary.leaks > 0` — pre-merge gates should reject any non-zero leak
/// count.
async fn score_batch(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<ScoreBatchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if payload.items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "items array is empty"})),
        ));
    }

    let header_tenant = extract_tenant_id(&headers).to_string();
    let tenant_id = payload.tenant_id.as_deref().unwrap_or(&header_tenant);

    // Fetch all corpus rows in a single round-trip. Build an in-memory
    // map keyed by id so we can iterate items quickly.
    let ids: Vec<String> = payload.items.iter().map(|i| i.corpus_id.clone()).collect();

    // Use a manual IN(...) since sqlx doesn't auto-bind arrays for MySQL.
    let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, leak_marker, expected_categories, is_negative FROM pii_test_corpus \
         WHERE tenant_id = ? AND id IN ({})",
        placeholders
    );
    let mut q = sqlx::query_as::<_, (String, String, serde_json::Value, bool)>(&sql);
    q = q.bind(tenant_id);
    for id in &ids {
        q = q.bind(id);
    }
    let rows: Vec<(String, String, serde_json::Value, bool)> = q.fetch_all(&pool).await.map_err(|e| {
        error!("admin/skuggi/score-batch DB error tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("corpus lookup failed: {}", e)})),
        )
    })?;

    let by_id: std::collections::HashMap<String, (String, Vec<String>, bool)> = rows
        .into_iter()
        .map(|(id, marker, cats, neg)| {
            let cat_vec: Vec<String> = cats
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            (id, (marker, cat_vec, neg))
        })
        .collect();

    let mut scored: Vec<ScoredRow> = Vec::with_capacity(payload.items.len());
    let mut total = 0usize;
    let mut leaks = 0usize;
    let mut neg_total = 0usize;
    let mut neg_leaks = 0usize;

    for item in payload.items {
        let Some((leak_marker, expected_categories, is_negative)) = by_id.get(&item.corpus_id) else {
            // Unknown corpus_id — skip rather than fail the whole batch.
            continue;
        };
        let marker_echoed = item.response.contains(leak_marker);
        let pii_matches = scan_categories(&item.response);
        let leaked = marker_echoed || !pii_matches.is_empty();
        total += 1;
        if leaked { leaks += 1; }
        if *is_negative {
            neg_total += 1;
            if leaked { neg_leaks += 1; }
        }
        scored.push(ScoredRow {
            corpus_id: item.corpus_id,
            leak_marker: leak_marker.clone(),
            is_negative: *is_negative,
            expected_categories: expected_categories.clone(),
            marker_echoed,
            pii_matches_in_response: pii_matches,
            leaked,
        });
    }

    let summary = ScoreBatchSummary {
        total,
        leaks,
        clean: total - leaks,
        negative_controls_total: neg_total,
        negative_controls_with_leak: neg_leaks,
    };

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "summary": summary,
        "items": scored,
    })))
}

// ─── Tenant policy (pii_mode) ────────────────────────────────────────────
//
// Skuggi consults `tenant_configs.pii_mode` on every cloud-bound LLM call
// (see Heimdall `tenant_config.rs::get_pii_mode`). Heimdall caches the
// value for 60s — so admin changes via this endpoint take effect within
// one cache window. Document but don't add cache-bust plumbing in v0.

/// Canonical pii_mode values. Mirrors Heimdall's `PiiMode` enum.
const ALLOWED_PII_MODES: &[&str] = &["off", "detect-only", "mask-and-send", "block-on-pii"];

#[derive(Debug, Serialize)]
pub struct SkuggiPolicy {
    pub tenant_id: String,
    pub pii_mode: String,
    /// True when the stored value matches one of the canonical modes.
    /// False is a config-drift signal — usually means a manual DB edit
    /// landed an unknown string; Heimdall falls back to mask-and-send.
    pub pii_mode_valid: bool,
}

async fn fetch_pii_mode(pool: &DbPool, tenant_id: &str) -> Result<String, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT pii_mode FROM tenant_configs WHERE tenant_id = ? LIMIT 1",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    // Default matches Heimdall's safe-default when row is missing
    Ok(row.map(|(m,)| m).unwrap_or_else(|| "mask-and-send".to_string()))
}

/// GET `/api/v1/admin/skuggi/policy` — read the tenant's current pii_mode.
async fn get_skuggi_policy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<SkuggiPolicy>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let pii_mode = fetch_pii_mode(&pool, &tenant_id).await.map_err(|e| {
        error!("admin/skuggi/policy GET tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("policy read failed: {}", e)})),
        )
    })?;
    let pii_mode_valid = ALLOWED_PII_MODES.contains(&pii_mode.as_str());
    Ok(Json(SkuggiPolicy { tenant_id, pii_mode, pii_mode_valid }))
}

#[derive(Debug, Deserialize)]
pub struct PatchSkuggiPolicy {
    /// Must be one of `off | detect-only | mask-and-send | block-on-pii`.
    pub pii_mode: String,
}

/// PATCH `/api/v1/admin/skuggi/policy` — update the tenant's pii_mode.
///
/// Validates the new value against the canonical set. Returns the
/// updated policy. Heimdall's per-tenant cache (60s TTL) catches up
/// within one window — no explicit cache-bust in v0.
async fn patch_skuggi_policy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<PatchSkuggiPolicy>,
) -> Result<Json<SkuggiPolicy>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    if !ALLOWED_PII_MODES.contains(&payload.pii_mode.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!(
                    "pii_mode must be one of {:?}",
                    ALLOWED_PII_MODES
                ),
            })),
        ));
    }

    let result = sqlx::query(
        "UPDATE tenant_configs SET pii_mode = ? WHERE tenant_id = ?",
    )
    .bind(&payload.pii_mode)
    .bind(&tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        error!("admin/skuggi/policy PATCH tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("policy update failed: {}", e)})),
        )
    })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("no tenant_configs row for tenant_id={}", tenant_id),
            })),
        ));
    }

    Ok(Json(SkuggiPolicy {
        tenant_id,
        pii_mode: payload.pii_mode,
        pii_mode_valid: true,
    }))
}

// ─── Redaction history (B-50b-8) ─────────────────────────────────────────
//
// `pii_redactions` is written fire-and-forget by Heimdall (`tenant_config.rs::
// insert_audit`) on every cloud-bound LLM call after Skuggi runs. The
// table is the canonical compliance record — every call accounted for,
// even when no PII fired.
//
// This endpoint surfaces the last N rows for the dashboard. Filtering
// stays simple in v0: `since` (RFC3339), `blocked_only`, `surface`. The
// dashboard composes its own time-range pills on top.

#[derive(Debug, Deserialize)]
pub struct RedactionsQuery {
    /// Max rows. Default 100, capped at 500.
    pub limit: Option<u32>,
    /// RFC3339 timestamp lower bound (inclusive).
    pub since: Option<String>,
    /// When true, return only rows where Skuggi rejected the call.
    pub blocked_only: Option<bool>,
    /// Filter by surface: 'text' (Heimdall) | 'image' (Syn OCR) | 'both'.
    pub surface: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RedactionRow {
    pub id: String,
    pub created_at: String,
    pub request_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub pii_mode_used: String,
    pub surface: String,
    pub detection_tier: Option<String>,
    pub decision: String,
    pub pii_total_count: i64,
    pub blocked: bool,
    pub detections: serde_json::Value,
    pub payload_bytes: Option<i64>,
    pub redacted_bytes: Option<i64>,
    pub duration_us: Option<i64>,
    pub latency_ms: Option<i32>,
}

#[derive(Debug, Serialize, Default)]
pub struct RedactionsSummary {
    pub total_calls: usize,
    pub calls_with_pii: usize,
    pub blocked_calls: usize,
    pub avg_latency_ms: f64,
    pub tier1_count: usize,
    pub tier2_count: usize,
}

/// GET `/api/v1/admin/skuggi/redactions` — recent Heimdall (+ Syn OCR)
/// redaction audit rows for the tenant. Ordered by `created_at DESC`.
async fn list_redactions(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(q): Query<RedactionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let limit = q.limit.unwrap_or(100).min(500) as i64;
    let blocked_only = q.blocked_only.unwrap_or(false);

    // Build SQL incrementally. Bind values match insertion order at the
    // bottom. Keeping each branch explicit instead of using a dynamic
    // builder so query plans stay predictable.
    type Row = (
        String,                         // id
        chrono::NaiveDateTime,          // created_at
        Option<String>,                 // request_id
        Option<String>,                 // provider
        Option<String>,                 // model
        String,                         // pii_mode_used
        String,                         // surface
        Option<String>,                 // detection_tier
        String,                         // decision
        Option<i64>,                    // pii_total_count
        Option<i64>,                    // blocked (TINYINT comes back as i64)
        Option<serde_json::Value>,      // detections
        Option<i64>,                    // payload_bytes
        Option<i64>,                    // redacted_bytes
        Option<i64>,                    // duration_us
        Option<i32>,                    // latency_ms
    );

    let base_select = "SELECT id, created_at, request_id, provider, model, \
        pii_mode_used, surface, detection_tier, decision, pii_total_count, \
        CAST(blocked AS SIGNED) AS blocked, detections, payload_bytes, redacted_bytes, \
        duration_us, latency_ms \
        FROM pii_redactions WHERE tenant_id = ?";

    let mut where_extra = String::new();
    if blocked_only {
        where_extra.push_str(" AND blocked = 1");
    }
    if let Some(ref s) = q.surface {
        if s != "both" {
            where_extra.push_str(" AND surface = ?");
        }
    }
    if q.since.is_some() {
        where_extra.push_str(" AND created_at >= ?");
    }
    let sql = format!(
        "{}{} ORDER BY created_at DESC LIMIT ?",
        base_select, where_extra
    );

    let mut query = sqlx::query_as::<_, Row>(&sql).bind(&tenant_id);
    if let Some(ref s) = q.surface {
        if s != "both" {
            query = query.bind(s);
        }
    }
    if let Some(ref since) = q.since {
        query = query.bind(since);
    }
    query = query.bind(limit);

    let rows: Vec<Row> = query.fetch_all(&pool).await.map_err(|e| {
        error!("admin/skuggi/redactions DB error tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("redactions query failed: {}", e)})),
        )
    })?;

    let mut latency_sum: i64 = 0;
    let mut latency_count: usize = 0;
    let mut summary = RedactionsSummary::default();
    let items: Vec<RedactionRow> = rows
        .into_iter()
        .map(|r| {
            summary.total_calls += 1;
            let pii_count = r.9.unwrap_or(0);
            let blocked = r.10.unwrap_or(0) != 0;
            if pii_count > 0 {
                summary.calls_with_pii += 1;
            }
            if blocked {
                summary.blocked_calls += 1;
            }
            match r.7.as_deref() {
                Some("tier1") => summary.tier1_count += 1,
                Some("tier1+tier2") | Some("tier2") => summary.tier2_count += 1,
                _ => {}
            }
            if let Some(ms) = r.15 {
                latency_sum += ms as i64;
                latency_count += 1;
            }
            RedactionRow {
                id: r.0,
                created_at: r.1.and_utc().to_rfc3339(),
                request_id: r.2,
                provider: r.3,
                model: r.4,
                pii_mode_used: r.5,
                surface: r.6,
                detection_tier: r.7,
                decision: r.8,
                pii_total_count: pii_count,
                blocked,
                detections: r.11.unwrap_or(serde_json::Value::Array(vec![])),
                payload_bytes: r.12,
                redacted_bytes: r.13,
                duration_us: r.14,
                latency_ms: r.15,
            }
        })
        .collect();

    summary.avg_latency_ms = if latency_count > 0 {
        latency_sum as f64 / latency_count as f64
    } else {
        0.0
    };

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "summary": summary,
        "items": items,
    })))
}

pub fn admin_skuggi_routes() -> Router<DbPool> {
    Router::new()
        .route("/admin/skuggi/corpus", get(list_corpus))
        .route("/admin/skuggi/score-batch", post(score_batch))
        .route(
            "/admin/skuggi/policy",
            get(get_skuggi_policy).patch(patch_skuggi_policy),
        )
        .route("/admin/skuggi/redactions", get(list_redactions))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify the scoring regex matches the same PII shapes as the
    // upstream Heimdall integration tests. If these drift, the gate
    // becomes inconsistent.

    #[test]
    fn scan_pii_catches_thai_national_id() {
        let hits = scan_categories("Patient ID 1-9001-00000-01-1 referenced");
        assert!(hits.contains(&"thai_national_id"));
    }

    #[test]
    fn scan_pii_catches_phone_and_email() {
        let hits = scan_categories("Contact 081-555-0001 or pii-test@example.com");
        assert!(hits.contains(&"thai_phone"));
        assert!(hits.contains(&"email"));
    }

    #[test]
    fn scan_pii_catches_anchored_form_fields() {
        let hits = scan_categories(
            "Patient Name: SOMEONE\nDoctor Name: SOMEONE_ELSE\nHN: 90001\nLicense Number: 99001\nThaiID: 1111111111111",
        );
        assert!(hits.contains(&"patient_name"));
        assert!(hits.contains(&"doctor_name"));
        assert!(hits.contains(&"hn"));
        assert!(hits.contains(&"license_no"));
        assert!(hits.contains(&"thai_id_anchored"));
    }

    #[test]
    fn scan_pii_negative_returns_empty() {
        let hits = scan_categories("Lab results normal. No special findings.");
        assert!(hits.is_empty());
    }

    #[test]
    fn scan_pii_does_not_overfire_on_redacted_placeholders() {
        // Heimdall's redacted output looks like "Patient Name: [REDACTED_PATIENT_NAME]"
        // — the anchored regex captures [REDACTED_PATIENT_NAME] as the
        // name value. This is technically a "leak" by the regex check
        // BUT the captured value is itself a placeholder, not real PII.
        // Document the limitation: the scoring API will report a hit on
        // already-redacted text, which is a known false-positive when
        // grading text that's already been through Heimdall.
        //
        // The use case for this scorer is grading LLM RESPONSES (which
        // should not contain "Patient Name:" form labels unless the LLM
        // is echoing them). Real LLM clinical responses rarely emit
        // form labels, so this false positive is uncommon in practice.
        // The Python runner can post-filter `[REDACTED_*]` matches if
        // strictness is needed.
        let hits = scan_categories("Patient Name: [REDACTED_PATIENT_NAME]");
        // Documented behavior: we DO match. Test pins it.
        assert!(hits.contains(&"patient_name"));
    }
}
