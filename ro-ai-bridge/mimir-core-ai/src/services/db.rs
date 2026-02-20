use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use anyhow::Result;
use std::env;
use tracing::info;

pub type DbPool = MySqlPool;

use crate::models::model_config::ModelConfig;

pub async fn init_db() -> Result<MySqlPool> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    info!("🔌 Connecting to database: {}", database_url);
    
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    info!("✅ Database migrations applied");
    Ok(pool)
}

// ─── Model Config Functions ────────────────────────────────────────────────────

/// Get all active LLM models from the database
pub async fn get_active_llm_models(pool: &DbPool) -> Result<Vec<ModelConfig>> {
    let models = sqlx::query_as::<_, ModelConfig>(
        r#"SELECT model_id, provider, model_type, is_active, capabilities, metadata, created_at, updated_at
        FROM ai_models 
        WHERE is_active = true AND model_type = 'llm'
        ORDER BY provider, model_id"#
    )
    .fetch_all(pool)
    .await?;
    
    Ok(models)
}

/// Get a specific model by model_id
pub async fn get_model_by_id(pool: &DbPool, model_id: &str) -> Result<Option<ModelConfig>> {
    let model = sqlx::query_as::<_, ModelConfig>(
        r#"SELECT model_id, provider, model_type, is_active, capabilities, metadata, created_at, updated_at
        FROM ai_models 
        WHERE model_id = ?"#
    )
    .bind(model_id)
    .fetch_optional(pool)
    .await?;
    
    Ok(model)
}

/// Get all models for a specific provider
pub async fn get_models_by_provider(pool: &DbPool, provider: &str) -> Result<Vec<ModelConfig>> {
    let models = sqlx::query_as::<_, ModelConfig>(
        r#"SELECT model_id, provider, model_type, is_active, capabilities, metadata, created_at, updated_at
        FROM ai_models 
        WHERE provider = ? AND is_active = true
        ORDER BY model_id"#
    )
    .bind(provider)
    .fetch_all(pool)
    .await?;
    
    Ok(models)
}
