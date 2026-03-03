use axum::{
    routing::{get, post},
    Router, Json,
    extract::{Path, State, Query},
    http::{StatusCode, HeaderMap},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::models::chunks::Chunk;
use crate::routes::tenant::extract_tenant_id;

#[derive(Debug, Deserialize)]
pub struct ChunkListQuery {
    pub source_id: Option<i64>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateQaRequest {
    pub chunk_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct ChunkListResponse {
    pub chunks: Vec<ChunkWithSource>,
    pub total: i64,
    pub total_tokens: i64,
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
        .route("/generate-qa", post(generate_qa_for_chunks))
        .route("/{id}", get(get_chunk))
}

async fn list_chunks(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<ChunkListQuery>,
) -> Result<Json<ChunkListResponse>, (StatusCode, Json<Value>)> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let tenant_id = extract_tenant_id(&headers);

    // Build dynamic query based on filters — all queries filter by tenant_id via data_sources JOIN
    let (count_query, data_query) = if let Some(ref search) = params.search {
        if let Some(source_id) = params.source_id {
            (
                format!("SELECT COUNT(*), CAST(COALESCE(SUM(c.token_count), 0) AS SIGNED) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}' AND c.source_id = {} AND c.content LIKE '%{}%'", tenant_id, source_id, search.replace('\'', "''")),
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
                format!("SELECT COUNT(*), CAST(COALESCE(SUM(c.token_count), 0) AS SIGNED) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}' AND c.content LIKE '%{}%'", tenant_id, search.replace('\'', "''")),
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
            format!("SELECT COUNT(*), CAST(COALESCE(SUM(c.token_count), 0) AS SIGNED) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}' AND c.source_id = {}", tenant_id, source_id),
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
            format!("SELECT COUNT(*), CAST(COALESCE(SUM(c.token_count), 0) AS SIGNED) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = '{}'", tenant_id),
            format!(
                "SELECT c.id, c.source_id, COALESCE(d.name, 'Unknown') as source_name, c.chunk_index, c.content, c.token_count, c.metadata_json, c.created_at \
                 FROM chunks c JOIN data_sources d ON c.source_id = d.id \
                 WHERE d.tenant_id = '{}' \
                 ORDER BY c.id DESC LIMIT {} OFFSET {}",
                tenant_id, per_page, offset
            ),
        )
    };

    let count_row: (i64, i64) = sqlx::query_as(&count_query)
        .fetch_one(&pool)
        .await
        .unwrap_or((0, 0));

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
        total: count_row.0,
        total_tokens: count_row.1,
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

/// POST /api/v1/chunks/generate-qa — Generate QA for selected chunks
async fn generate_qa_for_chunks(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(req): Json<GenerateQaRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if req.chunk_ids.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "No chunks selected"}))));
    }

    let tenant_id = extract_tenant_id(&headers).to_string();
    let chunk_count = req.chunk_ids.len();

    // Verify chunks exist and belong to this tenant
    let placeholders: Vec<String> = req.chunk_ids.iter().map(|_| "?".to_string()).collect();
    let verify_query = format!(
        "SELECT COUNT(*) FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE d.tenant_id = ? AND c.id IN ({})",
        placeholders.join(",")
    );
    let mut query = sqlx::query_scalar::<_, i64>(&verify_query).bind(&tenant_id);
    for id in &req.chunk_ids {
        query = query.bind(id);
    }
    let verified_count = query.fetch_one(&pool).await.unwrap_or(0);

    if verified_count == 0 {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "No matching chunks found for this tenant"}))));
    }

    // Spawn background QA generation
    let pool_clone = pool.clone();
    let chunk_ids = req.chunk_ids.clone();
    let tenant_clone = tenant_id.clone();

    tokio::spawn(async move {
        tracing::info!("Starting QA generation for {} chunks (tenant: {})", chunk_ids.len(), tenant_clone);
        for chunk_id in &chunk_ids {
            // Fetch chunk content
            let chunk_result: Option<(String,)> = sqlx::query_as(
                "SELECT c.content FROM chunks c JOIN data_sources d ON c.source_id = d.id WHERE c.id = ? AND d.tenant_id = ?"
            )
            .bind(chunk_id)
            .bind(&tenant_clone)
            .fetch_optional(&pool_clone)
            .await
            .unwrap_or(None);

            if let Some((content,)) = chunk_result {
                // Mark chunk as QA in-progress via metadata
                let _ = sqlx::query("UPDATE chunks SET metadata_json = JSON_SET(COALESCE(metadata_json, '{}'), '$.qa_status', 'processing') WHERE id = ?")
                    .bind(chunk_id)
                    .execute(&pool_clone)
                    .await;

                // Use clustering service to generate QA for this chunk content
                match mimir_core_ai::qa_qc::clustering::ClusteringService::generate_qa_for_content(
                    &pool_clone, &tenant_clone, *chunk_id, &content
                ).await {
                    Ok(_) => {
                        let _ = sqlx::query("UPDATE chunks SET metadata_json = JSON_SET(COALESCE(metadata_json, '{}'), '$.qa_status', 'completed') WHERE id = ?")
                            .bind(chunk_id)
                            .execute(&pool_clone)
                            .await;
                        tracing::info!("QA generated for chunk {}", chunk_id);
                    }
                    Err(e) => {
                        let _ = sqlx::query("UPDATE chunks SET metadata_json = JSON_SET(COALESCE(metadata_json, '{}'), '$.qa_status', 'failed') WHERE id = ?")
                            .bind(chunk_id)
                            .execute(&pool_clone)
                            .await;
                        tracing::error!("QA generation failed for chunk {}: {}", chunk_id, e);
                    }
                }
            }
        }
        tracing::info!("QA generation completed for {} chunks", chunk_ids.len());
    });

    Ok(Json(json!({
        "success": true,
        "message": format!("QA generation started for {} chunks", chunk_count),
        "chunk_count": chunk_count
    })))
}
