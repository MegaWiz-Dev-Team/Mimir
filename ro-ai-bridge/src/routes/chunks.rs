use axum::{
    routing::get,
    Router, Json,
    extract::{Path, State, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::models::chunks::Chunk;

#[derive(Debug, Deserialize)]
pub struct ChunkListQuery {
    pub source_id: Option<i64>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ChunkListResponse {
    pub chunks: Vec<ChunkWithSource>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Serialize)]
pub struct ChunkWithSource {
    pub id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: Option<i32>,
    pub metadata_json: Option<serde_json::Value>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn chunks_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_chunks))
        .route("/{id}", get(get_chunk))
}

async fn list_chunks(
    State(pool): State<DbPool>,
    Query(params): Query<ChunkListQuery>,
) -> Result<Json<ChunkListResponse>, (StatusCode, Json<Value>)> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let tenant_id = "default_tenant"; // Future: extract from JWT

    // Build dynamic query based on filters — all queries filter by tenant_id via data_sources JOIN
    let (count_query, data_query) = if let Some(ref search) = params.search {
        if let Some(source_id) = params.source_id {
            (
                format!("SELECT COUNT(*) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}' AND c.source_id = {} AND c.content LIKE '%{}%'", tenant_id, source_id, search.replace('\'', "''")),
                format!(
                    "SELECT c.id, c.source_id, COALESCE(d.name, 'Unknown') as source_name, c.chunk_index, c.content, c.token_count, c.metadata_json, c.created_at \
                     FROM chunks c JOIN data_sources d ON c.source_id = d.id \
                     WHERE d.tenant_id = '{}' AND c.source_id = {} AND c.content LIKE '%{}%' \
                     ORDER BY c.id DESC LIMIT {} OFFSET {}",
                    tenant_id, source_id, search.replace('\'', "''"), per_page, offset
                ),
            )
        } else {
            (
                format!("SELECT COUNT(*) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}' AND c.content LIKE '%{}%'", tenant_id, search.replace('\'', "''")),
                format!(
                    "SELECT c.id, c.source_id, COALESCE(d.name, 'Unknown') as source_name, c.chunk_index, c.content, c.token_count, c.metadata_json, c.created_at \
                     FROM chunks c JOIN data_sources d ON c.source_id = d.id \
                     WHERE d.tenant_id = '{}' AND c.content LIKE '%{}%' \
                     ORDER BY c.id DESC LIMIT {} OFFSET {}",
                    tenant_id, search.replace('\'', "''"), per_page, offset
                ),
            )
        }
    } else if let Some(source_id) = params.source_id {
        (
            format!("SELECT COUNT(*) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}' AND c.source_id = {}", tenant_id, source_id),
            format!(
                "SELECT c.id, c.source_id, COALESCE(d.name, 'Unknown') as source_name, c.chunk_index, c.content, c.token_count, c.metadata_json, c.created_at \
                 FROM chunks c JOIN data_sources d ON c.source_id = d.id \
                 WHERE d.tenant_id = '{}' AND c.source_id = {} \
                 ORDER BY c.id DESC LIMIT {} OFFSET {}",
                tenant_id, source_id, per_page, offset
            ),
        )
    } else {
        (
            format!("SELECT COUNT(*) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}'", tenant_id),
            format!(
                "SELECT c.id, c.source_id, COALESCE(d.name, 'Unknown') as source_name, c.chunk_index, c.content, c.token_count, c.metadata_json, c.created_at \
                 FROM chunks c JOIN data_sources d ON c.source_id = d.id \
                 WHERE d.tenant_id = '{}' \
                 ORDER BY c.id DESC LIMIT {} OFFSET {}",
                tenant_id, per_page, offset
            ),
        )
    };

    let total: (i64,) = sqlx::query_as(&count_query)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    let chunks: Vec<ChunkWithSource> = sqlx::query_as::<_, (i64, i64, String, i32, String, Option<i32>, Option<serde_json::Value>, Option<chrono::DateTime<chrono::Utc>>)>(
        &data_query
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
    .into_iter()
    .map(|(id, source_id, source_name, chunk_index, content, token_count, metadata_json, created_at)| {
        ChunkWithSource { id, source_id, source_name, chunk_index, content, token_count, metadata_json, created_at }
    })
    .collect();

    Ok(Json(ChunkListResponse {
        chunks,
        total: total.0,
        page,
        per_page,
    }))
}

async fn get_chunk(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<ChunkWithSource>, (StatusCode, Json<Value>)> {
    let result: Option<(i64, i64, String, i32, String, Option<i32>, Option<serde_json::Value>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT c.id, c.source_id, COALESCE(d.name, 'Unknown') as source_name, c.chunk_index, c.content, c.token_count, c.metadata_json, c.created_at \
         FROM chunks c LEFT JOIN data_sources d ON c.source_id = d.id \
         WHERE c.id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match result {
        Some((id, source_id, source_name, chunk_index, content, token_count, metadata_json, created_at)) => {
            Ok(Json(ChunkWithSource { id, source_id, source_name, chunk_index, content, token_count, metadata_json, created_at }))
        }
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Chunk not found"})))),
    }
}
