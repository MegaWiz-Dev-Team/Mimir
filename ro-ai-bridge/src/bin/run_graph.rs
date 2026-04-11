use anyhow::Result;
use dotenvy::dotenv;
use mimir_core_ai::services::db::init_db;
use mimir_core_ai::services::llm_router::LlmRouter;
use mimir_core_ai::services::graph_analytics::generate_graph_insights;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();
    
    // Set DATABASE_URL if missing
    if std::env::var("DATABASE_URL").is_err() {
        if let Ok(mariadb) = std::env::var("MARIADB_URL") {
            std::env::set_var("DATABASE_URL", mariadb);
        }
    }

    let pool = init_db().await.unwrap();
    let tenant_id = "megacare";

    println!("🚀 Resolving LlmRouter for graph insights...");
    let router = LlmRouter::new(pool.clone(), tenant_id).await.unwrap();

    println!("🧠 Running generate_graph_insights for tenant={}...", tenant_id);
    match generate_graph_insights(&pool, tenant_id, &router, Some("heimdall"), Some("mlx-community/gemma-4-31b-it-4bit")).await {
        Ok(_) => println!("✅ Graph Insights Generation Completed Successfully!"),
        Err(e) => println!("❌ Error: {}", e)
    }

    Ok(())
}
