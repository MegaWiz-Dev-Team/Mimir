//! Sync orchestration: trigger source sync and SSE log streaming.

use crate::config::Config;
use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::stream::Stream;
use mimir_core_ai::models::sources::DataSource;
use mimir_core_ai::services::chunking;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::dedup;
use mimir_core_ai::services::ingress::IngressManager;
use mimir_core_ai::services::link_discovery;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use super::upload::download_from_s3;

pub(crate) async fn sync_source(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Check if source exists
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

    if source.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Source not found"})),
        ));
    }

    // Update status to RUNNING
    sqlx::query("UPDATE data_sources SET last_sync_status = 'RUNNING' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    let pool_clone = pool.clone();
    let config_clone = config.clone();
    let source_clone = source.unwrap();
    // Spawn a background task to process the source
    tokio::spawn(async move {
        info!("Started background sync task for source id {}", id);

        let mut result: Result<String, anyhow::Error> = match source_clone.source_type.as_str() {
            // File-based sources: download from S3, then extract
            "file" | "document" | "tabular" => {
                match &source_clone.s3_key {
                    Some(s3_key) if !s3_key.is_empty() => {
                        info!("Downloading from S3: key={}", s3_key);
                        // Download file from RustFS
                        match download_from_s3(&config_clone, s3_key).await {
                            Ok(data) => {
                                info!(
                                    "Downloaded {} bytes from S3, running extraction",
                                    data.len()
                                );
                                let mut extraction = IngressManager::process_source_with_data(&source_clone, &data);

                                // ─── OCR Fallback: if PDF extraction returned empty (image-only PDF),
                                //     use Gemini Vision to OCR the document ───────────────────────
                                if let Err(ref e) = extraction {
                                    let err_msg = e.to_string();
                                    if err_msg.contains("image-only") || err_msg.contains("empty text") {
                                        let ext = s3_key.rsplit('.').next().unwrap_or("").to_lowercase();
                                        if ext == "pdf" {
                                            info!(
                                                "PDF text extraction empty for {} ({}), falling back to Gemini Vision OCR",
                                                source_clone.name, id
                                            );

                                            let gemini_api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
                                            let gemini_base = std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| {
                                                "https://generativelanguage.googleapis.com/v1beta/openai/".to_string()
                                            });

                                            if !gemini_api_key.is_empty() {
                                                match mimir_core_ai::services::ocr::extract_text_from_image(
                                                    &data,
                                                    s3_key,
                                                    &gemini_api_key,
                                                    &gemini_base,
                                                    "gemini-3-flash-preview",
                                                )
                                                .await
                                                {
                                                    Ok((ocr_text, tokens)) => {
                                                        info!(
                                                            "Gemini OCR fallback succeeded for {} ({}): {} chars, {} tokens",
                                                            source_clone.name, id, ocr_text.len(), tokens
                                                        );
                                                        extraction = Ok(ocr_text);
                                                    }
                                                    Err(ocr_err) => {
                                                        error!(
                                                            "Gemini OCR fallback also failed for {} ({}): {}",
                                                            source_clone.name, id, ocr_err
                                                        );
                                                        // Keep original error
                                                    }
                                                }
                                            } else {
                                                warn!(
                                                    "GEMINI_API_KEY not set — cannot OCR fallback for image-only PDF (source_id={})",
                                                    id
                                                );
                                            }
                                        }
                                    }
                                }

                                extraction
                            }
                            Err(e) => Err(anyhow::anyhow!("S3 download failed: {}", e)),
                        }
                    }
                    _ => Err(anyhow::anyhow!(
                        "No S3 key found for source '{}' — file may not have been uploaded",
                        source_clone.name
                    )),
                }
            }
            // Network sources: fetch + extract
            _ => IngressManager::process_source(&source_clone).await,
        };

        // If the user manually edited and saved AI extraction, the source is COMPLETED.
        // We reuse their raw_markdown to generate chunks instead of destroying their work!
        if source_clone.last_sync_status.as_deref() == Some("COMPLETED") {
            if let Some(ref md) = source_clone.raw_markdown {
                if !md.trim().is_empty() {
                    info!("Using manually saved raw_markdown for source id {} (bypassing extraction)", id);
                    result = Ok(md.clone());
                }
            }
        }

        match result {
            Ok(raw_text) => {
                let mb_size = raw_text.len() as f64 / 1_048_576.0;

                // ─── Chunk the extracted text (A2: use tenant config if set) ────
                let strategy = {
                    let iam =
                        mimir_core_ai::services::iam::IamService::new_with_env(pool_clone.clone());
                    let tenant_cfg = iam.get_tenant_config(&source_clone.tenant_id).await.ok();
                    let pipeline_settings = tenant_cfg
                        .as_ref()
                        .and_then(|c| c.pipeline_settings.as_ref());

                    match pipeline_settings
                        .and_then(|p| p.get("chunk_strategy").and_then(|v| v.as_str()))
                    {
                        Some("fixed") => {
                            let size = pipeline_settings
                                .and_then(|p| p.get("chunk_size").and_then(|v| v.as_u64()))
                                .unwrap_or(512) as usize;
                            let overlap = pipeline_settings
                                .and_then(|p| p.get("chunk_overlap").and_then(|v| v.as_u64()))
                                .unwrap_or(50) as usize;
                            chunking::ChunkStrategy::Fixed { size, overlap }
                        }
                        Some("recursive") => {
                            let max_size = pipeline_settings
                                .and_then(|p| p.get("chunk_size").and_then(|v| v.as_u64()))
                                .unwrap_or(512) as usize;
                            chunking::ChunkStrategy::Recursive { max_size }
                        }
                        _ => chunking::auto_recommend(&raw_text), // "auto" or unset
                    }
                };
                let chunk_results = chunking::chunk(&raw_text, &strategy).unwrap_or_default();
                let total_chunks = chunk_results.len() as i32;

                info!(
                    "Sync completed for {} ({}): {} bytes, {} chunks",
                    source_clone.name,
                    id,
                    raw_text.len(),
                    total_chunks
                );

                // Store chunks in DB (with dedup)
                let mut dedup_tracker = dedup::DedupTracker::new();
                for cr in &chunk_results {
                    let content_hash = dedup::fingerprint(&cr.content);

                    // Check for existing fingerprint in DB
                    let existing: Option<(i64,)> = sqlx::query_as(
                        "SELECT source_id FROM content_fingerprints WHERE content_hash = ? LIMIT 1",
                    )
                    .bind(&content_hash)
                    .fetch_optional(&pool_clone)
                    .await
                    .unwrap_or(None);

                    if let Some((existing_source_id,)) = existing {
                        dedup_tracker.record_duplicate(
                            cr.chunk_index,
                            &content_hash,
                            existing_source_id,
                        );
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
                    info!(
                        "Dedup report for source {}: {} unique, {} duplicates skipped",
                        id,
                        dedup_tracker.report.unique_chunks,
                        dedup_tracker.report.duplicate_chunks
                    );
                }
                let total_chunks = dedup_tracker.report.unique_chunks as i32;

                // ─── Link Discovery for web sources ─────────────────
                if source_clone.source_type == "web" {
                    let content_hash = link_discovery::compute_content_hash(&raw_text);
                    let source_url = source_clone
                        .config_json
                        .get("url")
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

                    // Discover linked pages (same domain, respecting tenant config)
                    let max_pages = {
                        let iam = mimir_core_ai::services::iam::IamService::new_with_env(
                            pool_clone.clone(),
                        );
                        let tid = &source_clone.tenant_id;
                        iam.get_tenant_config(tid)
                            .await
                            .map(|c| c.max_crawl_pages as usize)
                            .unwrap_or(50)
                    };
                    let discovered =
                        link_discovery::discover_links(&raw_text, source_url, max_pages);
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
                let _ = sqlx::query(
                    "UPDATE data_sources SET last_sync_status = 'COMPLETED', raw_markdown = ?, mb_size = ?, total_chunks = ?, last_sync_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(&raw_text)
                .bind(mb_size)
                .bind(total_chunks)
                .bind(id)
                .execute(&pool_clone)
                .await
                .map_err(|e| error!("Failed to update source {} to COMPLETED: {}", id, e));
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                error!(
                    "Sync failed for {} ({}): {}",
                    source_clone.name, id, error_msg
                );
                let _ = sqlx::query(
                    "UPDATE data_sources SET last_sync_status = 'FAILED', raw_markdown = ? WHERE id = ?"
                )
                .bind(&error_msg)
                .bind(id)
                .execute(&pool_clone)
                .await
                .map_err(|e| error!("Failed to update source {} to FAILED: {}", id, e));
            }
        }
    });

    info!("Triggered sync for source id {}", id);

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "message": "Sync job triggered successfully",
            "source_id": id
        })),
    ))
}

// Simulated SSE stream for real-time logs
pub(crate) async fn stream_logs(
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
