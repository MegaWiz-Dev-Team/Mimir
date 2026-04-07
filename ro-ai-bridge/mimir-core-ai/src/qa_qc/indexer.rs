use crate::services::db::DbPool;
use crate::services::llm_router::LlmRouter;
use crate::services::qdrant::QdrantService;
use anyhow::Result;
use serde_json::json;
use sqlx::Row;
use std::collections::HashMap;
use tracing::{error, info};

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
        "SELECT qr.id, qr.question, qr.answer, qr.tenant_id, ps.file_name, ps.chunk_index, pr.source_id \
         FROM qa_results qr \
         JOIN pipeline_steps ps ON qr.step_id = ps.id \
         JOIN pipeline_runs pr ON ps.run_id = pr.id \
         WHERE qr.qc_scanned = 1 \
         AND qr.id NOT IN (SELECT qa_id FROM qa_cluster_items) \
         AND qr.indexed_at IS NULL \
         LIMIT 2000", // Process chunks of 2000 per index run to avoid memory blooms
    )
    .fetch_all(db_pool)
    .await?;

    info!(
        "📊 Found {} standalone golden Q/A pairs to index",
        unindexed_results.len()
    );

    // Group by tenant
    let mut tenant_standalone_groups: HashMap<String, Vec<sqlx::mysql::MySqlRow>> = HashMap::new();
    for row in unindexed_results {
        let tenant_id: String = row.get("tenant_id");
        tenant_standalone_groups.entry(tenant_id).or_default().push(row);
    }

    for (tenant_id, rows) in tenant_standalone_groups {
        if !routers.contains_key(&tenant_id) {
            match LlmRouter::new(db_pool.clone(), &tenant_id).await {
                Ok(router) => {
                    routers.insert(tenant_id.clone(), router);
                }
                Err(e) => {
                    error!("❌ Failed to init LlmRouter for tenant {}: {}", tenant_id, e);
                    continue;
                }
            }
        }
        let router = routers.get(&tenant_id).unwrap();

        for chunk in rows.chunks(64) {
            let mut texts_to_embed = Vec::new();
            for row in chunk {
                let question: String = row.get("question");
                let answer: String = row.get("answer");
                texts_to_embed.push(format!("Question: {} Answer: {}", question, answer));
            }

            match router.embed_texts_strict(&texts_to_embed).await {
                Ok(mut matrix) if matrix.len() == chunk.len() => {
                    let mut points = Vec::new();
                    let mut ids_to_update = Vec::new();

                    for (i, row) in chunk.iter().enumerate() {
                        let id: i64 = row.get("id");
                        let question: String = row.get("question");
                        let answer: String = row.get("answer");
                        let file_name: String = row.get("file_name");
                        let chunk_index: i64 = row.get("chunk_index");
                        let source_id: Option<i64> = row.get("source_id");
                        
                        // Parse source_ids properly as array of i64
                        let source_ids_arr: Vec<i64> = match source_id {
                            Some(sid) => vec![sid],
                            None => vec![],
                        };

                        points.push(json!({
                            "id": id,
                            "vector": { "dense": matrix[i].clone() },
                            "payload": {
                                "question": question,
                                "answer": answer,
                                "source_file": file_name,
                                "source_ids": source_ids_arr,
                                "chunk": chunk_index,
                                "tenant_id": &tenant_id,
                                "is_active": true,
                                "is_golden": true,
                                "source_type": "golden_qa"
                            }
                        }));
                        ids_to_update.push(id);
                    }

                    if let Err(e) = qdrant.upsert_points(collection_name, json!({"points": points})).await {
                        error!("❌ Failed to batch upsert standalone golden to Qdrant: {}", e);
                        continue;
                    }

                    // Batch update indexed_at
                    for id in ids_to_update {
                        let _ = sqlx::query("UPDATE qa_results SET indexed_at = NOW() WHERE id = ?")
                            .bind(id)
                            .execute(db_pool)
                            .await;
                    }
                }
                Ok(_) => error!("❌ Heimdall returned mismatched embeddings length for batch"),
                Err(e) => error!("❌ Failed to embed batch: {}", e),
            }
        }
    }

    // 2. Approved Clusters
    let unindexed_clusters = sqlx::query(
        "SELECT c.id, c.topic, c.golden_answer, c.tenant_id, \
          (SELECT GROUP_CONCAT(DISTINCT pr.source_id) \
           FROM qa_cluster_items ci \
           JOIN qa_results qr ON ci.qa_id = qr.id \
           JOIN pipeline_steps ps ON qr.step_id = ps.id \
           JOIN pipeline_runs pr ON ps.run_id = pr.id \
           WHERE ci.cluster_id = c.id \
          ) as source_ids \
         FROM qa_clusters c \
         WHERE c.status != 'PENDING' \
         AND c.indexed_at IS NULL \
         AND c.golden_answer IS NOT NULL \
         LIMIT 2000",
    )
    .fetch_all(db_pool)
    .await?;

    info!(
        "📊 Found {} approved Q/A clusters to index",
        unindexed_clusters.len()
    );

    let mut tenant_cluster_groups: HashMap<String, Vec<sqlx::mysql::MySqlRow>> = HashMap::new();
    for row in unindexed_clusters {
        let tenant_id: String = row.get("tenant_id");
        tenant_cluster_groups.entry(tenant_id).or_default().push(row);
    }

    for (tenant_id, rows) in tenant_cluster_groups {
        if !routers.contains_key(&tenant_id) {
            match LlmRouter::new(db_pool.clone(), &tenant_id).await {
                Ok(router) => {
                    routers.insert(tenant_id.clone(), router);
                }
                Err(e) => {
                    error!("❌ Failed to init LlmRouter for tenant {}: {}", tenant_id, e);
                    continue;
                }
            }
        }
        let router = routers.get(&tenant_id).unwrap();

        for chunk in rows.chunks(64) {
            let mut texts_to_embed = Vec::new();
            for row in chunk {
                let topic: String = row.get("topic");
                let golden_answer: String = row.get("golden_answer");
                texts_to_embed.push(format!("Question: {} Answer: {}", topic, golden_answer));
            }

            match router.embed_texts_strict(&texts_to_embed).await {
                Ok(mut matrix) if matrix.len() == chunk.len() => {
                    let mut points = Vec::new();
                    let mut ids_to_update = Vec::new();

                    for (i, row) in chunk.iter().enumerate() {
                        let id: String = row.get("id"); // UUID string
                        let topic: String = row.get("topic");
                        let golden_answer: String = row.get("golden_answer");
                        let source_ids_str: Option<String> = row.get("source_ids");
                        
                        // Parse source_ids from "1,2,3" to Vec<i64>
                        let mut source_ids_arr: Vec<i64> = Vec::new();
                        if let Some(sid_str) = source_ids_str {
                            for sid in sid_str.split(',') {
                                if let Ok(parsed) = sid.parse::<i64>() {
                                    source_ids_arr.push(parsed);
                                }
                            }
                        }

                        points.push(json!({
                            "id": id,
                            "vector": { "dense": matrix[i].clone() },
                            "payload": {
                                "question": topic,
                                "answer": golden_answer,
                                "source_file": "cluster-approved",
                                "source_ids": source_ids_arr,
                                "chunk": 0,
                                "tenant_id": &tenant_id,
                                "is_active": true,
                                "is_golden": true,
                                "source_type": "golden_qa_cluster",
                                "cluster_type": "Merged"
                            }
                        }));
                        ids_to_update.push(id);
                    }

                    if let Err(e) = qdrant.upsert_points(collection_name, json!({"points": points})).await {
                        error!("❌ Failed to batch upsert approved clusters to Qdrant: {}", e);
                        continue;
                    }

                    for id in ids_to_update {
                        let _ = sqlx::query("UPDATE qa_clusters SET indexed_at = NOW() WHERE id = ?")
                            .bind(&id)
                            .execute(db_pool)
                            .await;

                        let _ = sqlx::query("UPDATE qa_results SET indexed_at = NOW() WHERE id IN (SELECT qa_id FROM qa_cluster_items WHERE cluster_id = ?)")
                            .bind(&id)
                            .execute(db_pool)
                            .await;
                    }
                }
                Ok(_) => error!("❌ Heimdall returned mismatched embeddings length for cluster batch"),
                Err(e) => error!("❌ Failed to embed cluster batch: {}", e),
            }
        }
    }

    info!("✅ Golden QA Indexing Completed.");
    Ok(())
}
