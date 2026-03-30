use anyhow::Result;
use crate::services::db::DbPool;
use crate::services::qdrant::QdrantService;
use crate::services::llm_router::LlmRouter;
use sqlx::Row;
use serde_json::json;
use tracing::{info, error};
use std::collections::HashMap;

pub async fn run_indexer(
    db_pool: &DbPool,
    qdrant: &QdrantService,
    collection_name: &str,
) -> Result<()> {
    info!("🚀 Starting Golden QA Indexing Pipeline...");

    qdrant.init_collection(collection_name, 1024).await?;

    let mut routers: HashMap<String, LlmRouter> = HashMap::new();

    // 1. Standalone Golden QA Results
    let unindexed_results = sqlx::query(
        "SELECT qr.id, qr.question, qr.answer, qr.tenant_id, ps.file_name, ps.chunk_index \
         FROM qa_results qr \
         JOIN pipeline_steps ps ON qr.step_id = ps.id \
         WHERE qr.qc_scanned = 1 \
         AND qr.id NOT IN (SELECT qa_id FROM qa_cluster_items) \
         AND qr.indexed_at IS NULL"
    )
    .fetch_all(db_pool)
    .await?;

    info!("📊 Found {} standalone golden Q/A pairs to index", unindexed_results.len());

    for row in unindexed_results {
        let id: i64 = row.get("id");
        let question: String = row.get("question");
        let answer: String = row.get("answer");
        let tenant_id: String = row.get("tenant_id");
        let file_name: String = row.get("file_name");
        let chunk_index: i64 = row.get("chunk_index");

        let text_to_embed = format!("Question: {} Answer: {}", question, answer);

        if !routers.contains_key(&tenant_id) {
            match LlmRouter::new(db_pool.clone(), &tenant_id).await {
                Ok(router) => { routers.insert(tenant_id.clone(), router); },
                Err(e) => {
                    error!("❌ Failed to initialize LlmRouter for tenant {}: {}", tenant_id, e);
                    continue;
                }
            }
        }

        let router = routers.get(&tenant_id).unwrap();

        match router.embed_texts_strict(&[text_to_embed]).await {
            Ok(ref mut matrix) if !matrix.is_empty() => {
                let vector = matrix.remove(0);
                let point = json!({
                    "points": [
                        {
                            "id": id,
                            "vector": vector,
                            "payload": {
                                "question": question,
                                "answer": answer,
                                "source": file_name,
                                "source_id": file_name,
                                "chunk": chunk_index,
                                "tenant_id": tenant_id
                            }
                        }
                    ]
                });

                if let Err(e) = qdrant.upsert_points(collection_name, point).await {
                    error!("❌ Failed to upsert standalone golden {} to Qdrant: {}", id, e);
                    continue;
                }

                sqlx::query("UPDATE qa_results SET indexed_at = NOW() WHERE id = ?")
                    .bind(id)
                    .execute(db_pool).await?;
            },
            Ok(_) => error!("❌ Heimdall returned empty embeddings for Q/A {}", id),
            Err(e) => error!("❌ Failed to embed Q/A {}: {}", id, e)
        }
    }

    // 2. Approved Clusters
    let unindexed_clusters = sqlx::query(
        "SELECT id, topic, golden_answer, tenant_id \
         FROM qa_clusters \
         WHERE status != 'PENDING' \
         AND indexed_at IS NULL \
         AND golden_answer IS NOT NULL"
    )
    .fetch_all(db_pool)
    .await?;

    info!("📊 Found {} approved Q/A clusters to index", unindexed_clusters.len());

    for row in unindexed_clusters {
        let id: String = row.get("id"); // UUID string
        let topic: String = row.get("topic");
        let golden_answer: String = row.get("golden_answer");
        let tenant_id: String = row.get("tenant_id");

        let text_to_embed = format!("Question: {} Answer: {}", topic, golden_answer);

        if !routers.contains_key(&tenant_id) {
            match LlmRouter::new(db_pool.clone(), &tenant_id).await {
                Ok(router) => { routers.insert(tenant_id.clone(), router); },
                Err(e) => {
                    error!("❌ Failed to initialize LlmRouter for tenant {}: {}", tenant_id, e);
                    continue;
                }
            }
        }

        let router = routers.get(&tenant_id).unwrap();

        match router.embed_texts_strict(&[text_to_embed]).await {
            Ok(ref mut matrix) if !matrix.is_empty() => {
                let vector = matrix.remove(0);
                let point = json!({
                    "points": [
                        {
                            "id": id, // Qdrant accepts UUID string as ID
                            "vector": vector,
                            "payload": {
                                "question": topic,
                                "answer": golden_answer,
                                "source": "cluster-approved",
                                "source_id": "cluster-approved",
                                "chunk": 0,
                                "tenant_id": tenant_id
                            }
                        }
                    ]
                });

                if let Err(e) = qdrant.upsert_points(collection_name, point).await {
                    error!("❌ Failed to upsert approved cluster {} to Qdrant: {}", id, e);
                    continue;
                }

                sqlx::query("UPDATE qa_clusters SET indexed_at = NOW() WHERE id = ?")
                    .bind(&id)
                    .execute(db_pool).await?;
            },
            Ok(_) => error!("❌ Heimdall returned empty embeddings for cluster {}", id),
            Err(e) => error!("❌ Failed to embed cluster {}: {}", id, e)
        }
    }

    info!("✅ Golden QA Indexing Completed.");
    Ok(())
}
