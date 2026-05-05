//! Global app settings — key/value pairs editable from the dashboard.
//!
//! Currently used for:
//!   - `auto_tune_model`  — Gemini model for prompt/parameter optimization
//!   - `judge_model`      — LLM-as-judge model for evaluation scoring

use axum::{
    extract::{Path, State},
    routing::{get, patch},
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct AppSetting {
    pub setting_key: String,
    pub setting_value: String,
    pub description: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingRequest {
    pub value: String,
}

pub fn app_settings_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(list_settings))
        .route("/{key}", get(get_setting).patch(update_setting))
}

async fn list_settings(State(pool): State<DbPool>) -> Json<Vec<AppSetting>> {
    let rows = sqlx::query_as::<_, AppSetting>(
        "SELECT setting_key, setting_value, description, updated_at FROM app_settings ORDER BY setting_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "list_settings_failed", error = %e);
        vec![]
    });
    Json(rows)
}

async fn get_setting(State(pool): State<DbPool>, Path(key): Path<String>) -> Json<Option<AppSetting>> {
    let row = sqlx::query_as::<_, AppSetting>(
        "SELECT setting_key, setting_value, description, updated_at FROM app_settings WHERE setting_key = ?",
    )
    .bind(&key)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    Json(row)
}

async fn update_setting(
    State(pool): State<DbPool>,
    Path(key): Path<String>,
    Json(req): Json<UpdateSettingRequest>,
) -> Json<serde_json::Value> {
    let res = sqlx::query(
        "INSERT INTO app_settings (setting_key, setting_value)
         VALUES (?, ?)
         ON DUPLICATE KEY UPDATE setting_value = VALUES(setting_value)",
    )
    .bind(&key)
    .bind(&req.value)
    .execute(&pool)
    .await;

    match res {
        Ok(_) => Json(serde_json::json!({"status": "ok", "key": key, "value": req.value})),
        Err(e) => {
            tracing::error!(event = "update_setting_failed", key = %key, error = %e);
            Json(serde_json::json!({"error": e.to_string()}))
        }
    }
}

/// Helper to fetch a setting value with env var fallback.
pub async fn get_setting_value(pool: &DbPool, key: &str, env_fallback: &str) -> String {
    let row: Option<(String,)> = sqlx::query_as("SELECT setting_value FROM app_settings WHERE setting_key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    row.map(|(v,)| v)
        .or_else(|| std::env::var(env_fallback).ok())
        .unwrap_or_default()
}
