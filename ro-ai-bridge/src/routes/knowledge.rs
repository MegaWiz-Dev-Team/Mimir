//! Knowledge Routes — Collection-specific semantic search endpoints
//!
//! - POST /api/v1/knowledge/pubmed — Search PubMed biomedical literature
//! - POST /api/v1/knowledge/primekg — Search PrimeKG medical entities
//! - POST /api/v1/knowledge/clinical — Search clinical guidelines & protocols
//! - POST /api/v1/knowledge/icd10 — Search ICD-10 medical codes

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

use crate::retrieval::qdrant::{QdrantRetriever, RetrievalResult};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::iam::IamService;
use mimir_core_ai::services::qdrant::QdrantService;

/// Request body for knowledge search endpoints
#[derive(Debug, Deserialize)]
pub struct KnowledgeSearchRequest {
    /// The search query string.
    pub query: String,
    /// Tenant ID override (falls back to header).
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Maximum number of results. Default: 10, Max: 50.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Response wrapper for search results
#[derive(Debug, Serialize)]
pub struct KnowledgeSearchResponse {
    pub results: Vec<RetrievalResult>,
    pub count: usize,
    pub collection: String,
}

/// Handler: POST /api/v1/knowledge/pubmed
async fn search_pubmed(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<KnowledgeSearchRequest>,
) -> impl IntoResponse {
    search_collection_handler(&pool, &headers, &payload, "pubmed-abstracts").await
}

/// Handler: POST /api/v1/knowledge/primekg
async fn search_primekg(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<KnowledgeSearchRequest>,
) -> impl IntoResponse {
    search_collection_handler(&pool, &headers, &payload, "primekg-entities").await
}

/// Handler: POST /api/v1/knowledge/clinical
async fn search_clinical(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<KnowledgeSearchRequest>,
) -> impl IntoResponse {
    search_collection_handler(&pool, &headers, &payload, "clinical-wisdom").await
}

/// Handler: POST /api/v1/knowledge/icd10
async fn search_icd10(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<KnowledgeSearchRequest>,
) -> impl IntoResponse {
    search_collection_handler(&pool, &headers, &payload, "icd10-th").await
}

/// Internal handler for all collection searches
async fn search_collection_handler(
    pool: &DbPool,
    headers: &HeaderMap,
    payload: &KnowledgeSearchRequest,
    collection: &str,
) -> impl IntoResponse {
    let tenant_id = payload
        .tenant_id
        .as_deref()
        .unwrap_or_else(|| extract_tenant_id(headers))
        .to_string();

    let limit = payload.limit.unwrap_or(10).min(50);

    info!(
        event = "knowledge_search",
        query = %payload.query,
        collection = %collection,
        tenant = %tenant_id,
        limit = limit,
        "🔍 Knowledge search"
    );

    // Resolve embedding model from tenant config
    let iam = IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    // Create retriever for the specified collection
    let qdrant = QdrantService::new();
    let retriever = QdrantRetriever::new(qdrant, embed_model, collection.to_string());

    // Perform search with default parameters (alpha=0.7, threshold=0.0)
    match retriever
        .search_filtered(
            &payload.query,
            &tenant_id,
            limit,
            None,  // no source_ids filter
            0.7,   // alpha
            0.0,   // threshold
        )
        .await
    {
        Ok(results) => {
            let count = results.len();
            (
                StatusCode::OK,
                Json(KnowledgeSearchResponse {
                    results,
                    count,
                    collection: collection.to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                collection = %collection,
                tenant = %tenant_id,
                "Knowledge search failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Search failed: {}", e),
                    "collection": collection
                })),
            )
                .into_response()
        }
    }
}

pub fn knowledge_routes() -> Router<DbPool> {
    Router::new()
        .route("/pubmed", post(search_pubmed))
        .route("/primekg", post(search_primekg))
        .route("/clinical", post(search_clinical))
        .route("/icd10", post(search_icd10))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_routes_exist() {
        let router = knowledge_routes();
        // Just verify the router builds without panic
        let _ = router;
    }
}
