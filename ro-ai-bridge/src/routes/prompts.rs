//! Prompt Management API — CRUD for extraction prompts
//!
//! GET    /api/v1/prompts                — list all prompts
//! POST   /api/v1/prompts                — create new prompt version
//! PUT    /api/v1/prompts/:id/activate   — set as active
//! GET    /api/v1/prompts/:name/active   — get active prompt for a name

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExtractionPrompt {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub prompt_text: String,
    pub is_active: bool,
    pub tenant_id: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePromptRequest {
    pub name: String,
    pub version: String,
    pub prompt_text: String,
    pub notes: Option<String>,
    pub set_active: Option<bool>,
}

pub fn prompts_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_prompts).post(create_prompt))
        .route("/{id}/activate", put(activate_prompt))
        .route("/{name}/active", get(get_active_prompt))
}

/// GET /api/v1/prompts — List all prompts (global + tenant-specific)
async fn list_prompts(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Vec<ExtractionPrompt>>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let prompts = sqlx::query_as::<_, ExtractionPrompt>(
        "SELECT id, name, version, prompt_text, is_active, tenant_id, notes, created_at 
         FROM extraction_prompts 
         WHERE tenant_id IS NULL OR tenant_id = ? 
         ORDER BY name, version DESC",
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok(Json(prompts))
}

/// POST /api/v1/prompts — Create a new prompt version
async fn create_prompt(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(req): Json<CreatePromptRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    let result = sqlx::query(
        "INSERT INTO extraction_prompts (name, version, prompt_text, tenant_id, notes) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&req.name)
    .bind(&req.version)
    .bind(&req.prompt_text)
    .bind(&tenant_id)
    .bind(&req.notes)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let new_id = result.last_insert_id() as i32;

    // Optionally set as active
    if req.set_active.unwrap_or(false) {
        // Deactivate others with same name for this tenant
        let _ = sqlx::query(
            "UPDATE extraction_prompts SET is_active = FALSE WHERE name = ? AND (tenant_id = ? OR tenant_id IS NULL)"
        )
        .bind(&req.name)
        .bind(&tenant_id)
        .execute(&pool)
        .await;

        let _ = sqlx::query("UPDATE extraction_prompts SET is_active = TRUE WHERE id = ?")
            .bind(new_id)
            .execute(&pool)
            .await;
    }

    info!(
        "Created prompt: {} {} (id: {})",
        req.name, req.version, new_id
    );

    Ok(Json(json!({
        "id": new_id,
        "name": req.name,
        "version": req.version,
        "is_active": req.set_active.unwrap_or(false)
    })))
}

/// PUT /api/v1/prompts/:id/activate — Set a prompt as active
async fn activate_prompt(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    // Get the prompt to find its name
    let prompt: Option<(String,)> =
        sqlx::query_as("SELECT name FROM extraction_prompts WHERE id = ?")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
            })?;

    let (name,) = prompt.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Prompt not found"})),
        )
    })?;

    // Deactivate all with same name, then activate this one
    let _ = sqlx::query(
        "UPDATE extraction_prompts SET is_active = FALSE WHERE name = ? AND (tenant_id = ? OR tenant_id IS NULL)"
    )
    .bind(&name)
    .bind(&tenant_id)
    .execute(&pool)
    .await;

    let _ = sqlx::query("UPDATE extraction_prompts SET is_active = TRUE WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;

    info!("Activated prompt: {} (id: {})", name, id);

    Ok(Json(json!({"success": true, "id": id, "name": name})))
}

/// GET /api/v1/prompts/:name/active — Get the active prompt for a given name
async fn get_active_prompt(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Priority: tenant-specific active > global active
    let prompt: Option<ExtractionPrompt> = sqlx::query_as(
        "SELECT id, name, version, prompt_text, is_active, tenant_id, notes, created_at 
         FROM extraction_prompts 
         WHERE name = ? AND is_active = TRUE AND (tenant_id = ? OR tenant_id IS NULL)
         ORDER BY tenant_id DESC 
         LIMIT 1",
    )
    .bind(&name)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    match prompt {
        Some(p) => Ok(Json(json!({
            "id": p.id,
            "name": p.name,
            "version": p.version,
            "prompt_text": p.prompt_text,
            "tenant_id": p.tenant_id,
        }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("No active prompt found for '{}'", name)})),
        )),
    }
}
