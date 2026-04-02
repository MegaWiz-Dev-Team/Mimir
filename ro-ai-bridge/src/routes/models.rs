use axum::{extract::State, routing::get, Json, Router};
use mimir_core_ai::services::db::{get_active_llm_models, DbPool};
use tracing::error;

pub fn models_routes() -> Router<DbPool> {
    Router::new().route("/models", get(list_models))
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
