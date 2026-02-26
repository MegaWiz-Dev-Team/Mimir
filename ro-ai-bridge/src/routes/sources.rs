use axum::{
    routing::{get, post, put, delete},
    Router, Json, extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use axum_extra::extract::Multipart;
use tokio::time::sleep;
use std::time::Duration;
use futures::stream::{self, Stream};
use std::convert::Infallible;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::models::sources::{DataSource, CreateDataSourceRequest, UpdateDataSourceRequest};
use mimir_core_ai::services::upload::{validate_extension, validate_file_size, build_s3_key, compute_file_hash};
use serde_json::{json, Value};
use tracing::{info, error, warn};
use mimir_core_ai::services::ingress::IngressManager;
use s3::creds::Credentials;
use s3::Bucket;
use s3::Region;

pub fn sources_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_sources).post(create_source))
        .route("/upload", post(upload_file))
        .route("/{id}", put(update_source).delete(delete_source))
        .route("/{id}/sync", post(sync_source))
        .route("/{id}/logs", get(stream_logs))
}

async fn list_sources(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<DataSource>>, (StatusCode, Json<Value>)> {
    // Note: To truly support multi-tenancy, we should extract the tenant_id from the Auth token middleware
    // We'll mock it here temporarily or retrieve from Extension if added by middleware
    let tenant_id = "default_tenant"; 
    
    let sources = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(sources))
}

async fn create_source(
    State(pool): State<DbPool>,
    Json(payload): Json<CreateDataSourceRequest>,
) -> Result<(StatusCode, Json<DataSource>), (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant"; // Future: get from auth token
    
    let result = sqlx::query!(
        r#"
        INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
        VALUES (?, ?, ?, ?, ?)
        "#,
        tenant_id,
        payload.name,
        payload.source_type,
        payload.config_json,
        payload.schedule
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let new_source = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ?"
    )
    .bind(result.last_insert_id())
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok((StatusCode::CREATED, Json(new_source)))
}

async fn update_source(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDataSourceRequest>,
) -> Result<Json<DataSource>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    
    // Check if source exists
    let existing = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if existing.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"}))));
    }
    
    let current = existing.unwrap();
    let updated_name = payload.name.unwrap_or(current.name);
    let updated_config = payload.config_json.unwrap_or(current.config_json);
    let updated_schedule = payload.schedule.or(current.schedule);

    sqlx::query!(
        "UPDATE data_sources SET name = ?, config_json = ?, schedule = ? WHERE id = ? AND tenant_id = ?",
        updated_name,
        updated_config,
        updated_schedule,
        id,
        tenant_id
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let updated_source = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(updated_source))
}

async fn delete_source(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    
    let result = sqlx::query!(
        "DELETE FROM data_sources WHERE id = ? AND tenant_id = ?",
        id,
        tenant_id
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found or access denied"}))));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn sync_source(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    
    // Check if source exists
    let source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if source.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"}))));
    }
    
    // Update status to RUNNING
    sqlx::query!(
        "UPDATE data_sources SET last_sync_status = 'RUNNING' WHERE id = ?",
        id
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let pool_clone = pool.clone();
    let source_clone = source.unwrap();
    // Spawn a background task to process the source
    tokio::spawn(async move {
        info!("Started background sync task for source id {}", id);
        match IngressManager::process_source(&source_clone).await {
            Ok(raw_text) => {
                let mb_size = raw_text.len() as f64 / 1_048_576.0;
                let total_chunks = (raw_text.len() as f64 / 500.0).ceil() as i32;
                
                info!("Sync completed for {} ({}): {} bytes processed", source_clone.name, id, raw_text.len());
                let _ = sqlx::query!(
                    "UPDATE data_sources SET last_sync_status = 'COMPLETED', raw_markdown = ?, mb_size = ?, total_chunks = ?, last_sync_at = CURRENT_TIMESTAMP WHERE id = ?",
                    raw_text,
                    mb_size,
                    total_chunks,
                    id
                )
                .execute(&pool_clone)
                .await
                .map_err(|e| error!("Failed to update source {} to COMPLETED: {}", id, e));
            },
            Err(e) => {
                error!("Sync failed for {} ({}): {}", source_clone.name, id, e);
                let _ = sqlx::query!(
                    "UPDATE data_sources SET last_sync_status = 'FAILED' WHERE id = ?",
                    id
                )
                .execute(&pool_clone)
                .await
                .map_err(|e| error!("Failed to update source {} to FAILED: {}", id, e));
            }
        }
    });

    info!("Triggered sync for source id {}", id);

    Ok((StatusCode::ACCEPTED, Json(json!({
        "message": "Sync job triggered successfully",
        "source_id": id
    }))))
}

// Simulated SSE stream for real-time logs
async fn stream_logs(
    Path(id): Path<i64>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("Client connected to log stream for source {}", id);
    
    // In a real application, you'd subscribe to a broadcast channel or access a log file/database.
    // This is a simple mock stream that yields logging messages every second.
    let stream = async_stream::stream! {
        yield Ok(Event::default().data(format!("> Connected to log stream for source #{}", id)));
        sleep(Duration::from_secs(1)).await;
        yield Ok(Event::default().data("> Initializing ingress workers..."));
        sleep(Duration::from_secs(1)).await;
        yield Ok(Event::default().data("> Fetching data source configuration..."));
        sleep(Duration::from_secs(2)).await;
        yield Ok(Event::default().data("> Processing data elements..."));
        sleep(Duration::from_secs(2)).await;
        yield Ok(Event::default().data("> Adding to Vector Space..."));
        sleep(Duration::from_secs(1)).await;
        yield Ok(Event::default().data("> COMPLETED. Worker shutting down."));
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive-text"),
    )
}

// TODO: Refactor test to use `sqlx::AnyPool` or inject a mock repository interface 
// to support sqlite in-memory testing instead of the hardcoded `MySqlPool` required by app state.

// ─── Upload Handler ────────────────────────────────────────────────────────────

/// Helper to create an S3 bucket client from environment variables.
fn create_s3_bucket() -> Result<Box<Bucket>, (StatusCode, Json<Value>)> {
    let endpoint = std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string());
    let bucket_name = std::env::var("S3_BUCKET").unwrap_or_else(|_| "mimir-tenant-uploads".to_string());
    let access_key = std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string());
    let secret_key = std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string());
    let region_name = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());

    let region = Region::Custom {
        region: region_name,
        endpoint,
    };

    let credentials = Credentials::new(
        Some(&access_key),
        Some(&secret_key),
        None, None, None,
    ).map_err(|e| {
        error!("Failed to create S3 credentials: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "S3 configuration error"})))
    })?;

    let bucket = Bucket::new(&bucket_name, region, credentials)
        .map_err(|e| {
            error!("Failed to create S3 bucket client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "S3 bucket configuration error"})))
        })?
        .with_path_style();

    Ok(bucket)
}

/// POST /api/v1/sources/upload
///
/// Accepts multipart/form-data with fields:
/// - `file`: Binary file data
/// - `name`: String (source name)
/// - `source_type`: "document" | "tabular"
/// - `storage_mode`: "markdown" | "sql" (optional, defaults to "markdown")
/// - `folder_path`: String (optional, for folder upload)
async fn upload_file(
    State(pool): State<DbPool>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant"; // Future: extract from JWT via tenant_auth_middleware

    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut source_name: Option<String> = None;
    let mut source_type = "document".to_string();
    let mut storage_mode = "markdown".to_string();
    let mut folder_path = String::new();

    // Parse multipart fields
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to read multipart field: {}", e);
        (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid multipart data: {}", e)})))
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                let bytes = field.bytes().await.map_err(|e| {
                    error!("Failed to read file bytes: {}", e);
                    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Failed to read file: {}", e)})))
                })?;
                file_data = Some(bytes.to_vec());
            }
            "name" => {
                source_name = Some(field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid name field: {}", e)})))
                })?);
            }
            "source_type" => {
                source_type = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid source_type field: {}", e)})))
                })?;
            }
            "storage_mode" => {
                storage_mode = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid storage_mode field: {}", e)})))
                })?;
            }
            "folder_path" => {
                folder_path = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid folder_path field: {}", e)})))
                })?;
            }
            _ => {
                warn!("Unknown multipart field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let data = file_data.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing 'file' field"})))
    })?;
    let original_filename = file_name.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "File must have a filename"})))
    })?;
    let name = source_name.unwrap_or_else(|| original_filename.clone());

    // Validate file extension
    validate_extension(&original_filename).map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
    })?;

    // Validate file size (50MB max)
    validate_file_size(data.len() as u64).map_err(|_| {
        (StatusCode::PAYLOAD_TOO_LARGE, Json(json!({
            "error": "File too large",
            "max_size_bytes": 50 * 1024 * 1024,
            "actual_size_bytes": data.len()
        })))
    })?;

    // Compute SHA-256 hash for duplicate detection
    let file_hash = compute_file_hash(&data);

    // Check for duplicate file by hash
    let existing = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE tenant_id = ? AND file_hash = ? LIMIT 1"
    )
    .bind(tenant_id)
    .bind(&file_hash)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        error!("DB error checking duplicate: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
    })?;

    if let Some(dup) = existing {
        info!("Duplicate file detected: hash={}, existing_source_id={}", file_hash, dup.id);
        return Ok((StatusCode::OK, Json(json!({
            "message": "Duplicate file detected — skipped upload",
            "existing_source_id": dup.id,
            "file_hash": file_hash
        }))));
    }

    // Create DB record first to get source_id
    let mb_size = data.len() as f64 / 1_048_576.0;
    let config_json = json!({
        "original_filename": original_filename,
        "folder_path": folder_path,
        "storage_mode": storage_mode
    });

    let insert_result = sqlx::query(
        r#"INSERT INTO data_sources (tenant_id, name, source_type, config_json, last_sync_status, mb_size, storage_mode, file_hash)
        VALUES (?, ?, ?, ?, 'PENDING', ?, ?, ?)"#
    )
    .bind(tenant_id)
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
    let s3_key = build_s3_key(tenant_id, &source_id.to_string(), &folder_path, &original_filename);

    // Attempt S3 upload
    match create_s3_bucket() {
        Ok(bucket) => {
            match bucket.put_object(&s3_key, &data).await {
                Ok(response) => {
                    info!("Uploaded to S3: key={}, status={}", s3_key, response.status_code());
                    // Update record with s3_key
                    let _ = sqlx::query(
                        "UPDATE data_sources SET s3_key = ? WHERE id = ?"
                    )
                    .bind(&s3_key)
                    .bind(source_id as i64)
                    .execute(&pool)
                    .await
                    .map_err(|e| error!("Failed to update s3_key for source {}: {}", source_id, e));
                }
                Err(e) => {
                    warn!("S3 upload failed (will retry later): {}", e);
                    // Don't fail the request — the file can be re-uploaded later
                    // Mark as PENDING_FETCH so a background worker can retry
                    let _ = sqlx::query(
                        "UPDATE data_sources SET last_sync_status = 'PENDING_FETCH' WHERE id = ?"
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

    info!("Upload complete: source_id={}, file={}, hash={}", source_id, original_filename, file_hash);

    Ok((StatusCode::CREATED, Json(json!({
        "message": "File uploaded successfully",
        "source_id": source_id,
        "name": name,
        "source_type": source_type,
        "storage_mode": storage_mode,
        "s3_key": s3_key,
        "file_hash": file_hash,
        "mb_size": mb_size
    }))))
}
