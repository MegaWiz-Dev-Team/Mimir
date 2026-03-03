//! Vault Secrets Management Routes (Issue #157)
//!
//! - GET  /api/v1/vault/status  — check Vault connectivity
//! - POST /api/v1/vault/rotate  — rotate a specific secret key

use axum::{
    Router,
    routing::{get, post},
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::MySqlPool;
use serde_json::json;
use tracing::{info, error};
use mimir_core_ai::services::vault::{
    self, RotateSecretRequest, VaultStatus,
};

pub fn vault_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/status", get(vault_status))
        .route("/rotate", post(rotate_secret))
}

/// GET /vault/status — check if Vault is enabled and connected
async fn vault_status() -> impl IntoResponse {
    if !vault::is_vault_enabled() {
        return (StatusCode::OK, Json(json!({
            "enabled": false,
            "message": "Vault not configured (VAULT_ADDR not set). Using env vars for secrets."
        }))).into_response();
    }

    match vault::parse_vault_config() {
        Ok(config) => {
            let status = vault::check_vault_status(&config).await;
            (StatusCode::OK, Json(json!(status))).into_response()
        }
        Err(e) => {
            error!(error = %e, "Vault config error");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "enabled": true,
                "connected": false,
                "error": format!("Config error: {}", e)
            }))).into_response()
        }
    }
}

/// POST /vault/rotate — rotate a secret key in Vault
async fn rotate_secret(
    Json(req): Json<RotateSecretRequest>,
) -> impl IntoResponse {
    if !vault::is_vault_enabled() {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": "Vault is not enabled. Set VAULT_ADDR to use secret rotation."
        }))).into_response();
    }

    let config = match vault::parse_vault_config() {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": format!("Vault config error: {}", e)
            }))).into_response();
        }
    };

    let vault_key = vault::map_config_to_vault_key(&req.key);

    match vault::rotate_secret(&config, &vault_key, &req.new_value).await {
        Ok(result) => {
            info!(
                key = %req.key,
                vault_key = %vault_key,
                version = ?result.vault_version,
                "Secret rotated successfully"
            );
            (StatusCode::OK, Json(json!(result))).into_response()
        }
        Err(e) => {
            error!(error = %e, key = %req.key, "Secret rotation failed");
            (StatusCode::BAD_GATEWAY, Json(json!({
                "error": format!("Rotation failed: {}", e),
                "key": req.key
            }))).into_response()
        }
    }
}
