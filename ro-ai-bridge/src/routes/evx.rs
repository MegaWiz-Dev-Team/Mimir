//! Unified evaluation API (evx_* layer).
//!
//! - GET  /api/v1/eval/scoreboard      — one cross-family scoreboard
//! - POST /api/v1/eval/evx/runs        — ingest ONE run (target + run + metrics)
//!
//! Backed by the `evx_*` tables: one row per (target, dataset) run with its
//! primary metric. Works identically for QA / RAG / OCR / OCR-layout / NER /
//! coding / STT / TTS — the UI registry decides how to label and colour each
//! family. `family` is a free VARCHAR by design (documented vocabulary lives
//! in the column COMMENT), so new families need no DDL change.
//!
//! Ingest is the evx-native analogue of `eval_ocr_layout::create_run` (the
//! endpoint syn-eval-ingest POSTs to): an external bench (e.g. a Sága STT/TTS
//! eval) POSTs its finished result once, server-side we upsert the target,
//! optionally register the dataset, and insert run + metric rows in one
//! transaction. `evx_target.id` is deterministic — SHA-256 of the natural key
//! `kind|name|model_id|runtime|quant` (absent parts = "") — so repeated
//! ingests of the same variant converge on one target row without a lookup.
//! (Backfilled targets from 20260604120100 use per-family legacy keys and may
//! not converge with native ones; that is accepted and documented there.)
//!
//! Tenant: scoreboard rows are scoped to the caller's tenant (TenantContext),
//! plus the cross-cutting engineering rows (tenant_id IS NULL). Ingest reads
//! `X-Tenant-Id` directly: absent header → tenant_id NULL, the engineering
//! bucket every tenant's scoreboard can see — matching the view's semantics
//! rather than `eval_ocr_layout`'s `asgard_platform` default.

use axum::{
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::services::db::DbPool;

#[derive(Debug, Serialize, FromRow)]
pub struct ScoreboardRow {
    pub family: String,
    pub run_id: String,
    pub experiment_id: Option<String>,
    pub tenant_id: Option<String>,
    pub target_kind: String,
    pub target_name: String,
    pub model_id: Option<String>,
    pub runtime: Option<String>,
    pub dataset_id: Option<String>,
    pub n_items: i32,
    pub primary_metric: Option<String>,
    pub primary_value: Option<f64>,
    pub unit: Option<String>,
    pub higher_is_better: Option<i8>,
    pub ci_low: Option<f64>,
    pub ci_high: Option<f64>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ScoreboardQuery {
    /// Optional family filter (qa | rag | ocr | ocr_layout | ner | coding ...).
    pub family: Option<String>,
}

async fn get_scoreboard(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Query(q): Query<ScoreboardQuery>,
) -> Json<Vec<ScoreboardRow>> {
    let rows = sqlx::query_as::<_, ScoreboardRow>(
        "SELECT family, run_id, experiment_id, tenant_id, target_kind, target_name,
                model_id, runtime, dataset_id, n_items, primary_metric,
                primary_value, unit, higher_is_better, ci_low, ci_high, finished_at
         FROM evx_scoreboard
         WHERE (tenant_id = ? OR tenant_id IS NULL)
           AND (? IS NULL OR family = ?)
         ORDER BY family ASC, finished_at DESC",
    )
    .bind(&tenant.tenant_id)
    .bind(&q.family)
    .bind(&q.family)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "evx_scoreboard_failed", tenant = %tenant.tenant_id, error = %e);
        Vec::new()
    });

    Json(rows)
}

// ─── Ingest request types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IngestTarget {
    /// model | agent | pipeline | runtime_variant (evx_target.kind).
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub quant: Option<String>,
    #[serde(default)]
    pub config: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct IngestDataset {
    /// Explicit evx_dataset.id; default `{family}:{name}:v{version}`.
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default = "default_dataset_version")]
    pub version: i32,
    #[serde(default)]
    pub item_count: i32,
    /// none | pseudonymized | raw (evx_dataset.pii_sensitivity).
    #[serde(default = "default_pii_sensitivity")]
    pub pii_sensitivity: String,
    #[serde(default)]
    pub spec: Option<Value>,
}

fn default_dataset_version() -> i32 {
    1
}
fn default_pii_sensitivity() -> String {
    "none".into()
}
fn default_higher_is_better() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct IngestMetric {
    /// cer | wer | accuracy | p95_latency_ms ... (evx_metric.name vocabulary).
    pub name: String,
    #[serde(default)]
    pub slice_dim: String,
    #[serde(default)]
    pub slice_val: String,
    pub value: f64,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default = "default_higher_is_better")]
    pub higher_is_better: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub ci_low: Option<f64>,
    #[serde(default)]
    pub ci_high: Option<f64>,
    #[serde(default)]
    pub n: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct IngestRunRequest {
    /// qa | rag | ocr | ocr_layout | ner | coding | stt | tts | rubric ...
    pub family: String,
    /// Client-stable id for idempotent re-ingest; default server UUID.
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub experiment_id: Option<String>,
    /// PENDING | RUNNING | COMPLETED | FAILED; default COMPLETED.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub n_items: i32,
    #[serde(default)]
    pub git_sha: Option<String>,
    #[serde(default)]
    pub judge_model: Option<String>,
    #[serde(default)]
    pub config: Option<Value>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    pub target: IngestTarget,
    #[serde(default)]
    pub dataset: Option<IngestDataset>,
    pub metrics: Vec<IngestMetric>,
}

#[derive(Debug, Serialize)]
pub struct IngestRunResponse {
    pub run_id: String,
    pub target_id: String,
    pub dataset_id: Option<String>,
    pub metrics_created: i64,
}

// ─── Ingest validation (pure — shared by handler and unit tests) ───────────

const ALLOWED_TARGET_KINDS: [&str; 4] = ["model", "agent", "pipeline", "runtime_variant"];
const ALLOWED_RUN_STATUSES: [&str; 4] = ["PENDING", "RUNNING", "COMPLETED", "FAILED"];
const ALLOWED_PII_SENSITIVITY: [&str; 3] = ["none", "pseudonymized", "raw"];

fn check_len(label: &str, s: &str, max: usize) -> Result<(), String> {
    if s.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if s.len() > max {
        return Err(format!("{label} exceeds {max} chars"));
    }
    Ok(())
}

fn validate_ingest_request(payload: &IngestRunRequest) -> Result<(), String> {
    check_len("family", &payload.family, 30)?;
    if !payload
        .family
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "family must match [a-z0-9_]+; got {:?}",
            payload.family
        ));
    }

    if !ALLOWED_TARGET_KINDS.contains(&payload.target.kind.as_str()) {
        return Err(format!(
            "target.kind must be one of {ALLOWED_TARGET_KINDS:?}; got {:?}",
            payload.target.kind
        ));
    }
    check_len("target.name", &payload.target.name, 120)?;

    if let Some(ref id) = payload.run_id {
        check_len("run_id", id, 64)?;
    }
    if let Some(ref id) = payload.experiment_id {
        check_len("experiment_id", id, 36)?;
    }
    if let Some(ref s) = payload.status {
        if !ALLOWED_RUN_STATUSES.contains(&s.as_str()) {
            return Err(format!(
                "status must be one of {ALLOWED_RUN_STATUSES:?}; got {s:?}"
            ));
        }
    }
    if let Some(ref s) = payload.git_sha {
        check_len("git_sha", s, 40)?;
    }
    if let Some(ref s) = payload.judge_model {
        check_len("judge_model", s, 100)?;
    }

    if let Some(ref d) = payload.dataset {
        check_len("dataset.name", &d.name, 160)?;
        if d.version < 1 {
            return Err("dataset.version must be >= 1".into());
        }
        if !ALLOWED_PII_SENSITIVITY.contains(&d.pii_sensitivity.as_str()) {
            return Err(format!(
                "dataset.pii_sensitivity must be one of {ALLOWED_PII_SENSITIVITY:?}; got {:?}",
                d.pii_sensitivity
            ));
        }
        check_len("dataset.id", &derive_dataset_id(&payload.family, d), 80)?;
    }

    if payload.metrics.is_empty() {
        return Err("metrics must contain at least one row".into());
    }
    let mut primaries = 0;
    let mut seen: Vec<(&str, &str, &str)> = Vec::with_capacity(payload.metrics.len());
    for m in &payload.metrics {
        check_len("metric.name", &m.name, 60)?;
        if m.slice_dim.len() > 40 {
            return Err(format!("metric.slice_dim exceeds 40 chars: {:?}", m.slice_dim));
        }
        if m.slice_val.len() > 120 {
            return Err(format!("metric.slice_val exceeds 120 chars: {:?}", m.slice_val));
        }
        if let Some(ref u) = m.unit {
            check_len("metric.unit", u, 20)?;
        }
        if let (Some(lo), Some(hi)) = (m.ci_low, m.ci_high) {
            if lo > hi {
                return Err(format!("metric {:?}: ci_low {lo} > ci_high {hi}", m.name));
            }
        }
        if m.is_primary {
            primaries += 1;
        }
        let key = (m.name.as_str(), m.slice_dim.as_str(), m.slice_val.as_str());
        if seen.contains(&key) {
            return Err(format!(
                "duplicate metric (name, slice_dim, slice_val) = {key:?}"
            ));
        }
        seen.push(key);
    }
    // uk_one_primary enforces this in the DB too — pre-check for a clean 400.
    if primaries > 1 {
        return Err(format!("at most one metric may be is_primary; got {primaries}"));
    }

    Ok(())
}

/// Natural key behind the deterministic `evx_target.id` — absent parts as ""
/// so the same variant always serializes identically.
fn target_natural_key(t: &IngestTarget) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        t.kind,
        t.name,
        t.model_id.as_deref().unwrap_or(""),
        t.runtime.as_deref().unwrap_or(""),
        t.quant.as_deref().unwrap_or("")
    )
}

fn derive_target_id(t: &IngestTarget) -> String {
    let mut hasher = Sha256::new();
    hasher.update(target_natural_key(t).as_bytes());
    format!("{:x}", hasher.finalize())
}

fn derive_dataset_id(family: &str, d: &IngestDataset) -> String {
    d.id
        .clone()
        .unwrap_or_else(|| format!("{family}:{}:v{}", d.name, d.version))
}

/// Ingest tenant: explicit `X-Tenant-Id`, else NULL — the cross-cutting
/// engineering bucket that every tenant's scoreboard read includes.
fn resolve_ingest_tenant(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn json_str(v: &Option<Value>) -> Option<String> {
    v.as_ref()
        .map(|j| serde_json::to_string(j).unwrap_or_else(|_| "{}".into()))
}

// ─── Ingest handler ────────────────────────────────────────────────────────

/// POST /api/v1/eval/evx/runs
async fn ingest_run(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(payload): Json<IngestRunRequest>,
) -> Result<(StatusCode, Json<IngestRunResponse>), (StatusCode, Json<Value>)> {
    if let Err(msg) = validate_ingest_request(&payload) {
        tracing::warn!(event = "evx_ingest_rejected", reason = %msg);
        return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))));
    }

    let tenant = resolve_ingest_tenant(&headers);
    let target_id = derive_target_id(&payload.target);
    let run_id = payload
        .run_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let dataset_id = payload
        .dataset
        .as_ref()
        .map(|d| derive_dataset_id(&payload.family, d));

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return err500("begin tx", e),
    };

    // INSERT IGNORE: deterministic id → repeated ingests of the same variant
    // converge on one target row (same trick as the backfill script).
    if let Err(e) = sqlx::query(
        "INSERT IGNORE INTO evx_target (id, kind, name, model_id, runtime, quant, config_json)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&target_id)
    .bind(&payload.target.kind)
    .bind(&payload.target.name)
    .bind(&payload.target.model_id)
    .bind(&payload.target.runtime)
    .bind(&payload.target.quant)
    .bind(json_str(&payload.target.config))
    .execute(&mut *tx)
    .await
    {
        return err500("insert evx_target", e);
    }

    if let Some(ref d) = payload.dataset {
        if let Err(e) = sqlx::query(
            "INSERT IGNORE INTO evx_dataset
                (id, family, tenant_id, name, version, item_count, pii_sensitivity, spec_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(dataset_id.as_deref())
        .bind(&payload.family)
        .bind(&tenant)
        .bind(&d.name)
        .bind(d.version)
        .bind(d.item_count)
        .bind(&d.pii_sensitivity)
        .bind(json_str(&d.spec))
        .execute(&mut *tx)
        .await
        {
            return err500("insert evx_dataset", e);
        }
    }

    let insert_run = sqlx::query(
        "INSERT INTO evx_run
            (id, experiment_id, family, target_id, dataset_id, dataset_version,
             tenant_id, status, n_items, git_sha, judge_model, config_json,
             started_at, finished_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(&payload.experiment_id)
    .bind(&payload.family)
    .bind(&target_id)
    .bind(dataset_id.as_deref())
    .bind(payload.dataset.as_ref().map(|d| d.version))
    .bind(&tenant)
    .bind(payload.status.as_deref().unwrap_or("COMPLETED"))
    .bind(payload.n_items)
    .bind(&payload.git_sha)
    .bind(&payload.judge_model)
    .bind(json_str(&payload.config))
    .bind(payload.started_at.unwrap_or_else(Utc::now))
    .bind(payload.finished_at)
    .execute(&mut *tx)
    .await;

    if let Err(e) = insert_run {
        let duplicate = e
            .as_database_error()
            .map(|d| d.is_unique_violation())
            .unwrap_or(false);
        if duplicate {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({ "error": format!("run_id {run_id:?} already exists") })),
            ));
        }
        return err500("insert evx_run", e);
    }

    let mut metrics_created: i64 = 0;
    for m in &payload.metrics {
        if let Err(e) = sqlx::query(
            "INSERT INTO evx_metric
                (run_id, name, slice_dim, slice_val, value, unit,
                 higher_is_better, is_primary, ci_low, ci_high, n)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&run_id)
        .bind(&m.name)
        .bind(&m.slice_dim)
        .bind(&m.slice_val)
        .bind(m.value)
        .bind(&m.unit)
        .bind(m.higher_is_better)
        .bind(m.is_primary)
        .bind(m.ci_low)
        .bind(m.ci_high)
        .bind(m.n)
        .execute(&mut *tx)
        .await
        {
            return err500("insert evx_metric", e);
        }
        metrics_created += 1;
    }

    if let Err(e) = tx.commit().await {
        return err500("commit tx", e);
    }

    tracing::info!(
        event = "evx_run_ingested",
        run_id = %run_id,
        family = %payload.family,
        target = %payload.target.name,
        tenant = tenant.as_deref().unwrap_or("<eng>"),
        metrics_created,
    );
    Ok((
        StatusCode::CREATED,
        Json(IngestRunResponse {
            run_id,
            target_id,
            dataset_id,
            metrics_created,
        }),
    ))
}

fn err500<T, E: std::fmt::Display>(label: &str, e: E) -> Result<T, (StatusCode, Json<Value>)> {
    tracing::error!(event = "evx_ingest_failed", step = label, error = %e);
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": format!("{label}: {e}") })),
    ))
}

pub fn evx_routes() -> Router<DbPool> {
    Router::new()
        .route("/api/v1/eval/scoreboard", get(get_scoreboard))
        .route("/api/v1/eval/evx/runs", post(ingest_run))
        // same tenant-auth layer eval_routes() applies — injects TenantContext
        .layer(axum::middleware::from_fn(tenant_auth_middleware))
}

// ─── Tests ─────────────────────────────────────────────────────────────────
// Pure-logic coverage (no DB): natural-key format, id derivation, request
// validation. The DB roundtrip needs a live MariaDB and is exercised by the
// runbook's end-to-end check (POST → GET /api/v1/eval/scoreboard).

#[cfg(test)]
mod tests {
    use super::*;

    fn req(v: Value) -> IngestRunRequest {
        serde_json::from_value(v).expect("valid IngestRunRequest json")
    }

    fn base() -> Value {
        json!({
            "family": "stt",
            "target": { "kind": "model", "name": "whisper-large-v3", "runtime": "mlx", "quant": "q4" },
            "metrics": [
                { "name": "cer", "value": 0.12, "unit": "ratio", "higher_is_better": false, "is_primary": true,
                  "ci_low": 0.10, "ci_high": 0.14, "n": 500 },
                { "name": "wer", "value": 0.21, "unit": "ratio", "higher_is_better": false }
            ]
        })
    }

    #[test]
    fn valid_minimal_request_passes() {
        assert_eq!(validate_ingest_request(&req(base())), Ok(()));
    }

    #[test]
    fn target_natural_key_pins_format() {
        let r = req(base());
        assert_eq!(target_natural_key(&r.target), "model|whisper-large-v3||mlx|q4");
    }

    #[test]
    fn target_id_is_deterministic_sha256_hex() {
        let r = req(base());
        let a = derive_target_id(&r.target);
        let b = derive_target_id(&r.target);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

        let mut other = base();
        other["target"]["quant"] = json!("q8");
        assert_ne!(derive_target_id(&req(other).target), a);
    }

    #[test]
    fn dataset_id_derived_or_explicit() {
        let d: IngestDataset =
            serde_json::from_value(json!({ "name": "thai-clinic-stt", "version": 2 })).unwrap();
        assert_eq!(derive_dataset_id("stt", &d), "stt:thai-clinic-stt:v2");

        let e: IngestDataset =
            serde_json::from_value(json!({ "id": "stt:legacy-uuid", "name": "x" })).unwrap();
        assert_eq!(derive_dataset_id("stt", &e), "stt:legacy-uuid");
    }

    #[test]
    fn tts_family_needs_no_special_case() {
        let mut v = base();
        v["family"] = json!("tts");
        v["metrics"] = json!([{ "name": "mos", "value": 4.2, "is_primary": true }]);
        assert_eq!(validate_ingest_request(&req(v)), Ok(()));
    }

    #[test]
    fn rejects_bad_family() {
        let too_long = "x".repeat(31);
        for bad in ["", "STT", "has space", too_long.as_str()] {
            let mut v = base();
            v["family"] = json!(bad);
            assert!(validate_ingest_request(&req(v)).is_err(), "family {bad:?}");
        }
    }

    #[test]
    fn rejects_unknown_target_kind() {
        let mut v = base();
        v["target"]["kind"] = json!("engine");
        let err = validate_ingest_request(&req(v)).unwrap_err();
        assert!(err.contains("target.kind"), "{err}");
    }

    #[test]
    fn rejects_empty_metrics() {
        let mut v = base();
        v["metrics"] = json!([]);
        let err = validate_ingest_request(&req(v)).unwrap_err();
        assert!(err.contains("at least one"), "{err}");
    }

    #[test]
    fn rejects_two_primary_metrics() {
        let mut v = base();
        v["metrics"][1]["is_primary"] = json!(true);
        let err = validate_ingest_request(&req(v)).unwrap_err();
        assert!(err.contains("is_primary"), "{err}");
    }

    #[test]
    fn rejects_duplicate_metric_key() {
        let mut v = base();
        v["metrics"][1] = v["metrics"][0].clone();
        v["metrics"][1]["is_primary"] = json!(false);
        let err = validate_ingest_request(&req(v)).unwrap_err();
        assert!(err.contains("duplicate metric"), "{err}");
    }

    #[test]
    fn slice_rows_with_same_name_are_not_duplicates() {
        let mut v = base();
        v["metrics"] = json!([
            { "name": "cer", "value": 0.12, "is_primary": true },
            { "name": "cer", "slice_dim": "doc_type", "slice_val": "handwriting", "value": 0.19 }
        ]);
        assert_eq!(validate_ingest_request(&req(v)), Ok(()));
    }

    #[test]
    fn rejects_inverted_ci() {
        let mut v = base();
        v["metrics"][0]["ci_low"] = json!(0.5);
        v["metrics"][0]["ci_high"] = json!(0.1);
        let err = validate_ingest_request(&req(v)).unwrap_err();
        assert!(err.contains("ci_low"), "{err}");
    }

    #[test]
    fn rejects_bad_status_and_pii() {
        let mut v = base();
        v["status"] = json!("done");
        assert!(validate_ingest_request(&req(v)).is_err());

        let mut v = base();
        v["dataset"] = json!({ "name": "d", "pii_sensitivity": "secret" });
        let err = validate_ingest_request(&req(v)).unwrap_err();
        assert!(err.contains("pii_sensitivity"), "{err}");
    }

    #[test]
    fn ingest_tenant_header_or_engineering_null() {
        let mut h = HeaderMap::new();
        assert_eq!(resolve_ingest_tenant(&h), None);
        h.insert("x-tenant-id", "  ".parse().unwrap());
        assert_eq!(resolve_ingest_tenant(&h), None);
        h.insert("x-tenant-id", "asgard_medical".parse().unwrap());
        assert_eq!(resolve_ingest_tenant(&h), Some("asgard_medical".into()));
    }
}
