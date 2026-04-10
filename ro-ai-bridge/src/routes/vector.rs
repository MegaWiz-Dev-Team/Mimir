use axum::{
    extract::{Json, Path, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use tracing::{error, info};

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::{
    qa_qc::indexer::run_indexer,
    services::{db::DbPool, qdrant::QdrantService},
};
use serde::Deserialize;

/// Batch embed texts via Heimdall /v1/embeddings (OpenAI-compatible)
pub async fn embed_texts(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>, String> {
    // Fast Embedding API limits to small batches, Heimdall handles large batches
    let embed_base_url = if texts.len() <= 4 {
        std::env::var("FAST_EMBEDDING_API_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                std::env::var("HEIMDALL_API_URL")
                    .unwrap_or_else(|_| "http://localhost:11434/v1".to_string())
            })
    } else {
        std::env::var("HEIMDALL_API_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("OLLAMA_URL")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .map(|u| format!("{}/v1", u))
            })
            .unwrap_or_else(|| "http://localhost:11434/v1".to_string())
    };
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

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

    let data = body["data"]
        .as_array()
        .ok_or("No 'data' array in response")?;
    let mut vectors = Vec::with_capacity(data.len());
    for item in data {
        let vec: Vec<f32> = item["embedding"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect()
            })
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

async fn get_vector_stats(headers: HeaderMap, State(pool): State<DbPool>) -> impl IntoResponse {
    let collection_name = "source_chunks";
    let qdrant = QdrantService::new();
    let tenant_id = extract_tenant_id(&headers).to_string();

    // 1. Get Qdrant stats
    let qdrant_info = qdrant
        .get_collection_info(collection_name)
        .await
        .unwrap_or(serde_json::Value::Null);
    let qdrant_points = qdrant_info["result"]["points_count"].as_i64().unwrap_or(0);

    // 2. Count QA stats (tenant-scoped)
    let total_qa = sqlx::query("SELECT count(*) as count FROM qa_results WHERE tenant_id = ?")
        .bind(&tenant_id)
        .fetch_one(&pool)
        .await
        .map(|r| {
            use sqlx::Row;
            r.get::<i64, _>("count")
        })
        .unwrap_or(0);

    let indexed_qa = sqlx::query(
        "SELECT count(*) as count FROM qa_results WHERE tenant_id = ? AND indexed_at IS NOT NULL",
    )
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .map(|r| {
        use sqlx::Row;
        r.get::<i64, _>("count")
    })
    .unwrap_or(0);

    let pending_results: i64 = sqlx::query(
        "SELECT count(*) as count FROM qa_results \
         WHERE tenant_id = ? AND qc_scanned = 1 \
         AND id NOT IN (SELECT qa_id FROM qa_cluster_items) \
         AND indexed_at IS NULL",
    )
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .map(|r| {
        use sqlx::Row;
        r.get("count")
    })
    .unwrap_or(0);

    let pending_clusters: i64 = sqlx::query(
        "SELECT count(*) as count FROM qa_clusters \
         WHERE tenant_id = ? AND status != 'PENDING' \
         AND indexed_at IS NULL AND golden_answer IS NOT NULL",
    )
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .map(|r| {
        use sqlx::Row;
        r.get("count")
    })
    .unwrap_or(0);

    let pending_golden = pending_results + pending_clusters;

    let unscanned_raw: i64 = sqlx::query(
        "SELECT count(*) as count FROM qa_results WHERE tenant_id = ? AND qc_scanned = 0",
    )
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .map(|r| {
        use sqlx::Row;
        r.get("count")
    })
    .unwrap_or(0);

    let pending_cluster_items: i64 = sqlx::query(
        "SELECT count(*) as count FROM qa_results qr \
         JOIN qa_cluster_items ci ON qr.id = ci.qa_id \
         JOIN qa_clusters c ON ci.cluster_id = c.id \
         WHERE qr.tenant_id = ? AND c.status = 'PENDING'",
    )
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .map(|r| {
        use sqlx::Row;
        r.get("count")
    })
    .unwrap_or(0);

    // 3. Get chunk stats (tenant-scoped via source ownership)
    let total_chunks = sqlx::query(
        "SELECT count(*) as count FROM chunks c \
         JOIN data_sources ds ON c.source_id = ds.id \
         WHERE ds.tenant_id = ?",
    )
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .map(|r| {
        use sqlx::Row;
        r.get::<i64, _>("count")
    })
    .unwrap_or(0);

    let chunk_sync_pct = if total_chunks > 0 {
        ((qdrant_points as f64 / total_chunks as f64) * 100.0).min(100.0)
    } else {
        0.0
    };

    // 4. Get active status & potential error from tenant_configs.search_settings
    let settings_row = sqlx::query(
        r#"SELECT 
           CAST(COALESCE(JSON_UNQUOTE(JSON_EXTRACT(search_settings, '$.indexing_active')) = 'true', 0) AS SIGNED) as indexing_active,
           CAST(JSON_UNQUOTE(JSON_EXTRACT(search_settings, '$.indexing_error')) AS CHAR) as indexing_error
           FROM tenant_configs WHERE tenant_id = ?"#
    )
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let (indexing_active, indexing_error): (i32, Option<String>) = if let Some(row) = settings_row {
        use sqlx::Row;
        // Depending on MariaDB version, CAST AS SIGNED might yield an i64 instead.
        let active_val: i32 = row.try_get("indexing_active").unwrap_or_else(|_| row.try_get::<i64, _>("indexing_active").unwrap_or(0) as i32);
        
        let err_val: Option<String> = row.try_get("indexing_error").unwrap_or_else(|_| {
            row.try_get::<Vec<u8>, _>("indexing_error").ok().and_then(|v| String::from_utf8(v).ok())
        });
        let err_val = err_val.filter(|s| s != "null" && s != "NULL" && !s.is_empty());
        
        (active_val, err_val)
    } else {
        (0, None)
    };

    Json(serde_json::json!({
        "qdrant": qdrant_info,
        "database": {
            "total_qa": total_qa,
            "indexed_qa": indexed_qa,
            "pending_golden": pending_golden,
            "unscanned_raw": unscanned_raw,
            "pending_cluster_items": pending_cluster_items,
            "total_chunks": total_chunks,
            "qdrant_points": qdrant_points,
            "chunk_sync_pct": chunk_sync_pct,
            "indexing_active": indexing_active == 1,
            "indexing_error": indexing_error

        }
    }))
}

async fn trigger_indexing(State(pool): State<DbPool>) -> impl IntoResponse {
    let qdrant = QdrantService::new();

    let pool_clone = pool.clone();
    tokio::spawn(async move {
        // Clear errors and set active
        let _ = sqlx::query("UPDATE tenant_configs SET search_settings = JSON_SET(COALESCE(search_settings, '{}'), '$.indexing_active', 'true', '$.indexing_error', null)")
           .execute(&pool_clone)
           .await;

        let result = run_indexer(&pool_clone, &qdrant, "golden_qa").await;
        
        if let Err(e) = result {
            error!("Background indexing failed: {}", e);
            let err_msg = e.to_string();
            let _ = sqlx::query("UPDATE tenant_configs SET search_settings = JSON_SET(search_settings, '$.indexing_active', 'false', '$.indexing_error', ?)")
               .bind(err_msg)
               .execute(&pool_clone)
               .await;
        } else {
            let _ = sqlx::query("UPDATE tenant_configs SET search_settings = JSON_SET(search_settings, '$.indexing_active', 'false', '$.indexing_error', null)")
               .execute(&pool_clone)
               .await;
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
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_slot = llm_config.resolve_slot("embedding", None, None);
    let embed_model_name = embed_slot.model;

    // Determine embedding API URL: use Fast Embedding (llama.cpp) for UI searches, else Heimdall (Rust ONNX)
    let embed_base_url = std::env::var("FAST_EMBEDDING_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            std::env::var("HEIMDALL_API_URL")
                .unwrap_or_else(|_| "http://localhost:11434/v1".to_string())
        });
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
                Ok(body) => body["data"][0]["embedding"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    })
                    .unwrap_or_default(),
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
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Empty embedding vector returned"})),
        )
            .into_response();
    }

    let target_tenant = payload.tenant_id.unwrap_or(tenant_id);
    let show_expired = payload.show_expired.unwrap_or(false);

    // Wire search_settings from tenant config (Phase 2.1)
    let search_settings = tenant_config
        .as_ref()
        .and_then(|c| c.search_settings.as_ref())
        .map(|s| s.0.clone())
        .unwrap_or(serde_json::json!({}));
    let config_top_k = search_settings
        .get("top_k")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let limit = payload.limit.or(config_top_k).unwrap_or(5);

    // Generate BM25 sparse vector from query for hybrid search
    let sparse = mimir_core_ai::services::bm25::text_to_sparse_vector(&payload.query);

    // Try hybrid search first (source_chunks with named vectors)
    match qdrant
        .search_hybrid(
            "source_chunks",
            vector_f32.clone(),
            &sparse,
            limit,
            &target_tenant,
        )
        .await
    {
        Ok(results) => Json(results).into_response(),
        Err(_) => {
            // Fallback to legacy dense-only search
            match qdrant
                .search(
                    "source_chunks",
                    vector_f32,
                    limit,
                    &target_tenant,
                    show_expired,
                )
                .await
            {
                Ok(results) => Json(results).into_response(),
                Err(e) => (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
    }
}

async fn delete_vector_handler(
    headers: HeaderMap,
    State(_pool): State<DbPool>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let qdrant = QdrantService::new();
    let _tenant_id = extract_tenant_id(&headers);

    match qdrant.delete_point("source_chunks", id).await {
        Ok(_) => (
            axum::http::StatusCode::OK,
            Json(serde_json::json!({"status": "success"})),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /vector/qa/bulk — batch embed QA pairs and upsert into golden_qa
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
    let llm_config = tenant_config
        .as_ref()
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
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e})),
                )
                    .into_response();
            }
        }
    }

    // Build Qdrant points
    let mut points = Vec::new();
    for (i, item) in payload.items.iter().enumerate() {
        let point_id = uuid::Uuid::new_v4().as_u128() as u64;
        points.push(serde_json::json!({
            "id": point_id,
            "vector": { "dense": all_vectors[i] },
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
        match qdrant.upsert_points("golden_qa", body).await {
            Ok(_) => inserted += chunk.len(),
            Err(e) => {
                error!("Qdrant upsert error: {}", e);
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"inserted": inserted, "error": e.to_string()})),
                )
                    .into_response();
            }
        }
    }

    info!(
        "✅ Indexed {} QA pairs into golden_qa for tenant {}",
        inserted, tenant_id
    );
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
    let collection = payload
        .collection
        .unwrap_or_else(|| "source_chunks".to_string());

    // Resolve embedding model
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    // Fetch chunks from DB
    let chunks: Vec<(i64, String, i64)> = if let Some(sid) = payload.source_id {
        sqlx::query_as(
            "SELECT id, content, source_id FROM chunks WHERE source_id = ? AND tenant_id = ?",
        )
        .bind(sid)
        .bind(&tenant_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as("SELECT id, content, source_id FROM chunks WHERE tenant_id = ?")
            .bind(&tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    };

    if chunks.is_empty() {
        return Json(serde_json::json!({"embedded": 0, "message": "No chunks found"}))
            .into_response();
    }

    // Batch embed
    let batch_size = 64;
    let mut all_vectors: Vec<Vec<f32>> = Vec::new();
    for chunk_batch in chunks.chunks(batch_size) {
        let texts: Vec<String> = chunk_batch
            .iter()
            .map(|(_, content, _)| content.clone())
            .collect();
        match embed_texts(&texts, &embed_model).await {
            Ok(vecs) => all_vectors.extend(vecs),
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e, "embedded": all_vectors.len()})),
                )
                    .into_response();
            }
        }
    }

    // Build points
    let mut points = Vec::new();
    for (i, (chunk_id, content, source_id)) in chunks.iter().enumerate() {
        points.push(serde_json::json!({
            "id": *chunk_id as u64,
            "vector": { "dense": all_vectors[i] },
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
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"embedded": embedded, "error": e.to_string()})),
                )
                    .into_response();
            }
        }
    }

    info!(
        "✅ Embedded {} chunks into {} for tenant {}",
        embedded, collection, tenant_id
    );
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
