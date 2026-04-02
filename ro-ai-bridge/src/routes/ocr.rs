use crate::config::Config;
use crate::routes::sources::{call_llm_api_with_logging, resolve_llm_credentials};
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
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info, warn};

pub fn ocr_routes() -> Router<DbPool> {
    Router::new()
        .route("/ocr/extract", post(ocr_extract))
        .route("/ocr/extract-source/{id}", post(ocr_extract_source))
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
    let mut model_override: Option<String> = None;

    // Parse multipart fields
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
            "model" => {
                model_override = Some(field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid model field: {}", e)})),
                    )
                })?);
            }
            _ => {
                warn!("Unknown OCR field: {}", field_name);
            }
        }
    }

    // Validate
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

    // Resolve Gemini credentials
    let model = model_override.unwrap_or_else(|| config.gemini_model.clone());
    let api_key = config.gemini_api_key.clone().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "GEMINI_API_KEY not configured. OCR requires Gemini API access."
            })),
        )
    })?;
    let api_base = config.gemini_base_url.clone();

    info!("OCR extract request: file={}, model={}", filename, model);

    // Call Gemini vision
    let (content, tokens_used) =
        ocr::extract_text_from_image(&data, &filename, &api_key, &api_base, &model)
            .await
            .map_err(|e| {
                error!("OCR extraction failed: {}", e);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": format!("OCR extraction failed: {}", e)})),
                )
            })?;

    // Log usage
    let tenant_id = extract_tenant_id(&headers);
    let _ = crate::routes::llm_usage::insert_llm_usage_log(
        &pool,
        tenant_id,
        &model,
        "gemini",
        Some(&format!("{}chat/completions", api_base)),
        Some("ocr_extract"),
        0,
        0,
        tokens_used as i32,
        0,
        "success",
        None,
    )
    .await;

    Ok(Json(json!({
        "content": content,
        "tokens_used": tokens_used,
        "model": model,
        "filename": filename,
        "content_length": content.len()
    })))
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

    // Resolve Gemini credentials
    let model = config.gemini_model.clone();
    let api_key = config.gemini_api_key.clone().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "GEMINI_API_KEY not configured"
            })),
        )
    })?;
    let api_base = config.gemini_base_url.clone();

    info!(
        "OCR extract-source: id={}, s3_key={}, model={}",
        id, s3_key, model
    );

    // Update status
    let _ = sqlx::query("UPDATE data_sources SET last_sync_status = 'OCR_RUNNING' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;

    // Call Gemini vision
    let (content, tokens_used) =
        ocr::extract_text_from_image(&data, s3_key, &api_key, &api_base, &model)
            .await
            .map_err(|e| {
                error!("OCR failed for source {}: {}", id, e);
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
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": format!("OCR failed: {}", e)})),
                )
            })?;

    // Update source with OCR result
    let mb = content.len() as f64 / 1_048_576.0;
    let _ = sqlx::query(
        "UPDATE data_sources SET raw_markdown = ?, mb_size = ?, last_sync_status = 'COMPLETED', last_sync_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(&content)
    .bind(mb)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| {
        error!("Failed to update source {} after OCR: {}", id, e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save OCR result"})))
    })?;

    // Log usage
    let _ = crate::routes::llm_usage::insert_llm_usage_log(
        &pool,
        tenant_id,
        &model,
        "gemini",
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

    info!(
        "OCR completed for source {}: {} chars, {} tokens",
        id,
        content.len(),
        tokens_used
    );

    Ok(Json(json!({
        "source_id": id,
        "content_length": content.len(),
        "tokens_used": tokens_used,
        "model": model,
        "status": "COMPLETED"
    })))
}
