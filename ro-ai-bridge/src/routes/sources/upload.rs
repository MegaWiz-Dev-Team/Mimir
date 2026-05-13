//! File upload handler and S3 helper functions.

use crate::config::Config;
use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use axum_extra::extract::Multipart;
use mimir_core_ai::models::sources::DataSource;
use mimir_core_ai::services::chunking;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::dedup;
use mimir_core_ai::services::ingress::IngressManager;
use mimir_core_ai::services::upload::{
    build_s3_key, compute_file_hash, detect_source_type, validate_extension,
};
use s3::creds::Credentials;
use s3::Bucket;
use s3::Region;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info, warn};

// ─── S3 Helpers ────────────────────────────────────────────────────────────────

/// Download a file from RustFS/S3 by its key (public variant for cross-route use).
pub async fn download_from_s3_public(config: &Config, s3_key: &str) -> anyhow::Result<Vec<u8>> {
    download_from_s3(config, s3_key).await
}

/// Download a file from RustFS/S3 by its key.
///
/// Used by sync_source to retrieve uploaded files for extraction.
pub(crate) async fn download_from_s3(config: &Config, s3_key: &str) -> anyhow::Result<Vec<u8>> {
    let region = Region::Custom {
        region: config.s3_region.clone(),
        endpoint: config.s3_endpoint.clone(),
    };

    let credentials = Credentials::new(
        Some(&config.s3_access_key),
        Some(&config.s3_secret_key),
        None,
        None,
        None,
    )
    .map_err(|e| anyhow::anyhow!("S3 credentials error: {}", e))?;

    let bucket = Bucket::new(&config.s3_bucket, region, credentials)
        .map_err(|e| anyhow::anyhow!("S3 bucket error: {}", e))?
        .with_path_style();

    let response = bucket
        .get_object(s3_key)
        .await
        .map_err(|e| anyhow::anyhow!("S3 get_object failed for '{}': {}", s3_key, e))?;

    if response.status_code() != 200 {
        return Err(anyhow::anyhow!(
            "S3 returned status {} for key '{}'",
            response.status_code(),
            s3_key
        ));
    }

    Ok(response.to_vec())
}

/// Helper to create an S3 bucket client from centralized Config.
fn create_s3_bucket(config: &Config) -> Result<Box<Bucket>, (StatusCode, Json<Value>)> {
    let region = Region::Custom {
        region: config.s3_region.clone(),
        endpoint: config.s3_endpoint.clone(),
    };

    let credentials = Credentials::new(
        Some(&config.s3_access_key),
        Some(&config.s3_secret_key),
        None,
        None,
        None,
    )
    .map_err(|e| {
        error!("Failed to create S3 credentials: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "S3 configuration error"})),
        )
    })?;

    let bucket = Bucket::new(&config.s3_bucket, region, credentials)
        .map_err(|e| {
            error!("Failed to create S3 bucket client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "S3 bucket configuration error"})),
            )
        })?
        .with_path_style();

    Ok(bucket)
}

// ─── Upload Handler ────────────────────────────────────────────────────────────

/// POST /api/v1/sources/upload
///
/// Accepts multipart/form-data with fields:
/// - `file`: Binary file data
/// - `name`: String (source name)
/// - `source_type`: "document" | "tabular"
/// - `storage_mode`: "markdown" | "sql" (optional, defaults to "markdown")
/// - `folder_path`: String (optional, for folder upload)
pub(crate) async fn upload_file(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut source_name: Option<String> = None;
    let mut user_source_type: Option<String> = None;
    let mut storage_mode = "markdown".to_string();
    let mut folder_path = String::new();
    let mut patient_id: Option<String> = None;

    // Parse multipart fields
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to read multipart field: {}", e);
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
                    error!("Failed to read file bytes: {}", e);
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed to read file: {}", e)})),
                    )
                })?;
                file_data = Some(bytes.to_vec());
            }
            "name" => {
                source_name = Some(field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid name field: {}", e)})),
                    )
                })?);
            }
            "source_type" => {
                user_source_type = Some(field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid source_type field: {}", e)})),
                    )
                })?);
            }
            "storage_mode" => {
                storage_mode = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid storage_mode field: {}", e)})),
                    )
                })?;
            }
            "folder_path" => {
                folder_path = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid folder_path field: {}", e)})),
                    )
                })?;
            }
            "patient_id" => {
                patient_id = Some(field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Invalid patient_id field: {}", e)})),
                    )
                })?);
            }
            _ => {
                warn!("Unknown multipart field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let data = file_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing 'file' field"})),
        )
    })?;
    let original_filename = file_name.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "File must have a filename"})),
        )
    })?;
    let name = source_name.unwrap_or_else(|| original_filename.clone());

    // Auto-detect source_type from file extension (user-provided value is optional override)
    let source_type =
        user_source_type.unwrap_or_else(|| detect_source_type(&original_filename).to_string());

    // Validate file extension
    validate_extension(&original_filename).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    // Validate file size from pipeline_settings (default 50MB)
    let mut max_mb = 50u64;
    let cfg_json: Option<(Option<Value>,)> = sqlx::query_as(
        "SELECT pipeline_settings FROM tenant_configs WHERE tenant_id = ?"
    )
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((Some(settings),)) = cfg_json {
        if let Some(m) = settings.get("max_upload_size_mb").and_then(|v| v.as_u64()) {
            max_mb = m;
        }
    }

    let max_size_bytes = max_mb * 1024 * 1024;
    if data.len() as u64 > max_size_bytes {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({
                "error": format!("File size ({:.1} MB) exceeds the maximum allowed limit of {} MB specified in Pipeline Settings.", data.len() as f64 / 1048576.0, max_mb),
                "max_size_bytes": max_size_bytes,
                "actual_size_bytes": data.len()
            })),
        ));
    }

    // Compute SHA-256 hash for duplicate detection
    let file_hash = compute_file_hash(&data);

    // Check for duplicate file by hash
    let existing = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE tenant_id = ? AND file_hash = ? LIMIT 1",
    )
    .bind(tenant_id)
    .bind(&file_hash)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        error!("DB error checking duplicate: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"})),
        )
    })?;

    if let Some(dup) = existing {
        info!(
            "Duplicate file detected: hash={}, existing_source_id={}",
            file_hash, dup.id
        );
        return Ok((
            StatusCode::OK,
            Json(json!({
                "message": "Duplicate file detected — skipped upload",
                "existing_source_id": dup.id,
                "file_hash": file_hash
            })),
        ));
    }

    // Create DB record first to get source_id
    let mb_size = data.len() as f64 / 1_048_576.0;
    let config_json = json!({
        "original_filename": original_filename,
        "folder_path": folder_path,
        "storage_mode": storage_mode
    });

    let insert_result = sqlx::query(
        r#"INSERT INTO data_sources (tenant_id, patient_id, name, source_type, config_json, last_sync_status, mb_size, storage_mode, file_hash)
        VALUES (?, ?, ?, ?, ?, 'PENDING', ?, ?, ?)"#
    )
    .bind(tenant_id)
    .bind(&patient_id)
    .bind(&name)
    .bind(&source_type)
    .bind(&config_json)
    .bind(mb_size)
    .bind(&storage_mode)
    .bind(&file_hash)
    .execute(&pool)
    .await
    .map_err(|e| {
        error!("Failed to insert data_source record: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create source record"})))
    })?;

    let source_id = insert_result.last_insert_id();

    // Build S3 key and upload to RustFS
    let s3_key = build_s3_key(
        tenant_id,
        &source_id.to_string(),
        &folder_path,
        &original_filename,
        patient_id.as_deref(),
    );

    // Attempt S3 upload
    match create_s3_bucket(&config) {
        Ok(bucket) => {
            match bucket.put_object(&s3_key, &data).await {
                Ok(response) => {
                    info!(
                        "Uploaded to S3: key={}, status={}",
                        s3_key,
                        response.status_code()
                    );
                    // Update record with s3_key
                    let _ = sqlx::query("UPDATE data_sources SET s3_key = ? WHERE id = ?")
                        .bind(&s3_key)
                        .bind(source_id as i64)
                        .execute(&pool)
                        .await
                        .map_err(|e| {
                            error!("Failed to update s3_key for source {}: {}", source_id, e)
                        });
                }
                Err(e) => {
                    warn!("S3 upload failed (will retry later): {}", e);
                    let _ = sqlx::query(
                        "UPDATE data_sources SET last_sync_status = 'PENDING_FETCH' WHERE id = ?",
                    )
                    .bind(source_id as i64)
                    .execute(&pool)
                    .await;
                }
            }
        }
        Err(_) => {
            warn!("S3 client not configured — file stored in DB record only");
        }
    }

    info!(
        "Upload complete: source_id={}, file={}, hash={}",
        source_id, original_filename, file_hash
    );

    // ─── Post-upload: trigger extraction in background ───────────────────────
    {
        let pool_bg = pool.clone();
        let src_type = source_type.clone();
        let src_name = name.clone();
        let s3_key_bg = s3_key.clone();
        let st_mode = storage_mode.clone();
        let tenant = tenant_id.to_string();
        let src_id = source_id as i64;

        tokio::spawn(async move {
            info!("Auto-extraction triggered for source_id={}", src_id);

            // Build a lightweight DataSource for extraction
            let ds = DataSource {
                id: src_id,
                tenant_id: tenant,
                name: src_name.clone(),
                source_type: src_type,
                config_json: json!({}),
                schedule: None,
                last_sync_status: Some("RUNNING".to_string()),
                last_sync_at: None,
                created_at: None,
                updated_at: None,
                raw_markdown: None,
                mb_size: None,
                total_chunks: None,
                pageindex_tree: None,
                storage_mode: Some(st_mode),
                s3_key: Some(s3_key_bg.clone()),
                file_hash: None,
                refresh_interval_hours: None,
                last_refreshed_at: None,
                next_refresh_at: None,
                refresh_status: None,
            };

            // Update status to RUNNING
            let _ =
                sqlx::query("UPDATE data_sources SET last_sync_status = 'RUNNING' WHERE id = ?")
                    .bind(src_id)
                    .execute(&pool_bg)
                    .await;

            let mut extraction_result = IngressManager::process_source_with_data(&ds, &data);

            // ─── OCR Fallback: if PDF extraction returned empty (image-only PDF),
            //     use Gemini Vision to OCR the document ───────────────────────
            if let Err(ref e) = extraction_result {
                let err_msg = e.to_string();
                if err_msg.contains("image-only") || err_msg.contains("empty text") {
                    let ext = s3_key_bg.rsplit('.').next().unwrap_or("").to_lowercase();
                    if ext == "pdf" {
                        info!(
                            "PDF text extraction empty for {} ({}), falling back to Gemini Vision OCR",
                            src_name, src_id
                        );

                        let gemini_api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
                        let gemini_base = std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| {
                            "https://generativelanguage.googleapis.com/v1beta/openai/".to_string()
                        });

                        if !gemini_api_key.is_empty() {
                            match mimir_core_ai::services::ocr::extract_text_from_image(
                                &data,
                                &s3_key_bg,
                                &gemini_api_key,
                                &gemini_base,
                                "gemini-3-flash-preview",
                            )
                            .await
                            {
                                Ok((ocr_text, tokens)) => {
                                    info!(
                                        "Gemini OCR fallback succeeded for {} ({}): {} chars, {} tokens",
                                        src_name, src_id, ocr_text.len(), tokens
                                    );
                                    extraction_result = Ok(ocr_text);
                                }
                                Err(ocr_err) => {
                                    error!(
                                        "Gemini OCR fallback also failed for {} ({}): {}",
                                        src_name, src_id, ocr_err
                                    );
                                    // Keep original error
                                }
                            }
                        } else {
                            warn!(
                                "GEMINI_API_KEY not set — cannot OCR fallback for image-only PDF (source_id={})",
                                src_id
                            );
                        }
                    }
                }
            }

            match extraction_result {
                Ok(raw_text) => {
                    let mb = raw_text.len() as f64 / 1_048_576.0;

                    // ─── Chunk the extracted text ────────────────────
                    let strategy = chunking::auto_recommend(&raw_text);
                    let chunk_results = chunking::chunk(&raw_text, &strategy).unwrap_or_default();
                    let chunks = chunk_results.len() as i32;

                    info!(
                        "Auto-extraction completed for {} ({}): {} bytes, {} chunks",
                        src_name,
                        src_id,
                        raw_text.len(),
                        chunks
                    );

                    // Store chunks in DB (with dedup)
                    let mut dedup_tracker = dedup::DedupTracker::new();
                    for cr in &chunk_results {
                        let content_hash = dedup::fingerprint(&cr.content);

                        // Check DB for existing fingerprint
                        let existing: Option<(i64,)> = sqlx::query_as(
                            "SELECT source_id FROM content_fingerprints WHERE content_hash = ? LIMIT 1"
                        )
                        .bind(&content_hash)
                        .fetch_optional(&pool_bg)
                        .await
                        .unwrap_or(None);

                        if let Some((existing_source_id,)) = existing {
                            dedup_tracker.record_duplicate(
                                cr.chunk_index,
                                &content_hash,
                                existing_source_id,
                            );
                            continue;
                        }

                        if let Some(existing_src) = dedup_tracker.is_seen(&content_hash) {
                            dedup_tracker.record_duplicate(
                                cr.chunk_index,
                                &content_hash,
                                existing_src,
                            );
                            continue;
                        }

                        let meta_str = serde_json::to_string(&cr.metadata).unwrap_or_default();
                        let token_ct = cr.token_count as i32;
                        let idx = cr.chunk_index as i32;
                        let chunk_insert = sqlx::query(
                            "INSERT INTO chunks (source_id, chunk_index, content, token_count, metadata_json) VALUES (?, ?, ?, ?, ?)"
                        )
                        .bind(src_id)
                        .bind(idx)
                        .bind(&cr.content)
                        .bind(token_ct)
                        .bind(&meta_str)
                        .execute(&pool_bg)
                        .await;

                        if let Ok(result) = chunk_insert {
                            let chunk_id = result.last_insert_id() as i64;
                            let _ = sqlx::query(
                                "INSERT INTO content_fingerprints (content_hash, source_id, chunk_id) VALUES (?, ?, ?)"
                            )
                            .bind(&content_hash)
                            .bind(src_id)
                            .bind(chunk_id)
                            .execute(&pool_bg)
                            .await;
                        }

                        dedup_tracker.track_hash(&content_hash, src_id);
                        dedup_tracker.record_unique();
                    }

                    if dedup_tracker.report.duplicate_chunks > 0 {
                        info!(
                            "Dedup report for source {}: {} unique, {} duplicates skipped",
                            src_id,
                            dedup_tracker.report.unique_chunks,
                            dedup_tracker.report.duplicate_chunks
                        );
                    }
                    let chunks = dedup_tracker.report.unique_chunks as i32;

                    let _ = sqlx::query(
                        "UPDATE data_sources SET last_sync_status = 'COMPLETED', raw_markdown = ?, mb_size = ?, total_chunks = ?, last_sync_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(&raw_text)
                    .bind(mb)
                    .bind(chunks)
                    .bind(src_id)
                    .execute(&pool_bg)
                    .await
                    .map_err(|e| error!("Failed to update source {} to COMPLETED: {}", src_id, e));
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    error!(
                        "Auto-extraction failed for {} ({}): {}",
                        src_name, src_id, err_msg
                    );
                    let _ = sqlx::query(
                        "UPDATE data_sources SET last_sync_status = 'FAILED', raw_markdown = ? WHERE id = ?"
                    )
                    .bind(&err_msg)
                    .bind(src_id)
                    .execute(&pool_bg)
                    .await
                    .map_err(|e| error!("Failed to update source {} to FAILED: {}", src_id, e));
                }
            }
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "File uploaded successfully",
            "source_id": source_id,
            "name": name,
            "source_type": source_type,
            "storage_mode": storage_mode,
            "s3_key": s3_key,
            "file_hash": file_hash,
            "mb_size": mb_size
        })),
    ))
}

// ─── Download Handler ──────────────────────────────────────────────────────

/// GET /api/v1/sources/{id}/file
///
/// Downloads a file from S3/RustFS and streams it to the client.
/// Query param: `tenant_id` (optional, extracted from header if not provided)
pub(crate) async fn download_file(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    axum::extract::Path(source_id): axum::extract::Path<i64>,
) -> Result<impl axum::response::IntoResponse, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Query data_sources to get s3_key and metadata
    let source: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT s3_key, name FROM data_sources WHERE id = ? AND tenant_id = ?",
    )
    .bind(source_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        error!("DB error fetching source {}: {}", source_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"})),
        )
    })?;

    let (s3_key, name) = source.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Source {} not found", source_id)})),
        )
    })?;

    // Download from S3
    let file_bytes = download_from_s3(&config, &s3_key)
        .await
        .map_err(|e| {
            error!("Failed to download file {} from S3: {}", source_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to download file"})),
            )
        })?;

    // Determine Content-Type based on file extension
    let content_type = match s3_key.rsplit('.').next() {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        _ => "application/octet-stream",
    };

    let filename = name.unwrap_or_else(|| "file".to_string());
    let disposition = format!("attachment; filename=\"{}\"", filename);

    use axum::http::header::{HeaderMap as RespHeaders, HeaderValue};

    let mut headers = RespHeaders::new();
    headers.insert("content-type", HeaderValue::from_static(content_type));
    headers.insert(
        "content-disposition",
        HeaderValue::from_str(&disposition).unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );

    Ok((headers, file_bytes))
}
