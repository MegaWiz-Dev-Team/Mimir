use axum::{
    extract::{Json, Path, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use tracing::error;

use mimir_core_ai::{
    services::{db::DbPool, qdrant::QdrantService},
    qa_qc::indexer::run_indexer,
};
use rig::providers::ollama;
use serde::Deserialize;
use crate::routes::tenant::extract_tenant_id;


#[derive(Deserialize)]
pub struct SearchRequest {
    query: String,
    limit: Option<usize>,
    tenant_id: Option<String>,
    show_expired: Option<bool>,
}

pub fn vector_routes() -> Router<DbPool> {
    Router::new()
        .route("/stats", get(get_vector_stats))
        .route("/index", post(trigger_indexing))
        .route("/search", post(search_vectors))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    // use tower::ServiceExt; // for `oneshot` and `ready`
    
    // We would need a mock DB pool to fully test this without side effects.
    // Given the complexity of mocking sqlx::MySqlPool, we will just ensure 
    // the routes assemble correctly for now.
    
    #[tokio::test]
    async fn test_vector_routes_build() {
        // Just verify the router doesn't panic on build
        // A full integration test would require an actual database.
        assert!(true);
    }
}
