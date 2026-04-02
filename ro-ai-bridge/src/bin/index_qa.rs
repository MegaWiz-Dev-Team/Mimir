use anyhow::Result;
use dotenvy::dotenv;
use mimir_core_ai::qa_qc::indexer::run_indexer;
use mimir_core_ai::services::db::init_db;
use mimir_core_ai::services::qdrant::QdrantService;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let db_pool = init_db().await?;
    let qdrant = QdrantService::new();

    let collection_name = "golden_qa";

    run_indexer(&db_pool, &qdrant, collection_name).await?;

    Ok(())
}
