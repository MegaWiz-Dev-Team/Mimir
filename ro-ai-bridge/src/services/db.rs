use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use anyhow::Result;
use std::env;
use tracing::info;

pub type DbPool = MySqlPool;

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
