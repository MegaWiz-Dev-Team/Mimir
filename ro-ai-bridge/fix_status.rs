use sqlx::mysql::MySqlPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "mysql://mimir:mimir_password@127.0.0.1:3307/mimir".to_string());
    let pool = MySqlPoolOptions::new().connect(&db_url).await?;

    // Fix stuck pipeline runs
    let _ = sqlx::query("UPDATE pipeline_runs SET status = 'failed', error_message = 'Manually stopped' WHERE status IN ('running', 'pending')").execute(&pool).await?;
    
    // Fix stuck pipeline steps
    let _ = sqlx::query("UPDATE pipeline_steps SET status = 'failed' WHERE status IN ('running', 'pending')").execute(&pool).await?;

    // Fix stuck QA extraction runs
    let _ = sqlx::query("UPDATE qa_extraction_runs SET status = 'failed' WHERE status IN ('running', 'pending')").execute(&pool).await?;

    // Fix stuck KG extraction runs
    let _ = sqlx::query("UPDATE kg_extraction_runs SET status = 'failed' WHERE status IN ('running', 'pending')").execute(&pool).await?;

    println!("All stuck states reset to failed.");
    Ok(())
}
