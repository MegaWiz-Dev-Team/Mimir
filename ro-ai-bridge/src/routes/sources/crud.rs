//! CRUD operations for data sources: list, create, update, delete.

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use mimir_core_ai::models::sources::{
    CreateDataSourceRequest, DataSource, UpdateDataSourceRequest,
};
use mimir_core_ai::services::db::DbPool;
use serde_json::{json, Value};

pub(crate) async fn list_sources(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<DataSource>>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let sources = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE tenant_id = ?")
        .bind(tenant_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    Ok(Json(sources))
}

pub(crate) async fn create_source(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<CreateDataSourceRequest>,
) -> Result<(StatusCode, Json<DataSource>), (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let result = sqlx::query(
        "INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&tenant_id)
    .bind(&payload.name)
    .bind(&payload.source_type)
    .bind(&payload.config_json)
    .bind(&payload.schedule)
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let new_source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ?")
        .bind(result.last_insert_id())
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    Ok((StatusCode::CREATED, Json(new_source)))
}

pub(crate) async fn update_source(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDataSourceRequest>,
) -> Result<Json<DataSource>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Check if source exists
    let existing = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    if existing.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Source not found"})),
        ));
    }

    let current = existing.unwrap();
    let updated_name = payload.name.unwrap_or(current.name);
    let updated_config = payload.config_json.unwrap_or(current.config_json);
    let updated_schedule = payload.schedule.or(current.schedule);
    let updated_raw_markdown = payload.raw_markdown.clone().or(current.raw_markdown);
    
    let mut updated_status = current.last_sync_status.clone();
    if payload.raw_markdown.is_some() && current.last_sync_status.as_deref() == Some("FAILED") {
        updated_status = Some("COMPLETED".to_string());
    }

    sqlx::query(
        "UPDATE data_sources SET name = ?, config_json = ?, schedule = ?, raw_markdown = ?, last_sync_status = ? WHERE id = ? AND tenant_id = ?"
    )
    .bind(&updated_name)
    .bind(&updated_config)
    .bind(&updated_schedule)
    .bind(&updated_raw_markdown)
    .bind(&updated_status)
    .bind(id)
    .bind(&tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let updated_source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    Ok(Json(updated_source))
}

pub(crate) async fn delete_source(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let result = sqlx::query("DELETE FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
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
            Json(json!({"error": "Source not found or access denied"})),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}
