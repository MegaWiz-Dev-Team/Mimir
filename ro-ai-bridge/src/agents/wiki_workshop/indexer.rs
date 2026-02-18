use anyhow::Result;
use super::{WikiChunk, QAPair, AtomicFact, CoverageReport};
use rig::providers::ollama::{Client as OllamaClient};
use rig::embeddings::EmbeddingModel; // This import is necessary for the `embed_text` method on `ollama.embedding_model`
use rig::completion::Prompt;
use crate::services::db::DbPool;
use crate::services::qdrant::QdrantService;
use sqlx::Row;
use serde_json::json;
use tracing::{info, error};
use chrono::Utc;

pub async fn run_indexer(
    db_pool: &DbPool,
    qdrant: &QdrantService,
    ollama: &OllamaClient,
    collection_name: &str,
) -> Result<()> {
    info!("🚀 Starting Vector Indexing Pipeline...");

    // 1. Ensure Collection Exists
    qdrant.init_collection(collection_name, 768).await?; // nomic-embed-text uses 768 dims

    // 2. Fetch Unindexed Q/A Results
    let unindexed = sqlx::query(
        "SELECT qr.id, qr.question, qr.answer, ps.file_name, ps.chunk_index \
         FROM qa_results qr \
         JOIN pipeline_steps ps ON qr.step_id = ps.id \
         WHERE qr.indexed_at IS NULL"
    )
    .fetch_all(db_pool)
    .await?;

    if unindexed.is_empty() {
        info!("✅ No new data to index.");
        return Ok(());
    }

    info!("📊 Found {} Q/A pairs to index", unindexed.len());

    let embed_model = ollama.embedding_model("nomic-embed-text");

    for row in unindexed {
        let id: i64 = row.get("id");
        let question: String = row.get("question");
        let answer: String = row.get("answer");
        let file_name: String = row.get("file_name");
        let chunk_index: i64 = row.get("chunk_index");

        // Combine Q/A for embedding
        let text_to_embed = format!("Question: {} Answer: {}", question, answer);

        match embed_model.embed_text(&text_to_embed).await {
            Ok(embedding) => {
                let point = json!({
                    "points": [
                        {
                            "id": id,
                            "vector": embedding.vec,
                            "payload": {
                                "question": question,
                                "answer": answer,
                                "source": file_name,
                                "chunk": chunk_index
                            }
                        }
                    ]
                });

                if let Err(e) = qdrant.upsert_points(collection_name, point).await {
                    error!("❌ Failed to upsert point {}: {}", id, e);
                    continue;
                }

                // Mark as indexed
                sqlx::query("UPDATE qa_results SET indexed_at = NOW() WHERE id = ?")
                    .bind(id)
                    .execute(db_pool).await?;
            },
            Err(e) => {
                error!("❌ Failed to embed Q/A {}: {}", id, e);
            }
        }
    }

    info!("✅ Vector Indexing Completed.");
    Ok(())
}
