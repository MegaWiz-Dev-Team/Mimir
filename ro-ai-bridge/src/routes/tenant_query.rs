use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::retrieval::qdrant::{QdrantRetriever, VectorRetriever};
use mimir_core_ai::services::qdrant::QdrantService;

use mimir_core_ai::services::db::DbPool;

// ── Models ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TenantQueryRequest {
    pub question: String,
    pub mode: Option<String>, // "tree", "vector", "hybrid" (default)
}

#[derive(Debug, Serialize)]
pub struct TenantQueryResponse {
    pub answer: String,
    pub sources: Vec<QuerySource>,
    pub mode_used: String,
}

#[derive(Debug, Serialize)]
pub struct QuerySource {
    pub document_title: String,
    pub relevant_sections: Vec<String>,
    pub source_type: String, // "tree" or "vector"
}

// ── Routes ────────────────────────────────────────────

pub fn tenant_query_routes() -> Router<DbPool> {
    Router::new().route("/", post(query_tenant))
}

// ── Handler ───────────────────────────────────────────

/// POST /api/v1/tenants/:id/query — Hybrid RAG query per tenant
async fn query_tenant(
    State(pool): State<DbPool>,
    Path(tenant_id): Path<String>,
    Json(req): Json<TenantQueryRequest>,
) -> Result<Json<TenantQueryResponse>, (StatusCode, Json<Value>)> {
    let mode = req.mode.as_deref().unwrap_or("hybrid");

    // Verify tenant exists
    let tenant_exists: Option<(String,)> = sqlx::query_as("SELECT id FROM tenants WHERE id = ?")
        .bind(&tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if tenant_exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Tenant '{}' not found", tenant_id)})),
        ));
    }

    // Fetch all tenant data sources with tree indexes
    let docs: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, name, CAST(raw_markdown AS CHAR), CAST(pageindex_tree AS CHAR) FROM data_sources WHERE tenant_id = ?",
    )
    .bind(&tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    if docs.is_empty() {
        return Ok(Json(TenantQueryResponse {
            answer: format!("No documents found for tenant '{}'", tenant_id),
            sources: vec![],
            mode_used: mode.to_string(),
        }));
    }

    let mut all_sources = Vec::new();
    let mut all_answers = Vec::new();

    // Strategy 1: Native Tree search (LLM)
    if mode == "tree" || mode == "hybrid" {
        use mimir_core_ai::services::llm_router::LlmRouter;
        
        let router = match LlmRouter::new(pool.clone(), &tenant_id).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to init LlmRouter for tree search: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to init router: {}", e)})),
                ));
            }
        };

        let retriever = crate::retrieval::tree::NativeTreeRetriever::new();

        // Collect docs that have both content and tree_index
        let searchable_docs: Vec<(String, String, String)> = docs
            .iter()
            .filter_map(
                |(_, title, content, tree_index)| match (content, tree_index) {
                    (Some(c), Some(t)) => Some((title.clone(), c.clone(), t.clone())),
                    _ => None,
                },
            )
            .collect();

        if !searchable_docs.is_empty() {
            use crate::retrieval::tree::TreeRetriever;
            let tree_results = retriever
                .search_parallel(&router, &searchable_docs, &req.question)
                .await;

            for result in &tree_results {
                if let Some(ref answer) = result.answer {
                    all_answers.push(answer.clone());
                }
                all_sources.push(QuerySource {
                    document_title: result.document_title.clone(),
                    relevant_sections: result.relevant_sections.clone(),
                    source_type: "tree".to_string(),
                });
            }
        }
    }

    // Strategy 2: Vector search via Qdrant (real semantic search)
    if mode == "vector" || (mode == "hybrid" && all_answers.is_empty()) {
        // Resolve embedding model from tenant config
        let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
        let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
        let llm_config = tenant_config
            .as_ref()
            .and_then(|c| c.llm_config.as_ref())
            .map(|c| c.0.clone())
            .unwrap_or_default();
        let embed_model = llm_config.resolve_slot("embedding", None, None).model;

        // Search both golden_qa and source_chunks collections
        let collections = vec!["golden_qa", "source_chunks"];
        for collection in collections {
            let retriever = QdrantRetriever::new(
                QdrantService::new(),
                embed_model.clone(),
                collection.to_string(),
            );
            match retriever.search(&req.question, &tenant_id, 5).await {
                Ok(results) => {
                    for result in results {
                        all_answers.push(result.content.clone());
                        all_sources.push(QuerySource {
                            document_title: result.title,
                            relevant_sections: vec![result.content],
                            source_type: "vector".to_string(),
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Vector search failed on {}: {}", collection, e);
                }
            }
        }
    }

    let final_answer = if all_answers.is_empty() {
        format!(
            "I found {} documents for tenant '{}' but couldn't extract a specific answer. Documents: {}",
            docs.len(),
            tenant_id,
            docs.iter().map(|(_, t, _, _)| t.as_str()).collect::<Vec<_>>().join(", ")
        )
    } else {
        all_answers.join("\n\n")
    };

    let mode_used = if all_sources.iter().any(|s| s.source_type == "tree")
        && all_sources.iter().any(|s| s.source_type == "vector")
    {
        "hybrid"
    } else if all_sources.iter().any(|s| s.source_type == "tree") {
        "tree"
    } else {
        "vector"
    };

    Ok(Json(TenantQueryResponse {
        answer: final_answer,
        sources: all_sources,
        mode_used: mode_used.to_string(),
    }))
}

// ── Sprint 31 Refactoring Notes ───────────────────────
// - tree_search: Moved to retrieval::tree::PageIndexRetriever (parallel join_all)
// - vector_search: Moved to retrieval::qdrant::QdrantRetriever (real Qdrant)
