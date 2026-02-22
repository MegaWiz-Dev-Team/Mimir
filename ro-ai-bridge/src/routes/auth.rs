use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use sqlx::MySqlPool;

use mimir_core_ai::models::iam::{LoginRequest, LoginResponse};
use mimir_core_ai::services::iam::IamService;

pub fn auth_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/login", post(login))
}

async fn login(
    State(pool): State<MySqlPool>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let iam_service = IamService::new(pool);
    match iam_service.login(&payload.username, &payload.password).await {
        Ok((token, tenant_id)) => Ok(Json(LoginResponse { token, tenant_id })),
        Err(e) => {
            tracing::warn!("Login failed for user {}: {}", payload.username, e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
