
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
    pub difficulty: Option<String>,
    pub question_type: Option<String>,
    pub qc_status: Option<String>,
}

fn jaccard_similarity(s1: &str, s2: &str) -> f64 {
    let s1_lower = s1.to_lowercase();
    let s2_lower = s2.to_lowercase();
    let tokens1: std::collections::HashSet<&str> = s1_lower.split_whitespace().collect();
    let tokens2: std::collections::HashSet<&str> = s2_lower.split_whitespace().collect();
    let intersection = tokens1.intersection(&tokens2).count() as f64;
    let union_size = tokens1.union(&tokens2).count() as f64;
    if union_size == 0.0 { 0.0 } else { intersection / union_size }
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
) -> axum::response::Response {
    let tenant_id = extract_tenant_id(&headers);

    // Deduplication Check (Task T3.4)
    let mut warnings = Vec::new();
    for i in 0..payload.eval_set.len() {
        for j in (i + 1)..payload.eval_set.len() {
            let sim = jaccard_similarity(&payload.eval_set[i].query, &payload.eval_set[j].query);
            if sim > 0.85 {
                warnings.push(format!(
                    "High semantic similarity ({:.2}) detected between queries: '{}' and '{}'",
                    sim, payload.eval_set[i].query, payload.eval_set[j].query
                ));
            }
        }
    }
    if !warnings.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Dataset validation failed due to duplicate queries", "warnings": warnings })),
        ).into_response();
    }

    let id = Uuid::new_v4().to_string();
    let eval_set_json = serde_json::to_string(&payload.eval_set).unwrap_or_default();
    let qc_status = payload.qc_status.unwrap_or_else(|| "Draft".to_string());

    // Query for latest version
    let version_row: (Option<i32>,) = sqlx::query_as(
        "SELECT MAX(version) FROM rag_eval_datasets WHERE name = ? AND tenant_id = ?"
    )
    .bind(&payload.name)
    .bind(&tenant_id)
    .fetch_one(&pool)
    .await
    .unwrap_or((Some(0),));

    let next_version = version_row.0.unwrap_or(0) + 1;

    match sqlx::query(
        "INSERT INTO rag_eval_datasets (id, tenant_id, name, description, eval_set, version, difficulty, question_type, qc_status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&tenant_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&eval_set_json)
    .bind(&next_version)
    .bind(&payload.difficulty)
    .bind(&payload.question_type)
    .bind(&qc_status)
    .execute(&pool)
    .await
    {
        Ok(_) => {
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id,
                    "name": payload.name,
                    "version": next_version,
                    "items_count": payload.eval_set.len(),
                })),
            ).into_response()
        }
        Err(e) => {
            eprintln!("[RAG EVal DB] Failed to save dataset: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
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

    // Use sqlx::Row instead of index to cleanly get new nullable fields without panic
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT id, name, description, eval_set, created_at, version, difficulty, question_type, qc_status FROM rag_eval_datasets WHERE tenant_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?"
    )
    .bind(&tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let datasets: Vec<Value> = rows.into_iter().map(|row| {
        let id: String = row.get("id");
        let name: String = row.get("name");
        let desc: Option<String> = row.get("description");
        let eval_set_json: String = row.get("eval_set");
        let created_at: Option<chrono::NaiveDateTime> = row.get("created_at");
        let version: Option<i32> = row.try_get("version").unwrap_or(Some(1));
        let diff: Option<String> = row.try_get("difficulty").unwrap_or(None);
        let q_type: Option<String> = row.try_get("question_type").unwrap_or(None);
        let qc_status: Option<String> = row.try_get("qc_status").unwrap_or(Some("Draft".into()));

        let items_len = serde_json::from_str::<Vec<Value>>(&eval_set_json).map(|v| v.len()).unwrap_or(0);
        json!({
            "id": id,
            "name": name,
            "description": desc,
            "items_count": items_len,
            "version": version.unwrap_or(1),
            "difficulty": diff,
            "question_type": q_type,
            "qc_status": qc_status,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_dataset_request_deserialization() {
        let json = r#"{
            "name": "Cardiology Basics",
            "difficulty": "Easy",
            "question_type": "clinical",
            "eval_set": []
        }"#;

        let req: CreateDatasetRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Cardiology Basics");
        assert_eq!(req.difficulty, Some("Easy".to_string()));
        assert_eq!(req.question_type, Some("clinical".to_string()));
    }
}
