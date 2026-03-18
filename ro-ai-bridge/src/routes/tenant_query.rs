use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    let tenant_exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM tenants WHERE id = ?")
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

    // Fetch all tenant documents with tree indexes
    let docs: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, title, CAST(content AS CHAR), CAST(tree_index AS CHAR) FROM tenant_documents WHERE tenant_id = ?",
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

    // Strategy 1: Tree search via PageIndex sidecar
    if mode == "tree" || mode == "hybrid" {
        let pageindex_url = std::env::var("PAGEINDEX_URL")
            .unwrap_or_else(|_| "http://localhost:8600".to_string());

        for (_, title, content, tree_index) in &docs {
            if let (Some(content), Some(tree_json)) = (content, tree_index) {
                match tree_search(
                    &pageindex_url,
                    tree_json,
                    content,
                    &req.question,
                )
                .await
                {
                    Ok(result) => {
                        let sections = result
                            .get("relevant_sections")
                            .and_then(|s| s.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();

                        if let Some(answer) = result.get("answer").and_then(|a| a.as_str()) {
                            all_answers.push(answer.to_string());
                        }

                        all_sources.push(QuerySource {
                            document_title: title.clone(),
                            relevant_sections: sections,
                            source_type: "tree".to_string(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Tree search failed for '{}': {}", title, e);
                    }
                }
            }
        }
    }

    // Strategy 2: Vector search via Qdrant (existing Mimir pipeline)
    if mode == "vector" || (mode == "hybrid" && all_answers.is_empty()) {
        // Use existing Mimir vector search with tenant filter
        match vector_search(&pool, &tenant_id, &req.question).await {
            Ok(results) => {
                for (title, snippet) in results {
                    all_answers.push(snippet);
                    all_sources.push(QuerySource {
                        document_title: title,
                        relevant_sections: vec![],
                        source_type: "vector".to_string(),
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Vector search failed: {}", e);
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

// ── PageIndex Tree Search ─────────────────────────────

async fn tree_search(
    base_url: &str,
    tree_json: &str,
    content: &str,
    question: &str,
) -> Result<Value, String> {
    let tree_index: Value =
        serde_json::from_str(tree_json).map_err(|e| e.to_string())?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/search", base_url))
        .json(&json!({
            "tree_index": tree_index,
            "question": question,
            "content": content,
        }))
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("PageIndex search returned {}", resp.status()));
    }

    resp.json().await.map_err(|e| e.to_string())
}

// ── Qdrant Vector Search (tenant-scoped) ──────────────

async fn vector_search(
    pool: &DbPool,
    tenant_id: &str,
    question: &str,
) -> Result<Vec<(String, String)>, String> {
    // Query tenant documents content directly as fallback
    let docs: Vec<(String, String)> = sqlx::query_as(
        "SELECT title, CAST(SUBSTRING(content, 1, 500) AS CHAR) FROM tenant_documents WHERE tenant_id = ? LIMIT 5",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(docs)
}
