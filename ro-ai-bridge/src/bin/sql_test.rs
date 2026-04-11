use sqlx::mysql::MySqlPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "mysql://mimir:REDACTED-PW@127.0.0.1:3307/mimir".to_string());
    let pool = MySqlPoolOptions::new().connect(&db_url).await?;

    let tables: Vec<(String,)> = sqlx::query_as("SHOW TABLES").fetch_all(&pool).await?;
    for (t,) in tables {
        if t.contains("run") || t.contains("step") || t.contains("extraction") {
            println!("TABLE: {}", t);
        }
    }
    Ok(())
}
