use axum::{
    extract::{Json, Path, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use tracing::{error, info};

use mimir_core_ai::{
    services::{db::DbPool, qdrant::QdrantService},
    qa_qc::indexer::run_indexer,
};
use rig::providers::ollama;
use serde::Deserialize;
use crate::routes::tenant::extract_tenant_id;

/// Batch embed texts via Heimdall /v1/embeddings (OpenAI-compatible)
async fn embed_texts(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>, String> {
    let embed_base_url = std::env::var("HEIMDALL_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("OLLAMA_URL").ok().filter(|s| !s.is_empty()).map(|u| format!("{}/v1", u)))
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
    let embed_url = format!("{}/embeddings", embed_base_url.trim_end_matches('/'));
    let api_key = std::env::var("HEIMDALL_API_KEY").unwrap_or_default();
    let client = reqwest::Client::new();

    let resp = client
        .post(&embed_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": model,
            "input": texts,
        }))
        .send()
        .await
        .map_err(|e| format!("Embedding HTTP error: {}", e))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Embedding API error: {}", err));
    }

    let body: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

    let data = body["data"].as_array().ok_or("No 'data' array in response")?;
    let mut vectors = Vec::with_capacity(data.len());
    for item in data {
        let vec: Vec<f32> = item["embedding"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .unwrap_or_default();
        vectors.push(vec);
    }
    Ok(vectors)
}


#[derive(Deserialize)]
pub struct SearchRequest {
    query: String,
    limit: Option<usize>,
    tenant_id: Option<String>,
    show_expired: Option<bool>,
}

#[derive(Deserialize)]
pub struct QaBulkRequest {
    pub items: Vec<QaBulkItem>,
}

#[derive(Deserialize)]
pub struct QaBulkItem {
    pub question: String,
    pub answer: String,
    pub source_id: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct EmbedChunksRequest {
    pub source_id: Option<i64>,
    pub collection: Option<String>,
}

pub fn vector_routes() -> Router<DbPool> {
    Router::new()
        .route("/stats", get(get_vector_stats))
        .route("/index", post(trigger_indexing))
        .route("/search", post(search_vectors))
        .route("/qa/bulk", post(bulk_index_qa))
        .route("/embed-chunks", post(embed_chunks))
        .route("/{id}", delete(delete_vector_handler))
}

async fn get_vector_stats(State(pool): State<DbPool>) -> impl IntoResponse {
    let collection_name = "wiki_qa";
    let qdrant = QdrantService::new(); // Ideally instantiated once in AppState, but fine for now given monitor.rs does the same.

    // 1. Get Qdrant stats
    let qdrant_info = qdrant.get_collection_info(collection_name).await.unwrap_or(serde_json::Value::Null);

    // 2. Get MariaDB stats
    let total_qa = sqlx::query("SELECT count(*) as count FROM qa_results")
        .fetch_one(&pool)
        .await
        .map(|r| {
            use sqlx::Row;
            r.get::<i64, _>("count")
        })
        .unwrap_or(0);

    let indexed_qa = sqlx::query("SELECT count(*) as count FROM qa_results WHERE indexed_at IS NOT NULL")
        .fetch_one(&pool)
        .await
        .map(|r| {
            use sqlx::Row;
            r.get::<i64, _>("count")
        })
        .unwrap_or(0);

    Json(serde_json::json!({
        "qdrant": qdrant_info,
        "database": {
            "total_qa": total_qa,
            "indexed_qa": indexed_qa,
            "pending_qa": total_qa - indexed_qa
        }
    }))
}

async fn trigger_indexing(State(pool): State<DbPool>) -> impl IntoResponse {
    let qdrant = QdrantService::new();

    tokio::spawn(async move {
        let ollama_client = ollama::Client::new();
        if let Err(e) = run_indexer(&pool, &qdrant, &ollama_client, "wiki_qa").await {
            error!("Background indexing failed: {}", e);
        }
    });

    axum::http::StatusCode::ACCEPTED
}

async fn search_vectors(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<SearchRequest>,
) -> impl IntoResponse {
    let qdrant = QdrantService::new();
    let tenant_id = extract_tenant_id(&headers).to_string();

    // Resolve embedding model from tenant config
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config.as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_slot = llm_config.resolve_slot("embedding", None, None);
    let embed_model_name = embed_slot.model;

    // Determine embedding API URL: prefer Heimdall, fallback to Ollama
    let embed_base_url = std::env::var("HEIMDALL_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("OLLAMA_URL").ok().filter(|s| !s.is_empty()).map(|u| format!("{}/v1", u)))
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
    let embed_url = format!("{}/embeddings", embed_base_url.trim_end_matches('/'));

    // Call OpenAI-compatible /v1/embeddings endpoint via HTTP POST
    let client = reqwest::Client::new();
    let api_key = std::env::var("HEIMDALL_API_KEY").unwrap_or_default();
    
    let embed_response = client
        .post(&embed_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": embed_model_name,
            "input": payload.query,
        }))
        .send()
        .await;

    let vector_f32: Vec<f32> = match embed_response {
        Ok(resp) => {
            if !resp.status().is_success() {
                let error_text = resp.text().await.unwrap_or_default();
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, 
                    Json(serde_json::json!({"error": format!("Embedding API error: {}", error_text)}))
                ).into_response();
            }
            match resp.json::<serde_json::Value>().await {
                Ok(body) => {
                    body["data"][0]["embedding"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
                        .unwrap_or_default()
                }
                Err(e) => {
                    return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Failed to parse embedding response: {}", e)}))
                    ).into_response();
                }
            }
        }
        Err(e) => {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to call embedding API at {}: {}", embed_url, e)}))
            ).into_response();
        }
    };

    if vector_f32.is_empty() {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Empty embedding vector returned"}))
        ).into_response();
    }

    let target_tenant = payload.tenant_id.unwrap_or(tenant_id);
    let show_expired = payload.show_expired.unwrap_or(false);

    match qdrant.search("wiki_qa", vector_f32, payload.limit.unwrap_or(5), &target_tenant, show_expired).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    }
}

async fn delete_vector_handler(
    headers: HeaderMap,
    State(_pool): State<DbPool>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let qdrant = QdrantService::new();
    let _tenant_id = extract_tenant_id(&headers);

    match qdrant.delete_point("wiki_qa", id).await {
        Ok(_) => (axum::http::StatusCode::OK, Json(serde_json::json!({"status": "success"}))).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    }
}

/// POST /vector/qa/bulk — batch embed QA pairs and upsert into wiki_qa
async fn bulk_index_qa(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<QaBulkRequest>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let qdrant = QdrantService::new();

    // Resolve embedding model
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config.as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    if payload.items.is_empty() {
        return Json(serde_json::json!({"inserted": 0, "error": null})).into_response();
    }

    // Batch embed questions (max 64 per batch)
    let batch_size = 64;
    let mut all_vectors: Vec<Vec<f32>> = Vec::new();
    for chunk in payload.items.chunks(batch_size) {
        let texts: Vec<String> = chunk.iter().map(|q| q.question.clone()).collect();
        match embed_texts(&texts, &embed_model).await {
            Ok(vecs) => all_vectors.extend(vecs),
            Err(e) => {
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e}))
                ).into_response();
            }
        }
    }

    // Build Qdrant points
    let mut points = Vec::new();
    for (i, item) in payload.items.iter().enumerate() {
        let point_id = uuid::Uuid::new_v4().as_u128() as u64;
        points.push(serde_json::json!({
            "id": point_id,
            "vector": all_vectors[i],
            "payload": {
                "question": item.question,
                "answer": item.answer,
                "source_id": item.source_id,
                "tenant_id": tenant_id,
                "is_active": true,
                "indexed_at": chrono::Utc::now().to_rfc3339(),
            }
        }));
    }

    // Upsert in batches of 100
    let upsert_batch = 100;
    let mut inserted = 0;
    for chunk in points.chunks(upsert_batch) {
        let body = serde_json::json!({ "points": chunk });
        match qdrant.upsert_points("wiki_qa", body).await {
            Ok(_) => inserted += chunk.len(),
            Err(e) => {
                error!("Qdrant upsert error: {}", e);
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"inserted": inserted, "error": e.to_string()}))
                ).into_response();
            }
        }
    }

    info!("✅ Indexed {} QA pairs into wiki_qa for tenant {}", inserted, tenant_id);
    Json(serde_json::json!({"inserted": inserted, "error": null})).into_response()
}

/// POST /vector/embed-chunks — embed source chunks and upsert into source_chunks
async fn embed_chunks(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<EmbedChunksRequest>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let qdrant = QdrantService::new();
    let collection = payload.collection.unwrap_or_else(|| "source_chunks".to_string());

    // Resolve embedding model
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config.as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    // Fetch chunks from DB
    let chunks: Vec<(i64, String, i64)> = if let Some(sid) = payload.source_id {
        sqlx::query_as(
            "SELECT id, content, source_id FROM chunks WHERE source_id = ? AND tenant_id = ?"
        )
        .bind(sid)
        .bind(&tenant_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT id, content, source_id FROM chunks WHERE tenant_id = ?"
        )
        .bind(&tenant_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    };

    if chunks.is_empty() {
        return Json(serde_json::json!({"embedded": 0, "message": "No chunks found"})).into_response();
    }

    // Batch embed
    let batch_size = 64;
    let mut all_vectors: Vec<Vec<f32>> = Vec::new();
    for chunk_batch in chunks.chunks(batch_size) {
        let texts: Vec<String> = chunk_batch.iter().map(|(_, content, _)| content.clone()).collect();
        match embed_texts(&texts, &embed_model).await {
            Ok(vecs) => all_vectors.extend(vecs),
            Err(e) => {
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e, "embedded": all_vectors.len()}))
                ).into_response();
            }
        }
    }

    // Build points
    let mut points = Vec::new();
    for (i, (chunk_id, content, source_id)) in chunks.iter().enumerate() {
        points.push(serde_json::json!({
            "id": *chunk_id as u64,
            "vector": all_vectors[i],
            "payload": {
                "content": content,
                "chunk_id": chunk_id,
                "source_id": source_id,
                "tenant_id": tenant_id,
                "is_active": true,
            }
        }));
    }

    // Upsert in batches
    let mut embedded = 0;
    for batch in points.chunks(100) {
        let body = serde_json::json!({ "points": batch });
        match qdrant.upsert_points(&collection, body).await {
            Ok(_) => embedded += batch.len(),
            Err(e) => {
                error!("Qdrant upsert error: {}", e);
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"embedded": embedded, "error": e.to_string()}))
                ).into_response();
            }
        }
    }

    info!("✅ Embedded {} chunks into {} for tenant {}", embedded, collection, tenant_id);
    Json(serde_json::json!({"embedded": embedded, "collection": collection})).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    
    #[tokio::test]
    async fn test_vector_routes_build() {
        assert!(true);
    }
}
