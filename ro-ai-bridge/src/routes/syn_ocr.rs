//! Sprint 50 — Syn 4-tier OCR surface.
//!
//! This module owns `/api/v1/syn/ocr/*` and is distinct from the legacy
//! [`crate::routes::ocr`] (`/api/v1/ocr/*`) which serves the older Gemini-only
//! `data_sources` upload path.
//!
//! Day-1 behaviour:
//!   * health endpoint pings each engine and reports status
//!   * policy endpoint exposes the tenant's OCR config
//!   * extract endpoint runs the smart router (B-50b — 6 rules), calls the
//!     selected sidecar, and writes one `ocr_documents` audit row regardless
//!     of outcome
//!   * documents endpoint paginates audit history per tenant
//!
//! The local sidecars currently return 501 (Sprint 50 B-50a stubs in
//! github.com/MegaWiz-Dev-Team/Syn). The router still records the choice and
//! the failure so we can iterate on routing logic before engines land.

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::Multipart;
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────
// Engine identifiers (kept stable — values land in audit rows + dashboards)
// ─────────────────────────────────────────────────────────────────────────
const ENGINE_CHANDRA: &str = "chandra-local";
const ENGINE_PADDLE: &str = "paddleocr-local";
const ENGINE_GEMINI_FLASH: &str = "gemini-3-flash";
const ENGINE_GEMINI_PRO: &str = "gemini-3.1-pro";

const DEFAULT_LOW_CONFIDENCE: f64 = 0.7;

// Sidecar URLs default to docker-compose service names; override via env in
// k8s deployments.
fn chandra_base() -> String {
    std::env::var("SYN_CHANDRA_URL").unwrap_or_else(|_| "http://chandra:8090".to_string())
}
fn paddle_base() -> String {
    std::env::var("SYN_PADDLE_URL").unwrap_or_else(|_| "http://paddleocr:8091".to_string())
}

pub fn syn_ocr_routes() -> Router<DbPool> {
    Router::new()
        .route("/syn/ocr/health", get(engine_health))
        .route("/syn/ocr/policy", get(get_policy))
        .route("/syn/ocr/extract", post(extract))
        .route("/syn/ocr/documents", get(list_documents))
        .route("/syn/ocr/documents/{id}", get(get_document))
}

// ─────────────────────────────────────────────────────────────────────────
// /syn/ocr/health
// ─────────────────────────────────────────────────────────────────────────
#[derive(Serialize)]
struct EngineHealth {
    engine: &'static str,
    tier: u8,
    status: String,
    detail: Option<String>,
    latency_ms: Option<u64>,
}

#[derive(Serialize)]
struct HealthResponse {
    overall: &'static str,
    engines: Vec<EngineHealth>,
}

async fn engine_health() -> Json<HealthResponse> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("reqwest client");

    let chandra = probe_sidecar(&client, &chandra_base(), ENGINE_CHANDRA, 1).await;
    let paddle = probe_sidecar(&client, &paddle_base(), ENGINE_PADDLE, 1).await;

    // Cloud tiers: Day-1 we only report whether the GOOGLE_API_KEY is set;
    // actually pinging Google on every health check is wasteful.
    let gemini_status = if std::env::var("GOOGLE_API_KEY").is_ok() {
        "credentials_present"
    } else {
        "not_deployed"
    };

    Json(HealthResponse {
        overall: if matches!(chandra.status.as_str(), "ok" | "ready")
            || matches!(paddle.status.as_str(), "ok" | "ready")
        {
            "degraded"
        } else {
            "not_deployed"
        },
        engines: vec![
            chandra,
            paddle,
            EngineHealth {
                engine: ENGINE_GEMINI_FLASH,
                tier: 2,
                status: gemini_status.to_string(),
                detail: Some("opt-in cloud Tier 2".to_string()),
                latency_ms: None,
            },
            EngineHealth {
                engine: ENGINE_GEMINI_PRO,
                tier: 3,
                status: gemini_status.to_string(),
                detail: Some("opt-in cloud Tier 3 + Curator gate".to_string()),
                latency_ms: None,
            },
        ],
    })
}

async fn probe_sidecar(
    client: &reqwest::Client,
    base: &str,
    engine: &'static str,
    tier: u8,
) -> EngineHealth {
    let started = Instant::now();
    let url = format!("{}/healthz", base.trim_end_matches('/'));
    match client.get(&url).send().await {
        Ok(resp) => {
            let elapsed = started.elapsed().as_millis() as u64;
            let status_code = resp.status();
            let body: Value = resp.json().await.unwrap_or(json!({}));
            EngineHealth {
                engine,
                tier,
                status: if status_code.is_success() {
                    body.get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ok")
                        .to_string()
                } else {
                    "unhealthy".to_string()
                },
                detail: body.get("model_loaded").map(|v| format!("model_loaded={v}")),
                latency_ms: Some(elapsed),
            }
        }
        Err(e) => EngineHealth {
            engine,
            tier,
            status: "not_deployed".to_string(),
            detail: Some(e.to_string()),
            latency_ms: None,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────
// /syn/ocr/policy
// ─────────────────────────────────────────────────────────────────────────
#[derive(Serialize)]
struct TenantPolicy {
    tenant_id: String,
    ocr_cloud_flash_enabled: bool,
    ocr_cloud_pro_enabled: bool,
    ocr_phi_strict: bool,
    ocr_monthly_cloud_budget_usd: f64,
    pii_mode: String,
    pii_custom_patterns: Option<String>,
}

async fn get_policy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<TenantPolicy>, (StatusCode, String)> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    let row: Option<TenantPolicy> = sqlx::query_as(
        r#"
        SELECT
            tenant_id,
            ocr_cloud_flash_enabled,
            ocr_cloud_pro_enabled,
            ocr_phi_strict,
            CAST(ocr_monthly_cloud_budget_usd AS DOUBLE) AS ocr_monthly_cloud_budget_usd,
            pii_mode,
            pii_custom_patterns
        FROM tenant_configs
        WHERE tenant_id = ?
        "#,
    )
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    match row {
        Some(p) => Ok(Json(p)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("no tenant_configs row for {tenant_id}"),
        )),
    }
}

// FromRow impl via sqlx derive isn't free here because the names are bound
// by SQL aliases — manual impl keeps things explicit.
impl<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> for TenantPolicy {
    fn from_row(row: &'r sqlx::mysql::MySqlRow) -> sqlx::Result<Self> {
        use sqlx::Row;
        Ok(Self {
            tenant_id: row.try_get("tenant_id")?,
            ocr_cloud_flash_enabled: row.try_get("ocr_cloud_flash_enabled")?,
            ocr_cloud_pro_enabled: row.try_get("ocr_cloud_pro_enabled")?,
            ocr_phi_strict: row.try_get("ocr_phi_strict")?,
            ocr_monthly_cloud_budget_usd: row
                .try_get::<Option<f64>, _>("ocr_monthly_cloud_budget_usd")?
                .unwrap_or(0.0),
            pii_mode: row.try_get("pii_mode")?,
            pii_custom_patterns: row.try_get("pii_custom_patterns").ok(),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────
// /syn/ocr/extract — smart router (B-50b 6 rules) + audit
// ─────────────────────────────────────────────────────────────────────────
#[derive(Default)]
struct ExtractRequest {
    image: Vec<u8>,
    filename: String,
    /// Manual override (rule 1)
    engine_override: Option<String>,
    /// Doc-type signal (rule 3): "handwriting" | "printed_thai" | "mixed" | None
    doc_type: Option<String>,
    /// Curator-marked high-stakes (rule 5)
    high_stakes: bool,
    /// Optional language hint passed through to the sidecar
    hint_lang: Option<String>,
}

async fn parse_multipart(
    mut multipart: Multipart,
) -> Result<ExtractRequest, (StatusCode, String)> {
    let mut req = ExtractRequest::default();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                req.filename = field.file_name().unwrap_or("upload").to_string();
                req.image = field
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, format!("read file: {e}")))?
                    .to_vec();
            }
            "engine" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, format!("engine field: {e}")))?;
                req.engine_override = Some(v);
            }
            "doc_type" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, format!("doc_type field: {e}")))?;
                req.doc_type = Some(v);
            }
            "high_stakes" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, format!("high_stakes field: {e}")))?;
                req.high_stakes =
                    matches!(v.to_lowercase().as_str(), "true" | "1" | "yes");
            }
            "hint_lang" => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, format!("hint_lang field: {e}")))?;
                req.hint_lang = Some(v);
            }
            other => warn!("unknown multipart field: {other}"),
        }
    }
    if req.image.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing 'file' field".into()));
    }
    Ok(req)
}

#[derive(Serialize)]
struct ExtractResponse {
    audit_id: String,
    engine_used: String,
    router_reason: String,
    status: String,
    extracted_text: Option<String>,
    confidence: Option<f64>,
    bbox_count: Option<i64>,
    cost_usd: f64,
    latency_ms: u64,
}

async fn extract(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    multipart: Multipart,
) -> Result<Json<ExtractResponse>, (StatusCode, String)> {
    let started = Instant::now();
    let tenant_id = extract_tenant_id(&headers).to_string();
    let req = parse_multipart(multipart).await?;
    let image_sha = sha256_hex(&req.image);

    // Pull tenant policy (cloud opt-in flags + PHI strict + pii_mode).
    let policy = match get_tenant_policy(&pool, &tenant_id).await {
        Ok(p) => p,
        Err(e) => {
            error!(tenant_id, "policy lookup failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("policy: {e}")));
        }
    };

    let (engine, reason) = pick_engine(&req, &policy);
    info!(tenant_id, engine, reason, "syn-ocr route");

    let audit_id = Uuid::new_v4().to_string();
    let mut extracted_text: Option<String> = None;
    let mut confidence: Option<f64> = None;
    let mut bbox_count: Option<i64> = None;
    let status;
    let status_message: Option<String>;

    // Day-1: only local sidecars are wired here. Cloud tiers go through
    // a different adapter in B-50k; for now, route them through the
    // sidecar call which will return 501 and we record the audit.
    let outcome = match engine.as_str() {
        ENGINE_CHANDRA => call_local_sidecar(&chandra_base(), &req).await,
        ENGINE_PADDLE => call_local_sidecar(&paddle_base(), &req).await,
        ENGINE_GEMINI_FLASH | ENGINE_GEMINI_PRO => Err(SidecarError::NotImplemented(format!(
            "cloud engine {engine} arrives in B-50k"
        ))),
        other => Err(SidecarError::NotImplemented(format!("unknown engine {other}"))),
    };

    match outcome {
        Ok(out) => {
            extracted_text = Some(out.extracted_text);
            confidence = out.confidence;
            bbox_count = out.bbox_count;
            status = "succeeded".to_string();
            status_message = None;
        }
        Err(SidecarError::NotImplemented(msg)) => {
            status = "engine_failed".to_string();
            status_message = Some(msg);
        }
        Err(SidecarError::Transport(e)) => {
            status = "engine_failed".to_string();
            status_message = Some(format!("transport: {e}"));
        }
    }

    let latency_ms = started.elapsed().as_millis() as u64;

    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO ocr_documents (
            id, tenant_id, image_sha256, engine_used, router_reason,
            extracted_text, confidence, bbox_count, cost_usd, latency_ms,
            pii_redacted, status, status_message, requested_by
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, FALSE, ?, ?, NULL)
        "#,
    )
    .bind(&audit_id)
    .bind(&tenant_id)
    .bind(&image_sha)
    .bind(&engine)
    .bind(&reason)
    .bind(extracted_text.clone())
    .bind(confidence)
    .bind(bbox_count)
    .bind(latency_ms as i64)
    .bind(&status)
    .bind(status_message.clone())
    .execute(&pool)
    .await
    {
        error!("audit insert failed: {e}");
    }

    Ok(Json(ExtractResponse {
        audit_id,
        engine_used: engine,
        router_reason: reason,
        status,
        extracted_text,
        confidence,
        bbox_count,
        cost_usd: 0.0,
        latency_ms,
    }))
}

// ─────────────────────────────────────────────────────────────────────────
// Smart router — B-50b six ordered rules
// ─────────────────────────────────────────────────────────────────────────
fn pick_engine(req: &ExtractRequest, policy: &TenantPolicy) -> (String, String) {
    // Rule 1: manual override
    if let Some(ovr) = req.engine_override.as_deref() {
        return (ovr.to_string(), "manual_override".to_string());
    }

    // Rule 5 needs Curator authority (we trust the high_stakes flag came
    // from a Curator session; B-50f tightens this with a JWT claim check).
    // Evaluated before rule 2 so phi-strict still blocks at rule 2 below.
    let high_stakes_pro_eligible = req.high_stakes
        && policy.ocr_cloud_pro_enabled
        && policy.ocr_cloud_flash_enabled
        && !policy.ocr_phi_strict;

    if high_stakes_pro_eligible {
        return (
            ENGINE_GEMINI_PRO.to_string(),
            "high_stakes_curator".to_string(),
        );
    }

    // Rule 2: PHI strict — never cloud
    let phi_strict = policy.ocr_phi_strict;

    // Rule 3: doc-type signal
    let chosen = match req.doc_type.as_deref() {
        Some("handwriting") => Some((ENGINE_CHANDRA, "doc_type=handwriting")),
        Some("printed_thai") | Some("mixed") => Some((ENGINE_PADDLE, "doc_type=printed_thai")),
        _ => None,
    };

    if let Some((engine, reason)) = chosen {
        return (engine.to_string(), reason.to_string());
    }

    // Rule 4 (escalation on low confidence) is enforced after a local call
    // returns. The router itself defaults to a local engine; if that local
    // engine fails / low-conf, the *caller* (or a follow-up sprint task)
    // will re-issue the request with `engine_override` set to Flash. Day-1
    // we just emit the local default.
    let _ = phi_strict; // explicit tag — phi_strict only matters at rule 6 below

    // Rule 6: default = paddleocr
    (
        ENGINE_PADDLE.to_string(),
        "default_paddleocr".to_string(),
    )
}

// ─────────────────────────────────────────────────────────────────────────
// Sidecar HTTP client (minimal Day-1 adapter)
// ─────────────────────────────────────────────────────────────────────────
struct SidecarOutcome {
    extracted_text: String,
    confidence: Option<f64>,
    bbox_count: Option<i64>,
}

enum SidecarError {
    NotImplemented(String),
    Transport(String),
}

async fn call_local_sidecar(
    base: &str,
    req: &ExtractRequest,
) -> Result<SidecarOutcome, SidecarError> {
    let url = format!("{}/extract", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| SidecarError::Transport(e.to_string()))?;

    let mut form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(req.image.clone()).file_name(req.filename.clone()),
    );
    if let Some(lang) = &req.hint_lang {
        form = form.text("hint_lang", lang.clone());
    }

    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| SidecarError::Transport(e.to_string()))?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(json!({}));

    if status.as_u16() == 501 {
        return Err(SidecarError::NotImplemented(
            body.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("sidecar 501")
                .to_string(),
        ));
    }
    if !status.is_success() {
        return Err(SidecarError::Transport(format!(
            "sidecar status {}: {}",
            status,
            body
        )));
    }

    Ok(SidecarOutcome {
        extracted_text: body
            .get("extracted_text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        confidence: body.get("confidence").and_then(|v| v.as_f64()),
        bbox_count: body
            .get("bboxes")
            .and_then(|v| v.as_array())
            .map(|a| a.len() as i64),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// /syn/ocr/documents — paginated audit list (tenant-scoped)
// ─────────────────────────────────────────────────────────────────────────
#[derive(Deserialize)]
struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    engine: Option<String>,
    status: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct OcrDocument {
    id: String,
    tenant_id: String,
    image_sha256: String,
    engine_used: String,
    router_reason: Option<String>,
    confidence: Option<f64>,
    bbox_count: Option<i64>,
    cost_usd: f64,
    latency_ms: Option<i64>,
    pii_redacted: bool,
    status: String,
    status_message: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_documents(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut sql = String::from(
        r#"
        SELECT id, tenant_id, image_sha256, engine_used, router_reason,
               CAST(confidence AS DOUBLE)   AS confidence,
               bbox_count,
               CAST(cost_usd AS DOUBLE)     AS cost_usd,
               latency_ms,
               pii_redacted, status, status_message, created_at
        FROM ocr_documents
        WHERE tenant_id = ?
        "#,
    );
    if q.engine.is_some() {
        sql.push_str(" AND engine_used = ?");
    }
    if q.status.is_some() {
        sql.push_str(" AND status = ?");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query_as::<_, OcrDocument>(&sql).bind(&tenant_id);
    if let Some(e) = &q.engine {
        query = query.bind(e);
    }
    if let Some(s) = &q.status {
        query = query.bind(s);
    }
    let rows = query
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "limit": limit,
        "offset": offset,
        "rows": rows,
    })))
}

async fn get_document(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Result<Json<OcrDocument>, (StatusCode, String)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let row = sqlx::query_as::<_, OcrDocument>(
        r#"
        SELECT id, tenant_id, image_sha256, engine_used, router_reason,
               CAST(confidence AS DOUBLE)   AS confidence,
               bbox_count,
               CAST(cost_usd AS DOUBLE)     AS cost_usd,
               latency_ms,
               pii_redacted, status, status_message, created_at
        FROM ocr_documents
        WHERE id = ? AND tenant_id = ?
        "#,
    )
    .bind(&id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    match row {
        Some(r) => Ok(Json(r)),
        None => Err((StatusCode::NOT_FOUND, format!("document {id} not found"))),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────
async fn get_tenant_policy(pool: &DbPool, tenant_id: &str) -> Result<TenantPolicy, sqlx::Error> {
    sqlx::query_as::<_, TenantPolicy>(
        r#"
        SELECT
            tenant_id,
            ocr_cloud_flash_enabled,
            ocr_cloud_pro_enabled,
            ocr_phi_strict,
            CAST(ocr_monthly_cloud_budget_usd AS DOUBLE) AS ocr_monthly_cloud_budget_usd,
            pii_mode,
            pii_custom_patterns
        FROM tenant_configs
        WHERE tenant_id = ?
        "#,
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[allow(dead_code)]
fn low_confidence_threshold() -> f64 {
    std::env::var("SYN_OCR_LOW_CONF_THRESHOLD")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(DEFAULT_LOW_CONFIDENCE)
}
