use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::{get, post}, Extension, Json,
    Router,
};
use sqlx::MySqlPool;
use std::sync::Arc;

use crate::config::Config;
use mimir_core_ai::models::iam::{LoginRequest, LoginResponse};
use mimir_core_ai::services::iam::IamService;
use mimir_core_ai::services::sso::{SsoService, TokenExchangeRequest};

pub fn auth_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/login", post(login))
        .route("/sso-config", get(sso_config))
        .route("/sso-exchange", post(sso_exchange))
}

async fn login(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let iam_service = IamService::new(pool, config.jwt_secret.clone());
    match iam_service
        .login(&payload.username, &payload.password)
        .await
    {
        Ok((token, tenant_id)) => Ok(Json(LoginResponse { token, tenant_id })),
        Err(e) => {
            tracing::warn!("Login failed for user {}: {}", payload.username, e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

async fn sso_config() -> impl IntoResponse {
    let config = SsoService::get_sso_config();
    Json(config)
}

async fn sso_exchange(
    State(pool): State<MySqlPool>,
    Extension(config): Extension<Arc<Config>>,
    Json(payload): Json<TokenExchangeRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let sso_service = SsoService::new(pool, config.jwt_secret.clone());
    match sso_service.exchange_code(payload).await {
        Ok(res) => Ok(Json(res)),
        Err(e) => {
            tracing::error!("SSO Token Exchange Error: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}
