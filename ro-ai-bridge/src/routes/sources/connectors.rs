//! Web connectors: hierarchy discovery, page import, and URL preview.

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use mimir_core_ai::models::sources::DataSource;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::link_discovery;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info, warn};

// ─── Web Hierarchy Discovery ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct DiscoverHierarchyRequest {
    max_depth: Option<u32>,
    max_pages: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct HierarchyNode {
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
pub(crate) async fn discover_hierarchy(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<DiscoverHierarchyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let max_depth = payload.max_depth.unwrap_or(3).min(5);
    // Read configurable max from tenant config (Issue #164)
    let tenant_max: i32 =
        sqlx::query_scalar("SELECT max_crawl_pages FROM tenant_configs WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None)
            .unwrap_or(100);
    let max_pages = payload.max_pages.unwrap_or(tenant_max as u32).min(500);

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

    let root_url = source
        .config_json
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Source has no URL configured"})),
            )
        })?
        .to_string();

    info!(
        "Starting hierarchy discovery for source {} from {}",
        id, root_url
    );

    // BFS crawl
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

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
            Ok(resp) if resp.status().is_success() => resp.text().await.unwrap_or_default(),
            _ => {
                all_pages.push((url.clone(), None, depth, String::new()));
                continue;
            }
        };

        let doc = scraper::Html::parse_document(&html);
        let title_sel = scraper::Selector::parse("title").unwrap();
        let title = doc
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string());

        let content_hash = link_discovery::compute_content_hash(&html);
        all_pages.push((url.clone(), title, depth, content_hash));

        if depth < max_depth {
            let links = link_discovery::discover_links(&html, &url, 200);
            for link in links {
                if !visited.contains(&link.url)
                    && all_pages.len() + queue.len() < max_pages as usize
                {
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
                "SELECT content_hash FROM crawled_pages WHERE source_id = ? AND url = ?",
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
                        "SELECT source_id FROM content_fingerprints WHERE content_hash = ? LIMIT 1",
                    )
                    .bind(content_hash)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or(None);

                    if dup.is_some() {
                        "duplicate".to_string()
                    } else {
                        "new".to_string()
                    }
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
pub(crate) struct ImportPagesRequest {
    urls: Vec<ImportPageEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImportPageEntry {
    url: String,
    title: Option<String>,
    depth: Option<u32>,
}

/// POST /api/v1/sources/:id/import-pages
///
/// Import selected discovered pages into crawled_pages table.
pub(crate) async fn import_pages(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<ImportPagesRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

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

    info!(
        "Imported {} pages for source {} ({} skipped)",
        imported, id, skipped
    );

    Ok(Json(json!({
        "source_id": id,
        "imported": imported,
        "skipped": skipped,
        "total_requested": payload.urls.len()
    })))
}

// ─── URL Preview ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct PreviewQuery {
    url: String,
}

/// GET /api/v1/sources/preview?url=https://example.com
///
/// Returns OG metadata preview (title, description, image, favicon).
pub(crate) async fn preview_url(
    Query(params): Query<PreviewQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("Preview requested for: {}", params.url);

    let preview = link_discovery::fetch_url_preview(&params.url)
        .await
        .map_err(|e| {
            error!("Preview failed for {}: {}", params.url, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Failed to preview URL: {}", e)})),
            )
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
