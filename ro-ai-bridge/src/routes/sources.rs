use axum::{
    routing::{get, post, put, delete},
    Router, Json, Extension, extract::{Path, State, Query},
    http::{StatusCode, HeaderMap},
    response::sse::{Event, Sse},
};
use std::sync::Arc;
use crate::config::Config;
use axum_extra::extract::Multipart;
use tokio::time::sleep;
use std::time::Duration;
use futures::stream::{self, Stream};
use std::convert::Infallible;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::models::sources::{DataSource, CreateDataSourceRequest, UpdateDataSourceRequest};
use mimir_core_ai::services::upload::{validate_extension, validate_file_size, build_s3_key, compute_file_hash, detect_source_type};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use mimir_core_ai::services::ingress::IngressManager;
use mimir_core_ai::services::chunking;
use mimir_core_ai::services::link_discovery;
use mimir_core_ai::services::dedup;
use mimir_core_ai::services::db;
use s3::creds::Credentials;
use s3::Bucket;
use s3::Region;
use crate::routes::tenant::extract_tenant_id;

pub fn sources_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_sources).post(create_source))
        .route("/upload", post(upload_file))
        .route("/preview", get(preview_url))
        .route("/{id}", put(update_source).delete(delete_source))
        .route("/{id}/sync", post(sync_source))
        .route("/{id}/extract-ai", post(extract_with_ai))
        .route("/{id}/logs", get(stream_logs))
        .route("/{id}/discover-hierarchy", post(discover_hierarchy))
        .route("/{id}/import-pages", post(import_pages))
}

async fn list_sources(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<DataSource>>, (StatusCode, Json<Value>)> {
    // Note: To truly support multi-tenancy, we should extract the tenant_id from the Auth token middleware
    // We'll mock it here temporarily or retrieve from Extension if added by middleware
    let tenant_id = extract_tenant_id(&headers);
    
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
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<CreateDataSourceRequest>,
) -> Result<(StatusCode, Json<DataSource>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    
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
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDataSourceRequest>,
) -> Result<Json<DataSource>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    
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
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    
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
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    
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
    let config_clone = config.clone();
    let source_clone = source.unwrap();
    // Spawn a background task to process the source
    tokio::spawn(async move {
        info!("Started background sync task for source id {}", id);

        let result: Result<String, anyhow::Error> = match source_clone.source_type.as_str() {
            // File-based sources: download from S3, then extract
            "file" | "document" | "tabular" => {
                match &source_clone.s3_key {
                    Some(s3_key) if !s3_key.is_empty() => {
                        info!("Downloading from S3: key={}", s3_key);
                        // Download file from RustFS
                        match download_from_s3(&config_clone, s3_key).await {
                            Ok(data) => {
                                info!("Downloaded {} bytes from S3, running extraction", data.len());
                                IngressManager::process_source_with_data(&source_clone, &data)
                            },
                            Err(e) => Err(anyhow::anyhow!("S3 download failed: {}", e)),
                        }
                    },
                    _ => Err(anyhow::anyhow!(
                        "No S3 key found for source '{}' — file may not have been uploaded",
                        source_clone.name
                    )),
                }
            },
            // Network sources: fetch + extract
            _ => IngressManager::process_source(&source_clone).await,
        };

        match result {
            Ok(raw_text) => {
                let mb_size = raw_text.len() as f64 / 1_048_576.0;

                // ─── Chunk the extracted text ────────────────────────
                let strategy = chunking::auto_recommend(&raw_text);
                let chunk_results = chunking::chunk(&raw_text, &strategy).unwrap_or_default();
                let total_chunks = chunk_results.len() as i32;

                info!("Sync completed for {} ({}): {} bytes, {} chunks", source_clone.name, id, raw_text.len(), total_chunks);

                // Store chunks in DB (with dedup)
                let mut dedup_tracker = dedup::DedupTracker::new();
                for cr in &chunk_results {
                    let content_hash = dedup::fingerprint(&cr.content);

                    // Check for existing fingerprint in DB
                    let existing: Option<(i64,)> = sqlx::query_as(
                        "SELECT source_id FROM content_fingerprints WHERE content_hash = ? LIMIT 1"
                    )
                    .bind(&content_hash)
                    .fetch_optional(&pool_clone)
                    .await
                    .unwrap_or(None);

                    if let Some((existing_source_id,)) = existing {
                        dedup_tracker.record_duplicate(cr.chunk_index, &content_hash, existing_source_id);
                        continue; // Skip duplicate chunk
                    }

                    // Also check within this run
                    if let Some(existing_src) = dedup_tracker.is_seen(&content_hash) {
                        dedup_tracker.record_duplicate(cr.chunk_index, &content_hash, existing_src);
                        continue;
                    }

                    // Unique chunk — insert
                    let meta_str = serde_json::to_string(&cr.metadata).unwrap_or_default();
                    let token_ct = cr.token_count as i32;
                    let idx = cr.chunk_index as i32;
                    let chunk_insert = sqlx::query(
                        "INSERT INTO chunks (source_id, chunk_index, content, token_count, metadata_json) VALUES (?, ?, ?, ?, ?)"
                    )
                    .bind(id)
                    .bind(idx)
                    .bind(&cr.content)
                    .bind(token_ct)
                    .bind(&meta_str)
                    .execute(&pool_clone)
                    .await;

                    // Record fingerprint
                    if let Ok(result) = chunk_insert {
                        let chunk_id = result.last_insert_id() as i64;
                        let _ = sqlx::query(
                            "INSERT INTO content_fingerprints (content_hash, source_id, chunk_id) VALUES (?, ?, ?)"
                        )
                        .bind(&content_hash)
                        .bind(id)
                        .bind(chunk_id)
                        .execute(&pool_clone)
                        .await;
                    }

                    dedup_tracker.track_hash(&content_hash, id);
                    dedup_tracker.record_unique();
                }

                if dedup_tracker.report.duplicate_chunks > 0 {
                    info!("Dedup report for source {}: {} unique, {} duplicates skipped",
                        id, dedup_tracker.report.unique_chunks, dedup_tracker.report.duplicate_chunks);
                }
                let total_chunks = dedup_tracker.report.unique_chunks as i32;

                // ─── Link Discovery for web sources ─────────────────
                if source_clone.source_type == "web" {
                    let content_hash = link_discovery::compute_content_hash(&raw_text);
                    let source_url = source_clone.config_json.get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Record the main page
                    let _ = sqlx::query(
                        "INSERT INTO crawled_pages (source_id, url, status, content_hash, last_crawled_at) VALUES (?, ?, 'crawled', ?, CURRENT_TIMESTAMP) ON DUPLICATE KEY UPDATE status = 'crawled', content_hash = VALUES(content_hash), last_crawled_at = CURRENT_TIMESTAMP"
                    )
                    .bind(id)
                    .bind(source_url)
                    .bind(&content_hash)
                    .execute(&pool_clone)
                    .await
                    .map_err(|e| error!("Failed to insert crawled_page for source {}: {}", id, e));

                    // Discover linked pages (same domain, max 50)
                    let discovered = link_discovery::discover_links(&raw_text, source_url, 50);
                    info!("Discovered {} links for source {}", discovered.len(), id);

                    for link in &discovered {
                        let _ = sqlx::query(
                            "INSERT IGNORE INTO crawled_pages (source_id, url, status) VALUES (?, ?, 'pending')"
                        )
                        .bind(id)
                        .bind(&link.url)
                        .execute(&pool_clone)
                        .await
                        .map_err(|e| error!("Failed to insert discovered link for source {}: {}", id, e));
                    }
                }
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
                let error_msg = format!("{}", e);
                error!("Sync failed for {} ({}): {}", source_clone.name, id, error_msg);
                let _ = sqlx::query!(
                    "UPDATE data_sources SET last_sync_status = 'FAILED', raw_markdown = ? WHERE id = ?",
                    error_msg,
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

// ─── Web Hierarchy Discovery ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DiscoverHierarchyRequest {
    max_depth: Option<u32>,
    max_pages: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
struct HierarchyNode {
    url: String,
    title: Option<String>,
    depth: u32,
    status: String,
    children: Vec<HierarchyNode>,
}

/// POST /api/v1/sources/:id/discover-hierarchy
///
/// Crawl root URL and discover linked pages via BFS.
/// Returns a flat list of pages with status badges (new/updated/unchanged/duplicate).
async fn discover_hierarchy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<DiscoverHierarchyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let max_depth = payload.max_depth.unwrap_or(3).min(5);
    // Read configurable max from tenant config (Issue #164)
    let tenant_max: i32 = sqlx::query_scalar(
        "SELECT max_crawl_pages FROM tenant_configs WHERE tenant_id = ?"
    )
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None)
    .unwrap_or(100);
    let max_pages = payload.max_pages.unwrap_or(tenant_max as u32).min(500);

    let source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let source = source.ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"})))
    })?;

    let root_url = source.config_json.get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": "Source has no URL configured"})))
        })?
        .to_string();

    info!("Starting hierarchy discovery for source {} from {}", id, root_url);

    // BFS crawl
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(String, u32)> = std::collections::VecDeque::new();
    let mut all_pages: Vec<(String, Option<String>, u32, String)> = Vec::new();

    queue.push_back((root_url.clone(), 0));
    visited.insert(root_url.clone());

    while let Some((url, depth)) = queue.pop_front() {
        if all_pages.len() >= max_pages as usize {
            break;
        }

        let html = match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                resp.text().await.unwrap_or_default()
            }
            _ => {
                all_pages.push((url.clone(), None, depth, String::new()));
                continue;
            }
        };

        let doc = scraper::Html::parse_document(&html);
        let title_sel = scraper::Selector::parse("title").unwrap();
        let title = doc.select(&title_sel).next()
            .map(|el| el.text().collect::<String>().trim().to_string());

        let content_hash = link_discovery::compute_content_hash(&html);
        all_pages.push((url.clone(), title, depth, content_hash));

        if depth < max_depth {
            let links = link_discovery::discover_links(&html, &url, 200);
            for link in links {
                if !visited.contains(&link.url) && all_pages.len() + queue.len() < max_pages as usize {
                    visited.insert(link.url.clone());
                    queue.push_back((link.url, depth + 1));
                }
            }
        }
    }

    // Determine status for each page
    let mut nodes: Vec<HierarchyNode> = Vec::new();
    for (url, title, depth, content_hash) in &all_pages {
        let status = if content_hash.is_empty() {
            "error".to_string()
        } else {
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT content_hash FROM crawled_pages WHERE source_id = ? AND url = ?"
            )
            .bind(id)
            .bind(url)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

            match existing {
                Some((old_hash,)) if old_hash == *content_hash => "unchanged".to_string(),
                Some(_) => "updated".to_string(),
                None => {
                    let dup: Option<(i64,)> = sqlx::query_as(
                        "SELECT source_id FROM content_fingerprints WHERE content_hash = ? LIMIT 1"
                    )
                    .bind(content_hash)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or(None);

                    if dup.is_some() { "duplicate".to_string() } else { "new".to_string() }
                }
            }
        };

        nodes.push(HierarchyNode {
            url: url.clone(),
            title: title.clone(),
            depth: *depth,
            status,
            children: vec![],
        });
    }

    info!("Discovered {} pages for source {}", nodes.len(), id);

    Ok(Json(json!({
        "source_id": id,
        "root_url": root_url,
        "total_pages": nodes.len(),
        "pages": nodes
    })))
}

// ─── Import Selected Pages ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ImportPagesRequest {
    urls: Vec<ImportPageEntry>,
}

#[derive(Debug, Deserialize)]
struct ImportPageEntry {
    url: String,
    title: Option<String>,
    depth: Option<u32>,
}

/// POST /api/v1/sources/:id/import-pages
///
/// Import selected discovered pages into crawled_pages table.
async fn import_pages(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<ImportPagesRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if source.is_none() {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"}))));
    }

    let mut imported = 0;
    let mut skipped = 0;

    for entry in &payload.urls {
        let depth = entry.depth.unwrap_or(0) as i32;
        let result = sqlx::query(
            "INSERT INTO crawled_pages (source_id, url, title, depth, status) VALUES (?, ?, ?, ?, 'pending') ON DUPLICATE KEY UPDATE title = VALUES(title), depth = VALUES(depth)"
        )
        .bind(id)
        .bind(&entry.url)
        .bind(&entry.title)
        .bind(depth)
        .execute(&pool)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => imported += 1,
            Ok(_) => skipped += 1,
            Err(e) => {
                warn!("Failed to import page {}: {}", entry.url, e);
                skipped += 1;
            }
        }
    }

    info!("Imported {} pages for source {} ({} skipped)", imported, id, skipped);

    Ok(Json(json!({
        "source_id": id,
        "imported": imported,
        "skipped": skipped,
        "total_requested": payload.urls.len()
    })))
}

// ─── LLM Fallback Extraction ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AiExtractRequest {
    model: String,
    output_format: String,  // "markdown" | "table"
}

#[derive(Debug, Serialize)]
struct AiExtractResponse {
    content: String,
    tokens_used: u32,
    model: String,
}

/// POST /api/v1/sources/:id/extract-ai
///
/// Use an LLM to extract content from a source file when native extraction fails.
/// Downloads the file from S3, sends its text content to the selected LLM,
/// and returns the extracted content.
async fn extract_with_ai(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<AiExtractRequest>,
) -> Result<Json<AiExtractResponse>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // 1. Fetch source from DB
    let source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let source = source.ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"})))
    })?;

    // 2. Download file from S3
    let s3_key = source.s3_key.as_deref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "Source has no S3 file — nothing to extract"})))
    })?;

    let file_data = download_from_s3(&config, s3_key).await
        .map_err(|e| {
            error!("S3 download failed for AI extraction: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to download file: {}", e)})))
        })?;

    info!("Downloaded {} bytes from S3 for AI extraction (source_id={})", file_data.len(), id);

    // 3. Look up model configuration from DB
    let model_config = db::get_model_by_id(&pool, &payload.model).await
        .map_err(|e| {
            error!("Failed to look up model {}: {}", payload.model, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Model lookup failed: {}", e)})))
        })?;

    // 4. Determine API key and endpoint from model config or env
    let (api_key, api_base) = resolve_llm_credentials(&config, &model_config, &payload.model)?;

    // 5. Build the prompt
    let file_text = String::from_utf8_lossy(&file_data);
    let ext = s3_key.rsplit('.').next().unwrap_or("unknown");
    let format_instruction = match payload.output_format.as_str() {
        "table" => "Extract all tabular data from this content. Output as a Markdown table with headers and rows. If there are multiple tables, include all of them with section headers.",
        _ => "Extract the full content from this document. Output as clean, well-structured Markdown with headings, paragraphs, and lists as appropriate. Preserve the original structure and meaning.",
    };

    let prompt = format!(
        "{format_instruction}\n\n\
         The file is a .{ext} file.\n\n\
         --- FILE CONTENT ---\n{file_text}\n--- END ---"
    );

    // 6. Call the LLM API (with usage logging)
    let provider_str = model_config.as_ref().map(|m| m.provider.as_str()).unwrap_or("unknown");
    let (content, tokens_used) = call_llm_api_with_logging(
        &api_key, &api_base, &payload.model, &prompt,
        Some(&pool), Some(&tenant_id), Some(provider_str), Some("extract_with_ai"),
    ).await
        .map_err(|e| {
            error!("LLM API call failed: {}", e);
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("LLM extraction failed: {}", e)})))
        })?;

    info!("AI extraction completed for source {}: {} chars, {} tokens used", id, content.len(), tokens_used);

    Ok(Json(AiExtractResponse {
        content,
        tokens_used,
        model: payload.model,
    }))
}

/// Resolve API key and base URL for the given model.
pub fn resolve_llm_credentials(
    config: &Config,
    model_config: &Option<mimir_core_ai::models::model_config::ModelConfig>,
    model_id: &str,
) -> Result<(String, String), (StatusCode, Json<Value>)> {
    // Try to get API key from model metadata first
    if let Some(mc) = model_config {
        let api_key = mc.metadata.as_ref()
            .and_then(|m| m.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let api_base = mc.metadata.as_ref()
            .and_then(|m| m.get("api_base"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let Some(key) = api_key {
            let base = api_base.unwrap_or_else(|| infer_api_base(&mc.provider));
            return Ok((key, base));
        }
    }

    // Fall back to env-based credentials
    let provider = model_config.as_ref().map(|m| m.provider.as_str()).unwrap_or("");
    match provider {
        "gemini" | "google" => {
            let key = config.gemini_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for Gemini. Set GEMINI_API_KEY or add api_key to model metadata."
                })))
            })?;
            Ok((key, config.gemini_base_url.clone()))
        }
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for OpenAI. Set OPENAI_API_KEY or add api_key to model metadata."
                })))
            })?;
            Ok((key, "https://api.openai.com/v1/".to_string()))
        }
        "ollama" => {
            // Ollama doesn't need an API key
            Ok(("ollama".to_string(), format!("{}/v1/", config.ollama_url)))
        }
        "heimdall" => {
            let key = config.heimdall_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for Heimdall. Set HEIMDALL_API_KEY or add api_key to model metadata."
                })))
            })?;
            let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
            Ok((key, base))
        }
        _ => {
            // Try model_id to infer provider
            if model_id.starts_with("gpt-") || model_id.starts_with("o1-") || model_id.starts_with("o3-") {
                let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": "No API key for OpenAI model"})))
                })?;
                Ok((key, "https://api.openai.com/v1/".to_string()))
            } else if model_id.starts_with("gemini-") {
                let key = config.gemini_api_key.clone().ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": "No API key for Gemini model"})))
                })?;
                Ok((key, config.gemini_base_url.clone()))
            } else if model_id.starts_with("mlx-community/") || model_id.starts_with("lmstudio-community/") {
                // MLX/lmstudio models → Heimdall gateway
                let key = config.heimdall_api_key.clone().ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": "No API key for Heimdall (inferred from mlx model)"})))
                })?;
                let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
                Ok((key, base))
            } else {
                // Default to Ollama for unknown models
                Ok(("ollama".to_string(), format!("{}/v1/", config.ollama_url)))
            }
        }
    }
}

/// Infer API base URL from provider name.
pub fn infer_api_base(provider: &str) -> String {
    match provider {
        "openai" => "https://api.openai.com/v1/".to_string(),
        "gemini" | "google" => "https://generativelanguage.googleapis.com/v1beta/openai/".to_string(),
        "ollama" => "http://localhost:11434/v1/".to_string(),
        "heimdall" => {
            std::env::var("HEIMDALL_API_URL")
                .unwrap_or_else(|_| "http://192.168.1.133:3000/v1".to_string())
                + "/"
        }
        _ => "http://localhost:11434/v1/".to_string(),
    }
}

/// Call an OpenAI-compatible chat completions API.
/// If `pool` and `tenant_id` are provided, automatically logs usage to `llm_usage_logs`.
pub async fn call_llm_api(
    api_key: &str,
    api_base: &str,
    model: &str,
    prompt: &str,
) -> anyhow::Result<(String, u32)> {
    call_llm_api_with_logging(api_key, api_base, model, prompt, None, None, None, None).await
}

/// Internal LLM call with optional usage logging and daily token limit enforcement.
pub async fn call_llm_api_with_logging(
    api_key: &str,
    api_base: &str,
    model: &str,
    prompt: &str,
    pool: Option<&DbPool>,
    tenant_id: Option<&str>,
    provider: Option<&str>,
    caller: Option<&str>,
) -> anyhow::Result<(String, u32)> {
    // ─── Daily Token Limit Check ────────────────────────────────────────
    if let (Some(p), Some(tid)) = (pool, tenant_id) {
        let limit: Option<(i64,)> = sqlx::query_as(
            "SELECT max_daily_tokens FROM tenant_configs WHERE tenant_id = ?"
        )
        .bind(tid)
        .fetch_optional(p)
        .await
        .unwrap_or(None);

        if let Some((max_tokens,)) = limit {
            if max_tokens > 0 {
                let today_usage: (i64,) = sqlx::query_as(
                    "SELECT COALESCE(SUM(total_tokens), 0) FROM llm_usage_logs WHERE tenant_id = ? AND DATE(created_at) = CURDATE()"
                )
                .bind(tid)
                .fetch_one(p)
                .await
                .unwrap_or((0,));

                if today_usage.0 >= max_tokens {
                    return Err(anyhow::anyhow!(
                        "Daily token limit exceeded: used {}/{} tokens today",
                        today_usage.0, max_tokens
                    ));
                }
            }
        }
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let url = format!("{}chat/completions", api_base);

    let body = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a document extraction assistant. Extract content accurately and completely. Do not add commentary or explanation — only output the extracted content."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "max_tokens": 16000,
        "temperature": 0.1
    });

    info!("Calling LLM API: {} with model {}", url, model);

    let response = client.post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP request to LLM failed: {}", e));

    let latency_ms = start.elapsed().as_millis() as i32;

    // Handle HTTP error
    let response = match response {
        Ok(r) => r,
        Err(e) => {
            // Log error if pool is available
            if let (Some(p), Some(tid)) = (pool, tenant_id) {
                let _ = crate::routes::llm_usage::insert_llm_usage_log(
                    p, tid, model, provider.unwrap_or("unknown"), Some(&url), caller,
                    0, 0, 0, latency_ms, "error", Some(&e.to_string()),
                ).await;
            }
            return Err(e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        // Log error
        if let (Some(p), Some(tid)) = (pool, tenant_id) {
            let _ = crate::routes::llm_usage::insert_llm_usage_log(
                p, tid, model, provider.unwrap_or("unknown"), Some(&url), caller,
                0, 0, 0, latency_ms, "error", Some(&error_body),
            ).await;
        }
        return Err(anyhow::anyhow!("LLM API returned {}: {}", status, error_body));
    }

    let resp_json: Value = response.json().await
        .map_err(|e| anyhow::anyhow!("Failed to parse LLM response: {}", e))?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let input_tokens = resp_json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as i32;
    let output_tokens = resp_json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as i32;
    let total_tokens = resp_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as i32;

    // Log success
    if let (Some(p), Some(tid)) = (pool, tenant_id) {
        let _ = crate::routes::llm_usage::insert_llm_usage_log(
            p, tid, model, provider.unwrap_or("unknown"), Some(&url), caller,
            input_tokens, output_tokens, total_tokens, latency_ms, "success", None,
        ).await;
    }

    Ok((content, total_tokens as u32))
}


// ─── URL Preview ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PreviewQuery {
    url: String,
}

/// GET /api/v1/sources/preview?url=https://example.com
///
/// Returns OG metadata preview (title, description, image, favicon).
async fn preview_url(
    Query(params): Query<PreviewQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("Preview requested for: {}", params.url);

    let preview = link_discovery::fetch_url_preview(&params.url).await
        .map_err(|e| {
            error!("Preview failed for {}: {}", params.url, e);
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("Failed to preview URL: {}", e)})))
        })?;

    Ok(Json(json!({
        "url": preview.url,
        "domain": preview.domain,
        "title": preview.title,
        "description": preview.description,
        "image": preview.image,
        "favicon": preview.favicon
    })))
}

// ─── S3 Helpers ────────────────────────────────────────────────────────────────

/// Download a file from RustFS/S3 by its key (public variant for cross-route use).
pub async fn download_from_s3_public(config: &Config, s3_key: &str) -> anyhow::Result<Vec<u8>> {
    download_from_s3(config, s3_key).await
}

/// Download a file from RustFS/S3 by its key.
///
/// Used by sync_source to retrieve uploaded files for extraction.
async fn download_from_s3(config: &Config, s3_key: &str) -> anyhow::Result<Vec<u8>> {
    let region = Region::Custom {
        region: config.s3_region.clone(),
        endpoint: config.s3_endpoint.clone(),
    };

    let credentials = Credentials::new(
        Some(&config.s3_access_key),
        Some(&config.s3_secret_key),
        None, None, None,
    ).map_err(|e| anyhow::anyhow!("S3 credentials error: {}", e))?;

    let bucket = Bucket::new(&config.s3_bucket, region, credentials)
        .map_err(|e| anyhow::anyhow!("S3 bucket error: {}", e))?
        .with_path_style();

    let response = bucket.get_object(s3_key).await
        .map_err(|e| anyhow::anyhow!("S3 get_object failed for '{}': {}", s3_key, e))?;

    if response.status_code() != 200 {
        return Err(anyhow::anyhow!(
            "S3 returned status {} for key '{}'", response.status_code(), s3_key
        ));
    }

    Ok(response.to_vec())
}

// ─── Upload Handler ────────────────────────────────────────────────────────────

/// Helper to create an S3 bucket client from centralized Config.
fn create_s3_bucket(config: &Config) -> Result<Box<Bucket>, (StatusCode, Json<Value>)> {
    let region = Region::Custom {
        region: config.s3_region.clone(),
        endpoint: config.s3_endpoint.clone(),
    };

    let credentials = Credentials::new(
        Some(&config.s3_access_key),
        Some(&config.s3_secret_key),
        None, None, None,
    ).map_err(|e| {
        error!("Failed to create S3 credentials: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "S3 configuration error"})))
    })?;

    let bucket = Bucket::new(&config.s3_bucket, region, credentials)
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
                user_source_type = Some(field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Invalid source_type field: {}", e)})))
                })?);
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

    // Auto-detect source_type from file extension (user-provided value is optional override)
    let source_type = user_source_type.unwrap_or_else(|| detect_source_type(&original_filename).to_string());

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
    match create_s3_bucket(&config) {
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
                storage_mode: Some(st_mode),
                s3_key: Some(s3_key_bg),
                file_hash: None,
                refresh_interval_hours: None,
                last_refreshed_at: None,
                next_refresh_at: None,
                refresh_status: None,
            };

            // Update status to RUNNING
            let _ = sqlx::query("UPDATE data_sources SET last_sync_status = 'RUNNING' WHERE id = ?")
                .bind(src_id)
                .execute(&pool_bg)
                .await;

            match IngressManager::process_source_with_data(&ds, &data) {
                Ok(raw_text) => {
                    let mb = raw_text.len() as f64 / 1_048_576.0;

                    // ─── Chunk the extracted text ────────────────────
                    let strategy = chunking::auto_recommend(&raw_text);
                    let chunk_results = chunking::chunk(&raw_text, &strategy).unwrap_or_default();
                    let chunks = chunk_results.len() as i32;

                    info!("Auto-extraction completed for {} ({}): {} bytes, {} chunks", src_name, src_id, raw_text.len(), chunks);

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
                            dedup_tracker.record_duplicate(cr.chunk_index, &content_hash, existing_source_id);
                            continue;
                        }

                        if let Some(existing_src) = dedup_tracker.is_seen(&content_hash) {
                            dedup_tracker.record_duplicate(cr.chunk_index, &content_hash, existing_src);
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
                        info!("Dedup report for source {}: {} unique, {} duplicates skipped",
                            src_id, dedup_tracker.report.unique_chunks, dedup_tracker.report.duplicate_chunks);
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
                },
                Err(e) => {
                    let err_msg = format!("{}", e);
                    error!("Auto-extraction failed for {} ({}): {}", src_name, src_id, err_msg);
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

// ─── TDD Tests for LLM Credential Resolution (#182) ───────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use mimir_core_ai::models::model_config::ModelConfig;

    /// Helper: create a minimal Config with Heimdall credentials set.
    fn test_config_with_heimdall() -> Config {
        Config {
            port: 3000,
            mariadb_url: String::new(),
            qdrant_url: String::new(),
            redis_url: String::new(),
            s3_endpoint: String::new(),
            s3_bucket: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            s3_region: String::new(),
            ollama_url: "http://localhost:11434".to_string(),
            local_model: String::new(),
            embed_model: String::new(),
            gemini_base_url: "https://generativelanguage.googleapis.com/v1beta/openai/".to_string(),
            gemini_api_key: Some("test-gemini-key".to_string()),
            gemini_model: String::new(),
            heimdall_api_url: "http://192.168.1.133:3000/v1".to_string(),
            heimdall_api_key: Some("test-heimdall-key".to_string()),
            heimdall_model: "mlx-community/Qwen3.5-35B-A3B-4bit".to_string(),
            neo4j_uri: "bolt://localhost:7687".to_string(),
            neo4j_user: "neo4j".to_string(),
            neo4j_password: "test_password".to_string(),
            jwt_secret: String::new(),
        }
    }

    /// Helper: create a minimal Config without Heimdall API key.
    fn test_config_no_heimdall_key() -> Config {
        let mut cfg = test_config_with_heimdall();
        cfg.heimdall_api_key = None;
        cfg
    }

    fn make_model_config(provider: &str) -> Option<ModelConfig> {
        Some(ModelConfig {
            model_id: "test-model".to_string(),
            provider: provider.to_string(),
            model_type: "chat".to_string(),
            is_active: true,
            capabilities: None,
            metadata: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    // ─── infer_api_base tests ──────────────────────────────────────────

    #[test]
    fn test_infer_api_base_heimdall() {
        // Set env for test
        unsafe { std::env::set_var("HEIMDALL_API_URL", "http://192.168.1.133:3000/v1"); }
        let base = infer_api_base("heimdall");
        assert!(base.contains("192.168.1.133"), "Heimdall base should contain gateway host, got: {}", base);
        assert!(base.ends_with('/'), "Base URL should end with /");
    }

    #[test]
    fn test_infer_api_base_openai() {
        let base = infer_api_base("openai");
        assert_eq!(base, "https://api.openai.com/v1/");
    }

    #[test]
    fn test_infer_api_base_gemini() {
        let base = infer_api_base("gemini");
        assert!(base.contains("generativelanguage.googleapis.com"));
    }

    #[test]
    fn test_infer_api_base_ollama() {
        let base = infer_api_base("ollama");
        assert_eq!(base, "http://localhost:11434/v1/");
    }

    #[test]
    fn test_infer_api_base_unknown_defaults_to_ollama() {
        let base = infer_api_base("some_unknown_provider");
        assert_eq!(base, "http://localhost:11434/v1/");
    }

    // ─── resolve_llm_credentials tests ─────────────────────────────────

    #[test]
    fn test_resolve_heimdall_provider_explicit() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("heimdall");
        let result = resolve_llm_credentials(&config, &mc, "mlx-community/Qwen3.5-9B-MLX-4bit");
        assert!(result.is_ok(), "Should resolve Heimdall credentials");
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
        assert!(base.contains("192.168.1.133"), "Base should be Heimdall URL, got: {}", base);
    }

    #[test]
    fn test_resolve_heimdall_via_mlx_prefix() {
        let config = test_config_with_heimdall();
        // No model_config (None) — should infer from model_id prefix
        let result = resolve_llm_credentials(&config, &None, "mlx-community/Qwen3.5-35B-A3B-4bit");
        assert!(result.is_ok(), "Should infer Heimdall from mlx-community/ prefix");
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
        assert!(base.contains("192.168.1.133:3000"));
    }

    #[test]
    fn test_resolve_heimdall_via_lmstudio_prefix() {
        let config = test_config_with_heimdall();
        let result = resolve_llm_credentials(&config, &None, "lmstudio-community/medgemma-4b-it-MLX-4bit");
        assert!(result.is_ok(), "Should infer Heimdall from lmstudio-community/ prefix");
        let (key, _) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
    }

    #[test]
    fn test_resolve_heimdall_missing_key_returns_error() {
        let config = test_config_no_heimdall_key();
        let mc = make_model_config("heimdall");
        let result = resolve_llm_credentials(&config, &mc, "some-model");
        assert!(result.is_err(), "Should return error when Heimdall API key is missing");
    }

    #[test]
    fn test_resolve_ollama_still_works() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("ollama");
        let result = resolve_llm_credentials(&config, &mc, "llama3.2");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "ollama");
        assert!(base.contains("11434"));
    }

    #[test]
    fn test_resolve_gemini_still_works() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("gemini");
        let result = resolve_llm_credentials(&config, &mc, "gemini-2.5-flash");
        assert!(result.is_ok());
        let (key, _) = result.unwrap();
        assert_eq!(key, "test-gemini-key");
    }

    #[test]
    fn test_resolve_unknown_model_defaults_to_ollama() {
        let config = test_config_with_heimdall();
        let result = resolve_llm_credentials(&config, &None, "some-random-model");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "ollama");
        assert!(base.contains("11434"));
    }
}
