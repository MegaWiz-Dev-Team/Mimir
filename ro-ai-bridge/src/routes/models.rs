use axum::{extract::State, routing::{get, post}, Json, Router};
use mimir_core_ai::services::db::{get_active_llm_models, DbPool};
use tracing::{error, info};
use serde::{Deserialize, Serialize};

pub fn models_routes() -> Router<DbPool> {
    Router::new()
        .route("/models", get(list_models))
        .route("/config/models/pull", post(pull_model))
}

async fn list_models(
    State(pool): State<DbPool>,
) -> Result<Json<Vec<mimir_core_ai::models::model_config::ModelConfig>>, axum::http::StatusCode> {
    match get_active_llm_models(&pool).await {
        Ok(models) => Ok(Json(models)),
        Err(e) => {
            error!("Failed to fetch active LLM models: {:?}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
pub struct PullModelRequest {
    pub provider: String,
    pub model_id: String,
}

#[derive(Serialize)]
pub struct PullModelResponse {
    pub status: String,
    pub message: String,
}

async fn pull_model(
    State(pool): State<DbPool>,
    Json(payload): Json<PullModelRequest>,
) -> Result<Json<PullModelResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    info!("Received model pull request: Provider={} Model={}", payload.provider, payload.model_id);

    // Call downstream Provider
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(3600)).build().unwrap();

    let success = match payload.provider.as_str() {
        "ollama" => { // Usually bound inside K3s/Orbstack networking
            let ollama_url = std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://host.k3d.internal:11434".to_string());
            let pull_req = serde_json::json!({ "name": payload.model_id, "stream": false });
            let res = client.post(&format!("{}/api/pull", ollama_url)).json(&pull_req).send().await;
            res.is_ok() && res.unwrap().status().is_success()
        },
        "heimdall" => { // The new Host Daemon
            let heimdall_url = std::env::var("HEIMDALL_DAEMON_URL").unwrap_or_else(|_| "http://host.k3d.internal:3009".to_string());
            let pull_req = serde_json::json!({ "model": payload.model_id });
            let res = client.post(&format!("{}/pull", heimdall_url)).json(&pull_req).send().await;
            res.is_ok() && res.unwrap().status().is_success()
        },
        _ => return Err((axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Unsupported provider"})))),
    };

    if !success {
        return Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to pull model from provider daemon"}))));
    }

    // Insert into DB if successful
    let query = "INSERT INTO ai_models (model_id, provider, model_type, is_active, capabilities) VALUES (?, ?, 'llm', true, '{\"reasoning\":true,\"tools\":true,\"vision\":false}') ON DUPLICATE KEY UPDATE is_active = true";
    
    // We unwrap connection internally
    let mut conn = pool.acquire().await.map_err(|e| {
        error!("DB acquire error: {:?}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database connection error"})))
    })?;

    sqlx::query(query)
        .bind(&payload.model_id)
        .bind(&payload.provider)
        .execute(&mut *conn)
        .await
        .map_err(|e| {
            error!("DB insert error: {:?}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to register model in database"})))
        })?;

    Ok(Json(PullModelResponse {
        status: "success".into(),
        message: format!("Successfully pulled and registered {}", payload.model_id),
    }))
}
