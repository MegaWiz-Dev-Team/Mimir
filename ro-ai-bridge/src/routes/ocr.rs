use crate::config::Config;
use crate::routes::ocr_audit::{
    get_ocr_policy, insert_ocr_audit, OcrAuditRow, OcrStatus,
};
use crate::routes::ocr_budget::{
    check_budget, current_month_spend, estimate_pre_call_cost, BudgetCheckError, TierIntent,
};
use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Extension, Json, Router,
};
use axum_extra::extract::Multipart;
use mimir_core_ai::models::sources::DataSource;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::ocr;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

/// Subset of Syn's `ExtractResponse` that Mimir cares about. Syn's full
/// schema lives in `Syn/services/api/src/routes.rs::ExtractResponse`.
#[derive(Debug, Deserialize)]
struct SynExtractResponse {
    audit_id: String,
    engine_used: String,
    router_reason: String,
    status: String,
    #[serde(default)]
    extracted_text: Option<String>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    bbox_count: Option<i64>,
    #[serde(default)]
    cost_usd: f64,
    #[serde(default)]
    latency_ms: u64,
}

/// B-50b Path A — delegate an OCR call to Syn's smart-router instead of
/// hitting Gemini directly. Syn picks the engine (paddleocr / typhoon / Gemini
/// Flash / Gemini Pro) per its 6-rule router + writes its own audit row.
/// Mimir's B-50e audit is in addition to Syn's, providing a cross-system view.
async fn delegate_to_syn(
    syn_base: &str,
    tenant_id: &str,
    image_bytes: Vec<u8>,
    filename: String,
    engine_override: Option<String>,
    doc_type: Option<String>,
    high_stakes: bool,
) -> Result<SynExtractResponse, String> {
    let url = format!(
        "{}/api/v1/syn/ocr/extract",
        syn_base.trim_end_matches('/')
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| format!("syn client init failed: {e}"))?;

    let mut form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(image_bytes).file_name(filename),
    );
    if let Some(e) = engine_override {
        form = form.text("engine_override", e);
    }
    if let Some(d) = doc_type {
        form = form.text("doc_type", d);
    }
    if high_stakes {
        form = form.text("high_stakes", "true");
    }

    let resp = client
        .post(&url)
        .header("X-Tenant-Id", tenant_id)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("syn POST failed: {e}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("syn body read failed: {e}"))?;

    if !status.is_success() {
        return Err(format!("syn returned {}: {}", status, body));
    }

    serde_json::from_str::<SynExtractResponse>(&body)
        .map_err(|e| format!("syn response parse failed: {e} — body: {body}"))
}

/// Estimate USD cost for an OCR call. Treats tokens as output-side since OCR
/// produces text. Falls back to 0 if pricing not in `model_pricing` table —
/// matches `insights::estimate_cost` behavior.
async fn estimate_ocr_cost(pool: &DbPool, model_id: &str, tokens_used: u32) -> f64 {
    let pricing: Option<(f64, f64)> = sqlx::query_as(
        "SELECT CAST(input_per_1m_usd AS DOUBLE), CAST(output_per_1m_usd AS DOUBLE)
         FROM model_pricing WHERE model_id = ?",
    )
    .bind(model_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    if let Some((_, output_per_m)) = pricing {
        (tokens_used as f64) / 1_000_000.0 * output_per_m
    } else {
        0.0
    }
}

/// Map a (model, provider) tuple to the audit engine identifier defined in
/// ADR-006. Until B-50a sidecars land, only the cloud tiers populate; local
/// engines will be added when the smart-router (B-50b) goes live.
fn engine_id_for(model: &str, provider: &str) -> String {
    let m = model.to_ascii_lowercase();
    if m.contains("gemini-3.1-pro") || m.contains("gemini-2.5-pro") {
        "gemini-3.1-pro".to_string()
    } else if m.contains("gemini-3") || m.contains("gemini-2.5-flash") || m.contains("flash") {
        "gemini-3-flash".to_string()
    } else if m.contains("typhoon") {
        "typhoon-ocr-local".to_string()
    } else if m.contains("paddleocr") {
        "paddleocr-local".to_string()
    } else if matches!(provider, "google" | "gemini") {
        // Generic Gemini fallback — assume Flash since it's the default cloud tier.
        "gemini-3-flash".to_string()
    } else if !provider.is_empty() {
        provider.to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn ocr_routes() -> Router<DbPool> {
    Router::new()
        .route("/ocr/extract", post(ocr_extract))
        .route("/ocr/extract-source/{id}", post(ocr_extract_source))
        // B-50m admin: read + update tenant OCR policy (cloud opt-in flags +
        // monthly budget). Mimir-side direct-DB so the dashboard has one
        // API surface to talk to; Syn has an equivalent endpoint but talking
        // to it requires extra plumbing through Mimir's auth.
        .route(
            "/ocr/admin/policy",
            axum::routing::get(get_admin_policy).patch(patch_admin_policy),
        )
        // Recent OCR calls for this tenant — SQL-backed view of ocr_documents
        // (B-50e audit table). Used by the dashboard "Recent OCR Calls" table.
        // Laminar still provides span-tree drill-down on a per-call basis via
        // the link-out card; this endpoint is hot-path / same-blast-radius.
        .route("/ocr/admin/recent", axum::routing::get(get_admin_recent))
}

#[derive(Debug, Deserialize)]
pub struct PatchOcrPolicy {
    pub ocr_cloud_flash_enabled: Option<bool>,
    pub ocr_cloud_pro_enabled: Option<bool>,
    pub ocr_phi_strict: Option<bool>,
    pub ocr_monthly_cloud_budget_usd: Option<f64>,
}

/// GET `/api/v1/ocr/admin/policy` — returns the tenant's current OCR policy
/// plus live month-to-date cloud spend. The dashboard reads this to render
/// the "Cost guard" admin card + budget editor.
async fn get_admin_policy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let policy = get_ocr_policy(&pool, &tenant_id).await;
    let spend = current_month_spend(&pool, &tenant_id).await;
    let remaining = if policy.monthly_budget_usd > 0.0 {
        Some((policy.monthly_budget_usd - spend).max(0.0))
    } else {
        None
    };
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "ocr_phi_strict": policy.phi_strict,
        "ocr_cloud_flash_enabled": policy.cloud_flash_enabled,
        "ocr_cloud_pro_enabled": policy.cloud_pro_enabled,
        "ocr_monthly_cloud_budget_usd": policy.monthly_budget_usd,
        "current_month_spend_usd": spend,
        "current_month_remaining_usd": remaining,
        "pii_mode": policy.pii_mode,
    })))
}

/// PATCH `/api/v1/ocr/admin/policy` — partial update of tenant_configs OCR
/// cols. Only non-NULL fields in the body are applied. Validates budget ≥ 0
/// and Pro-tier requires Flash-tier.
async fn patch_admin_policy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<PatchOcrPolicy>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    if let Some(b) = payload.ocr_monthly_cloud_budget_usd {
        if b < 0.0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "ocr_monthly_cloud_budget_usd must be ≥ 0 (use 0 for 'no cap')"
                })),
            ));
        }
    }
    // Pro requires Flash (mirrors Syn's validation + ADR-006 rule)
    if matches!(payload.ocr_cloud_pro_enabled, Some(true))
        && matches!(payload.ocr_cloud_flash_enabled, Some(false))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "enabling Tier 3 Pro requires Tier 2 Flash to also be enabled (ADR-006)"
            })),
        ));
    }

    let mut sets: Vec<&str> = Vec::new();
    if payload.ocr_phi_strict.is_some() { sets.push("ocr_phi_strict = ?"); }
    if payload.ocr_cloud_flash_enabled.is_some() { sets.push("ocr_cloud_flash_enabled = ?"); }
    if payload.ocr_cloud_pro_enabled.is_some() { sets.push("ocr_cloud_pro_enabled = ?"); }
    if payload.ocr_monthly_cloud_budget_usd.is_some() {
        sets.push("ocr_monthly_cloud_budget_usd = ?");
    }
    if sets.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no fields to update"})),
        ));
    }

    let sql = format!(
        "UPDATE tenant_configs SET {} WHERE tenant_id = ?",
        sets.join(", ")
    );
    let mut q = sqlx::query(&sql);
    if let Some(v) = payload.ocr_phi_strict { q = q.bind(v); }
    if let Some(v) = payload.ocr_cloud_flash_enabled { q = q.bind(v); }
    if let Some(v) = payload.ocr_cloud_pro_enabled { q = q.bind(v); }
    if let Some(v) = payload.ocr_monthly_cloud_budget_usd { q = q.bind(v); }
    q = q.bind(&tenant_id);

    let result = q.execute(&pool).await.map_err(|e| {
        error!("PATCH ocr policy DB error tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("update failed: {}", e)})),
        )
    })?;

    info!(
        "Tenant {} OCR policy patched: {} row(s) affected",
        tenant_id,
        result.rows_affected()
    );

    // Return the updated policy for confirmation
    let policy = get_ocr_policy(&pool, &tenant_id).await;
    let spend = current_month_spend(&pool, &tenant_id).await;
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "rows_affected": result.rows_affected(),
        "ocr_phi_strict": policy.phi_strict,
        "ocr_cloud_flash_enabled": policy.cloud_flash_enabled,
        "ocr_cloud_pro_enabled": policy.cloud_pro_enabled,
        "ocr_monthly_cloud_budget_usd": policy.monthly_budget_usd,
        "current_month_spend_usd": spend,
    })))
}

#[derive(Debug, Deserialize)]
pub struct RecentOcrQuery {
    /// Page size. Defaults to 20, capped at 100 to keep the dashboard payload
    /// small. The dashboard table only renders recent activity; long-tail
    /// drill-down belongs in Laminar.
    pub limit: Option<u32>,
    /// Filter by status (e.g. "succeeded", "budget_exceeded"). Optional.
    pub status: Option<String>,
}

/// GET `/api/v1/ocr/admin/recent` — most-recent rows from `ocr_documents` for
/// this tenant. Backs the dashboard "Recent OCR Calls" table. We do NOT
/// proxy Laminar here: SQL is hot-path / same-blast-radius as Mimir, and the
/// audit table already has cost/engine/status/latency. The Laminar link-out
/// card handles deep span-tree drill-down on a per-row basis.
async fn get_admin_recent(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    axum::extract::Query(q): axum::extract::Query<RecentOcrQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let limit = q.limit.unwrap_or(20).min(100) as i64;

    // Two query shapes: with and without status filter. Keeping them separate
    // avoids dynamic SQL building for a single optional filter.
    let rows: Result<Vec<(String, String, Option<String>, Option<f64>, Option<i32>, f64, Option<i32>, bool, String, Option<String>, chrono::NaiveDateTime, Option<String>)>, _> =
        if let Some(ref status_filter) = q.status {
            sqlx::query_as(
                r#"
                SELECT id, engine_used, router_reason, CAST(confidence AS DOUBLE) AS confidence,
                       bbox_count, CAST(cost_usd AS DOUBLE) AS cost_usd, latency_ms,
                       pii_redacted, status, status_message, created_at, requested_by
                FROM ocr_documents
                WHERE tenant_id = ? AND status = ?
                ORDER BY created_at DESC
                LIMIT ?
                "#,
            )
            .bind(&tenant_id)
            .bind(status_filter)
            .bind(limit)
            .fetch_all(&pool)
            .await
        } else {
            sqlx::query_as(
                r#"
                SELECT id, engine_used, router_reason, CAST(confidence AS DOUBLE) AS confidence,
                       bbox_count, CAST(cost_usd AS DOUBLE) AS cost_usd, latency_ms,
                       pii_redacted, status, status_message, created_at, requested_by
                FROM ocr_documents
                WHERE tenant_id = ?
                ORDER BY created_at DESC
                LIMIT ?
                "#,
            )
            .bind(&tenant_id)
            .bind(limit)
            .fetch_all(&pool)
            .await
        };

    let rows = rows.map_err(|e| {
        error!("GET ocr recent DB error tenant={}: {}", tenant_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("query failed: {}", e)})),
        )
    })?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, engine, reason, conf, bbox, cost, latency, pii, status, msg, created, user)| {
            json!({
                "id": id,
                "engine_used": engine,
                "router_reason": reason,
                "confidence": conf,
                "bbox_count": bbox,
                "cost_usd": cost,
                "latency_ms": latency,
                "pii_redacted": pii,
                "status": status,
                "status_message": msg,
                "created_at": created.and_utc().to_rfc3339(),
                "requested_by": user,
            })
        })
        .collect();

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "count": items.len(),
        "items": items,
    })))
}

/// POST /api/v1/ocr/extract
///
/// Accept multipart/form-data with an image file and return extracted text via Gemini.
/// Fields:
/// - `file`: Binary image data (jpg, png, gif, webp, bmp, tiff, pdf)
/// - `model`: Optional model override (default: gemini-2.5-flash)
async fn ocr_extract(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    // B-50b-A: accept Syn router hints via multipart fields. The legacy
    // `model` field maps to Syn's `engine_override` for back-compat.
    let mut engine_override: Option<String> = None;
    let mut doc_type: Option<String> = None;
    let mut high_stakes = false;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Invalid multipart data: {}", e)})),
        )
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed to read file: {}", e)})),
                    )
                })?;
                file_data = Some(bytes.to_vec());
            }
            "model" | "engine_override" => {
                engine_override = Some(field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid engine_override field: {}", e)})),
                    )
                })?);
            }
            "doc_type" => {
                doc_type = Some(field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid doc_type field: {}", e)})),
                    )
                })?);
            }
            "high_stakes" => {
                let v = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid high_stakes field: {}", e)})),
                    )
                })?;
                high_stakes = matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes");
            }
            _ => {
                warn!("Unknown OCR field: {}", field_name);
            }
        }
    }

    let data = file_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing 'file' field"})),
        )
    })?;
    let filename = file_name.unwrap_or_else(|| "upload.png".to_string());

    if !ocr::is_ocr_capable(&filename) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("File type not supported for OCR: {}", filename),
                "supported": ["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "pdf"]
            })),
        ));
    }

    let tenant_id = extract_tenant_id(&headers).to_string();
    let user_override = engine_override.is_some();

    // B-50e policy + B-50m budget gate: enforce BEFORE delegating so a
    // budget-blown tenant never reaches Syn (saves the cloud-API call cost
    // and gives the user a fast 402).
    let policy = get_ocr_policy(&pool, &tenant_id).await;
    let intent = TierIntent::from_hints(engine_override.as_deref(), high_stakes);
    let current_spend = current_month_spend(&pool, &tenant_id).await;
    let est_cost = estimate_pre_call_cost(intent, high_stakes);

    if let Err(err) = check_budget(&policy, intent, current_spend, est_cost) {
        let (status_code, ocr_status, audit_msg) = match &err {
            BudgetCheckError::PhiStrict => (
                StatusCode::FORBIDDEN,
                OcrStatus::PiiStrictBlock,
                err.to_string(),
            ),
            BudgetCheckError::BudgetExceeded { .. } => (
                StatusCode::PAYMENT_REQUIRED,
                OcrStatus::BudgetExceeded,
                err.to_string(),
            ),
        };
        warn!(
            "OCR budget gate rejected (tenant={}, intent={:?}): {}",
            tenant_id, intent, err
        );

        // B-50e audit the rejection so operators see the block in ocr_documents.
        let _ = insert_ocr_audit(
            &pool,
            OcrAuditRow {
                tenant_id: &tenant_id,
                image_bytes: &data,
                engine_used: "budget_gate",
                engine_version: None,
                router_reason: Some("pre_call_budget_check"),
                extracted_text: None,
                confidence: None,
                bbox_count: None,
                cost_usd: 0.0,
                latency_ms: Some(0),
                pii_redacted: false,
                status: ocr_status,
                status_message: Some(&audit_msg),
                image_path: None,
                requested_by: None,
            },
        )
        .await;

        return Err((
            status_code,
            Json(json!({
                "error": audit_msg,
                "policy": {
                    "phi_strict": policy.phi_strict,
                    "monthly_budget_usd": policy.monthly_budget_usd,
                    "current_month_spend_usd": current_spend,
                    "cloud_flash_enabled": policy.cloud_flash_enabled,
                    "cloud_pro_enabled": policy.cloud_pro_enabled,
                }
            })),
        ));
    }

    info!(
        "OCR extract: filename={}, doc_type={:?}, engine_override={:?}, high_stakes={}, intent={:?}, spend=${:.4}/{:.2} → delegating to Syn",
        filename, doc_type, engine_override, high_stakes, intent, current_spend, policy.monthly_budget_usd
    );

    let started = Instant::now();
    let syn_result = delegate_to_syn(
        &config.syn_api_url,
        &tenant_id,
        data.clone(),
        filename.clone(),
        engine_override.clone(),
        doc_type,
        high_stakes,
    )
    .await;
    let latency_ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;

    let local_router_reason = if user_override {
        "manual_override"
    } else {
        "delegate_to_syn"
    };

    match syn_result {
        Ok(syn) => {
            let cross_ref = format!("syn_audit_id={}", syn.audit_id);
            // B-50e: write OCR audit row in Mimir's audit layer. Cross-link
            // Syn's audit_id via status_message so operators can join the two
            // tables when investigating a request.
            let mimir_audit_id = insert_ocr_audit(
                &pool,
                OcrAuditRow {
                    tenant_id: &tenant_id,
                    image_bytes: &data,
                    engine_used: &syn.engine_used,
                    engine_version: None,
                    router_reason: Some(&syn.router_reason),
                    extracted_text: syn.extracted_text.as_deref(),
                    confidence: syn.confidence,
                    bbox_count: syn.bbox_count.map(|c| c as i32),
                    cost_usd: syn.cost_usd,
                    latency_ms: Some(latency_ms),
                    pii_redacted: false,
                    status: status_from_syn(&syn.status),
                    status_message: Some(&cross_ref),
                    image_path: None,
                    requested_by: None,
                },
            )
            .await;

            Ok(Json(json!({
                "mimir_audit_id": mimir_audit_id,
                "syn_audit_id": syn.audit_id,
                "content": syn.extracted_text.unwrap_or_default(),
                "engine_used": syn.engine_used,
                "router_reason": syn.router_reason,
                "status": syn.status,
                "confidence": syn.confidence,
                "bbox_count": syn.bbox_count,
                "cost_usd": syn.cost_usd,
                "filename": filename,
                "mimir_latency_ms": latency_ms,
                "syn_latency_ms": syn.latency_ms,
            })))
        }
        Err(e) => {
            error!("Syn delegation failed: {}", e);
            let msg = format!("Syn OCR delegation failed: {}", e);

            let _ = insert_ocr_audit(
                &pool,
                OcrAuditRow {
                    tenant_id: &tenant_id,
                    image_bytes: &data,
                    engine_used: "syn_delegation",
                    engine_version: None,
                    router_reason: Some(local_router_reason),
                    extracted_text: None,
                    confidence: None,
                    bbox_count: None,
                    cost_usd: 0.0,
                    latency_ms: Some(latency_ms),
                    pii_redacted: false,
                    status: OcrStatus::EngineFailed,
                    status_message: Some(&msg),
                    image_path: None,
                    requested_by: None,
                },
            )
            .await;

            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": msg})),
            ))
        }
    }
}

/// Map Syn's status string back to our OcrStatus enum for the audit row.
fn status_from_syn(s: &str) -> OcrStatus {
    match s {
        "succeeded" => OcrStatus::Succeeded,
        "pii_blocked" => OcrStatus::PiiBlocked,
        "budget_exceeded" => OcrStatus::BudgetExceeded,
        "pii_strict_block" => OcrStatus::PiiStrictBlock,
        _ => OcrStatus::EngineFailed,
    }
}

#[derive(Debug, Deserialize)]
pub struct ExtractSourceOcrRequest {
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// POST /api/v1/ocr/extract-source/:id
///
/// Run OCR on an existing data source's file (downloaded from S3).
/// Updates the source's raw_markdown and adds OCR metadata.
async fn ocr_extract_source(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    payload: Option<Json<ExtractSourceOcrRequest>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Fetch source
    let source = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let source = source.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Source not found"})),
        )
    })?;

    let s3_key = source.s3_key.as_deref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Source has no S3 file"})),
        )
    })?;

    // Check if file type supports OCR
    if !ocr::is_ocr_capable(s3_key) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Source file type does not support OCR: {}", s3_key)
            })),
        ));
    }

    // Download from S3
    let data = crate::routes::sources::download_from_s3_public(&config, s3_key)
        .await
        .map_err(|e| {
            error!("S3 download failed for OCR: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to download file: {}", e)})),
            )
        })?;

    // Determine target model
    let user_override = payload
        .as_ref()
        .and_then(|p| p.model.as_ref())
        .is_some();
    let target_model = payload
        .as_ref()
        .and_then(|p| p.model.clone())
        .unwrap_or_else(|| config.heimdall_model.clone());

    // Resolve LLM credentials using unified config
    let model_config = mimir_core_ai::services::db::get_model_by_id(&pool, &target_model)
        .await
        .unwrap_or(None);

    let (api_key, api_base) = crate::routes::sources::resolve_llm_credentials(
        &config,
        &model_config,
        &target_model,
    )?;

    let provider = model_config
        .as_ref()
        .map(|m| m.provider.as_str())
        .unwrap_or("unknown");

    info!(
        "OCR extract-source: id={}, s3_key={}, model={}",
        id, s3_key, target_model
    );

    // B-50e: load tenant policy. Enforcement deferred to B-50b smart-router.
    let _policy = get_ocr_policy(&pool, tenant_id).await;
    let engine_id = engine_id_for(&target_model, provider);
    let router_reason = if user_override {
        "manual_override"
    } else {
        "default_cloud"
    };

    // Update status
    let _ = sqlx::query("UPDATE data_sources SET last_sync_status = 'OCR_RUNNING' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;

    let started = Instant::now();
    let result =
        ocr::extract_text_from_image(&data, s3_key, &api_key, &api_base, &target_model).await;
    let latency_ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;

    match result {
        Ok((content, tokens_used)) => {
            // Update source with OCR result
            let mb = content.len() as f64 / 1_048_576.0;
            let update_res = sqlx::query(
                "UPDATE data_sources SET raw_markdown = ?, mb_size = ?, last_sync_status = 'COMPLETED', last_sync_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&content)
            .bind(mb)
            .bind(id)
            .execute(&pool)
            .await;
            if let Err(e) = update_res {
                error!("Failed to update source {} after OCR: {}", id, e);
            }

            let cost_usd = estimate_ocr_cost(&pool, &target_model, tokens_used).await;

            // Log usage
            let _ = crate::routes::llm_usage::insert_llm_usage_log(
                &pool,
                tenant_id,
                &target_model,
                provider,
                Some(&format!("{}chat/completions", api_base)),
                Some("ocr_extract_source"),
                0,
                0,
                tokens_used as i32,
                0,
                "success",
                None,
            )
            .await;

            // B-50e: write OCR audit row.
            let audit_id = insert_ocr_audit(
                &pool,
                OcrAuditRow {
                    tenant_id,
                    image_bytes: &data,
                    engine_used: &engine_id,
                    engine_version: Some(&target_model),
                    router_reason: Some(router_reason),
                    extracted_text: Some(&content),
                    confidence: None,
                    bbox_count: None,
                    cost_usd,
                    latency_ms: Some(latency_ms),
                    pii_redacted: false,
                    status: OcrStatus::Succeeded,
                    status_message: None,
                    image_path: Some(s3_key),
                    requested_by: None,
                },
            )
            .await;

            info!(
                "OCR completed for source {}: {} chars, {} tokens, audit={}",
                id,
                content.len(),
                tokens_used,
                audit_id
            );

            Ok(Json(json!({
                "source_id": id,
                "audit_id": audit_id,
                "content": content,
                "content_length": content.len(),
                "tokens_used": tokens_used,
                "model": target_model,
                "engine_used": engine_id,
                "latency_ms": latency_ms,
                "cost_usd": cost_usd,
                "status": "COMPLETED"
            })))
        }
        Err(e) => {
            error!("OCR failed for source {}: {}", id, e);
            let msg = format!("OCR failed: {}", e);

            // Revert status
            let pool_clone = pool.clone();
            tokio::spawn(async move {
                let _ = sqlx::query(
                    "UPDATE data_sources SET last_sync_status = 'OCR_FAILED' WHERE id = ?",
                )
                .bind(id)
                .execute(&pool_clone)
                .await;
            });

            // B-50e: audit the failure.
            let _ = insert_ocr_audit(
                &pool,
                OcrAuditRow {
                    tenant_id,
                    image_bytes: &data,
                    engine_used: &engine_id,
                    engine_version: Some(&target_model),
                    router_reason: Some(router_reason),
                    extracted_text: None,
                    confidence: None,
                    bbox_count: None,
                    cost_usd: 0.0,
                    latency_ms: Some(latency_ms),
                    pii_redacted: false,
                    status: OcrStatus::EngineFailed,
                    status_message: Some(&msg),
                    image_path: Some(s3_key),
                    requested_by: None,
                },
            )
            .await;

            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": msg})),
            ))
        }
    }
}
