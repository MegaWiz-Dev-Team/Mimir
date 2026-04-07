use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;
use super::rag_eval::RagEvalItem;

#[derive(Debug, Deserialize)]
pub struct CreateDatasetRequest {
    pub name: String,
    pub description: Option<String>,
    pub eval_set: Vec<RagEvalItem>,
}

#[derive(Debug, Deserialize)]
pub struct ListDatasetsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// POST /api/v1/rag-eval/datasets
pub async fn create_dataset(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<CreateDatasetRequest>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);
    let id = Uuid::new_v4().to_string();
    let eval_set_json = serde_json::to_string(&payload.eval_set).unwrap_or_default();

    match sqlx::query(
        "INSERT INTO rag_eval_datasets (id, tenant_id, name, description, eval_set) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&tenant_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&eval_set_json)
    .execute(&pool)
    .await
    {
        Ok(_) => {
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id,
                    "name": payload.name,
                    "items_count": payload.eval_set.len(),
                })),
            )
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to save dataset: {}", e) })),
            )
        }
    }
}

/// GET /api/v1/rag-eval/datasets
pub async fn list_datasets(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<ListDatasetsQuery>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let rows: Vec<(String, String, Option<String>, String, Option<chrono::NaiveDateTime>)> = sqlx::query_as(
        "SELECT id, name, description, eval_set, created_at FROM rag_eval_datasets WHERE tenant_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?"
    )
    .bind(&tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let datasets: Vec<Value> = rows.into_iter().map(|(id, name, desc, eval_set_json, created_at)| {
        let items_len = serde_json::from_str::<Vec<Value>>(&eval_set_json).map(|v| v.len()).unwrap_or(0);
        json!({
            "id": id,
            "name": name,
            "description": desc,
            "items_count": items_len,
            "eval_set": serde_json::from_str::<Value>(&eval_set_json).unwrap_or_else(|_| json!([])),
            "created_at": created_at
        })
    }).collect();

    (
        StatusCode::OK,
        Json(json!({ "datasets": datasets, "page": page, "per_page": per_page })),
    )
}

/// DELETE /api/v1/rag-eval/datasets/:id
pub async fn delete_dataset(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);

    match sqlx::query("DELETE FROM rag_eval_datasets WHERE id = ? AND tenant_id = ?")
        .bind(&id)
        .bind(&tenant_id)
        .execute(&pool)
        .await
    {
        Ok(_) => {
            (StatusCode::OK, Json(json!({ "message": "Dataset deleted successfully" })))
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to delete dataset: {}", e) })),
            )
        }
    }
}
