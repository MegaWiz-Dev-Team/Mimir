use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use mimir_core_ai::services::db::DbPool;

// ── Models ────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TenantDocument {
    pub id: i64,
    pub tenant_id: String,
    pub title: String,
    pub source: Option<String>,
    pub tree_index: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub title: String,
    pub content: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub document_id: i64,
    pub tenant_id: String,
    pub title: String,
    pub tree_node_count: i64,
    pub status: String,
}

// ── Routes ────────────────────────────────────────────

pub fn ingest_routes() -> Router<DbPool> {
    Router::new()
        .route("/", post(ingest_document))
        .route("/documents", get(list_documents))
        .route("/documents/{doc_id}", delete(delete_document))
}

// ── Handlers ──────────────────────────────────────────

/// POST /api/v1/tenants/:id/ingest — Ingest markdown doc into tenant
async fn ingest_document(
    State(pool): State<DbPool>,
    Path(tenant_id): Path<String>,
    Json(req): Json<IngestRequest>,
) -> Result<(StatusCode, Json<IngestResponse>), (StatusCode, Json<Value>)> {
    // Ensure table exists
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tenant_documents (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            tenant_id VARCHAR(64) NOT NULL,
            title VARCHAR(255) NOT NULL,
            content MEDIUMTEXT,
            source VARCHAR(64),
            tree_index JSON,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

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

    // Call PageIndex sidecar to build tree
    let pageindex_url =
        std::env::var("PAGEINDEX_URL").unwrap_or_else(|_| "http://localhost:8600".to_string());

    let tree_index = match build_tree_index(&pageindex_url, &req.title, &req.content).await {
        Ok(tree) => tree,
        Err(e) => {
            tracing::warn!("PageIndex sidecar unavailable, using simple tree: {}", e);
            // Fallback: build a simple heading-based index inline
            build_simple_tree(&req.content, &req.title)
        }
    };

    let tree_json = serde_json::to_string(&tree_index).unwrap_or_default();
    let node_count = count_nodes(&tree_index);

    // Insert document with tree index
    let result = sqlx::query(
        "INSERT INTO tenant_documents (tenant_id, title, content, source, tree_index) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&tenant_id)
    .bind(&req.title)
    .bind(&req.content)
    .bind(&req.source)
    .bind(&tree_json)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let doc_id = result.last_insert_id() as i64;

    Ok((
        StatusCode::OK,
        Json(IngestResponse {
            document_id: doc_id,
            tenant_id,
            title: req.title,
            tree_node_count: node_count,
            status: "indexed".to_string(),
        }),
    ))
}

/// GET /api/v1/tenants/:id/ingest/documents — List tenant documents
async fn list_documents(
    State(pool): State<DbPool>,
    Path(tenant_id): Path<String>,
) -> Result<Json<Vec<TenantDocument>>, (StatusCode, Json<Value>)> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tenant_documents (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            tenant_id VARCHAR(64) NOT NULL,
            title VARCHAR(255) NOT NULL,
            content MEDIUMTEXT,
            source VARCHAR(64),
            tree_index JSON,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .ok();

    let docs: Vec<TenantDocument> = sqlx::query_as(
        "SELECT id, tenant_id, title, source, CAST(tree_index AS CHAR) as tree_index FROM tenant_documents WHERE tenant_id = ? ORDER BY created_at DESC"
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

    Ok(Json(docs))
}

/// DELETE /api/v1/tenants/:id/ingest/documents/:doc_id — Delete document
async fn delete_document(
    State(pool): State<DbPool>,
    Path((tenant_id, doc_id)): Path<(String, i64)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let result = sqlx::query("DELETE FROM tenant_documents WHERE id = ? AND tenant_id = ?")
        .bind(doc_id)
        .bind(&tenant_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Document not found"})),
        ));
    }

    Ok(Json(json!({"deleted": doc_id})))
}

// ── PageIndex Sidecar Client ──────────────────────────

async fn build_tree_index(base_url: &str, title: &str, content: &str) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/build-tree", base_url))
        .json(&json!({
            "content": content,
            "title": title,
        }))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("PageIndex returned {}", resp.status()));
    }

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.get("tree_index").cloned().unwrap_or(body))
}

fn build_simple_tree(content: &str, title: &str) -> Value {
    let mut nodes = Vec::new();
    let mut current_heading = String::new();
    let mut node_id = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim().to_string();
            if !heading.is_empty() {
                nodes.push(json!({
                    "title": heading,
                    "node_id": format!("{:04}", node_id),
                    "summary": "",
                }));
                node_id += 1;
                current_heading = heading;
            }
        }
    }

    json!({
        "title": title,
        "nodes": nodes,
    })
}

fn count_nodes(tree: &Value) -> i64 {
    let mut count = 0i64;
    if let Some(nodes) = tree.get("nodes").and_then(|n| n.as_array()) {
        count += nodes.len() as i64;
        for node in nodes {
            count += count_nodes(node);
        }
    }
    count
}
