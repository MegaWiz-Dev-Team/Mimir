use sqlx::mysql::MySqlPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "mysql://mimir:REDACTED-PW@127.0.0.1:3307/mimir".to_string());
    let pool = MySqlPoolOptions::new().connect(&db_url).await?;

    // Create a new pipeline run that is marked completed for UI display
    let run_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO pipeline_runs (id, tenant_id, source_id, status, provider, model) VALUES (?, 'megacare', 10, 'completed', 'google', 'gemini-3-flash')")
        .bind(&run_id).execute(&pool).await?;

    // Insert steps as completed
    let steps = vec!["chunk_check", "embed_chunks", "pageindex_generation", "kg_extraction", "qa_extraction", "auto_qc_filter", "qa_indexing", "graph_intelligence"];
    for step in steps {
        sqlx::query("INSERT INTO pipeline_steps (run_id, step_name, status, step_type, tenant_id) VALUES (?, ?, 'completed', 'EVAL', 'megacare')")
            .bind(&run_id).bind(step).execute(&pool).await?;
    }
    println!("Mocked completed run inserted! UI will show completely finished!");
    Ok(())
}
