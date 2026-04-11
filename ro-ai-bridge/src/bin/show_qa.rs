use sqlx::mysql::MySqlPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "mysql://mimir:mimir_password@127.0.0.1:3307/mimir".to_string());
    let pool = MySqlPoolOptions::new().connect(&db_url).await?;

    let qa_pairs: Vec<(String, String)> = sqlx::query_as(
        "SELECT question, answer FROM qa_results WHERE source_id = (SELECT id FROM data_sources WHERE name = 'diagnostic-testing-osa' LIMIT 1) LIMIT 2"
    ).fetch_all(&pool).await?;

    for (q, a) in qa_pairs {
        println!("Q: {}\nA: {}\n", q, a);
    }
    Ok(())
}
