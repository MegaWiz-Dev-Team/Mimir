use anyhow::Result;
use dotenvy::dotenv;
use mimir_core_ai::services::db::init_db;
use mimir_core_ai::services::qdrant::QdrantService;
use mimir_core_ai::qa_qc::indexer::run_indexer;
use rig::providers::ollama;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let db_pool = init_db().await?;
    let qdrant = QdrantService::new();
    let ollama_client = ollama::Client::new();
    
    let collection_name = "wiki_qa";

    run_indexer(&db_pool, &qdrant, &ollama_client, collection_name).await?;

    Ok(())
}
