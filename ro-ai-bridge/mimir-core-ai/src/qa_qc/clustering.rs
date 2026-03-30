use anyhow::Result;
use sqlx::{MySqlPool, Row};
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use uuid::Uuid;


use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Global flag to prevent multiple clustering jobs executing concurrently
pub static IS_CLUSTERING_RUNNING: AtomicBool = AtomicBool::new(false);
pub static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
pub static PROCESSED_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static TOTAL_COUNT: AtomicUsize = AtomicUsize::new(0);

struct ClusteringGuard;
impl Drop for ClusteringGuard {
    fn drop(&mut self) {
        IS_CLUSTERING_RUNNING.store(false, Ordering::SeqCst);
        STOP_REQUESTED.store(false, Ordering::SeqCst);
        PROCESSED_COUNT.store(0, Ordering::SeqCst);
        TOTAL_COUNT.store(0, Ordering::SeqCst);
        info!("Released global Clustering lock.");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterItemDTO {
    pub qa_id: i64,
    pub source_label: String,
    pub question: String,
    pub answer: String,
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterDTO {
    pub id: String,
    pub tenant_id: String,
    pub topic: String,
    pub reasoning: Option<String>,
    pub cluster_type: String,
    pub golden_answer: Option<String>,
    pub status: String,
    pub items: Vec<ClusterItemDTO>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveClusterRequest {
    pub resolution_type: String, // "ACCEPT_A", "ACCEPT_B", "MERGE"
    pub golden_answer: Option<String>, // Required if MERGE
}

pub struct ClusteringService;

impl ClusteringService {
    pub async fn get_clusters(pool: &MySqlPool, tenant_id: &str, status: Option<&str>) -> Result<Vec<ClusterDTO>> {
        let status_filter = status.unwrap_or("ALL");
        
        let clusters = if status_filter == "ALL" {
            sqlx::query(
                r#"SELECT id, tenant_id, topic, reasoning, cluster_type, golden_answer, status 
                   FROM qa_clusters 
                   WHERE tenant_id = ?"#
            )
            .bind(tenant_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"SELECT id, tenant_id, topic, reasoning, cluster_type, golden_answer, status 
                   FROM qa_clusters 
                   WHERE tenant_id = ? AND status = ?"#
            )
            .bind(tenant_id)
            .bind(status_filter)
            .fetch_all(pool)
            .await?
        };

        let mut dtos = Vec::new();
        for c in clusters {
            let items = sqlx::query(
                r#"SELECT ci.qa_id, ci.source_label, qr.question, qr.answer, qr.context
                   FROM qa_cluster_items ci
                   JOIN qa_results qr ON ci.qa_id = qr.id
                   WHERE ci.cluster_id = ?"#
            )
            .bind(&c.get::<String, _>("id"))
            .fetch_all(pool)
            .await?;

            let item_dtos = items.into_iter().map(|i| ClusterItemDTO {
                qa_id: i.get("qa_id"),
                source_label: i.get("source_label"),
                question: i.get("question"),
                answer: i.get("answer"),
                context: i.get("context"),
            }).collect();

            dtos.push(ClusterDTO {
                id: c.get("id"),
                tenant_id: c.get("tenant_id"),
                topic: c.get("topic"),
                reasoning: c.get("reasoning"),
                cluster_type: c.get("cluster_type"),
                golden_answer: c.get("golden_answer"),
                status: c.get("status"),
                items: item_dtos,
            });
        }

        Ok(dtos)
    }

    /// Resolve a cluster and mark it as completed
    pub async fn resolve_cluster(pool: &MySqlPool, cluster_id: &str, req: ResolveClusterRequest) -> Result<()> {
        let status = match req.resolution_type.as_str() {
            "ACCEPT_A" => "RESOLVED_A",
            "ACCEPT_B" => "RESOLVED_B",
            _ => "MERGED"
        };

        sqlx::query(
            "UPDATE qa_clusters SET status = ?, golden_answer = ? WHERE id = ?"
        )
        .bind(status)
        .bind(req.golden_answer)
        .bind(cluster_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Generate clusters and detect conflicts using Gemini (Mock HDBSCAN for Phase 7 MVP)
    pub async fn trigger_clustering(pool: &MySqlPool, tenant_id: &str) -> Result<()> {
        if IS_CLUSTERING_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            info!("Clustering job is already running, skipping trigger.");
            return Ok(());
        }
        
        let _guard = ClusteringGuard;
        PROCESSED_COUNT.store(0, Ordering::SeqCst);
        
        info!("Starting Clustering Job for tenant: {}", tenant_id);

        // Fetch total unclustered QA count for tracking
        let total_row: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) 
               FROM qa_results 
               WHERE tenant_id = ? 
               AND qc_scanned = FALSE
               AND id NOT IN (SELECT qa_id FROM qa_cluster_items)"#
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;
        
        TOTAL_COUNT.store(total_row.0 as usize, Ordering::SeqCst);
        
        if total_row.0 < 2 {
            info!("Not enough unclustered QA pairs to form conflicts/duplicates.");
            return Ok(());
        }

        let max_iterations = 100; // Safety cap
        let mut iteration = 0;

        loop {
            // Check for stop request
            if STOP_REQUESTED.load(Ordering::SeqCst) {
                info!("Clustering job stopped by user request.");
                break;
            }

            // Safety: max iterations
            iteration += 1;
            if iteration > max_iterations {
                warn!("Clustering hit max iteration limit ({}), stopping.", max_iterations);
                break;
            }

            // Fetch batch of 10 unclustered QA results
            let qas = sqlx::query(
                r#"SELECT id, question, answer 
                   FROM qa_results 
                   WHERE tenant_id = ? 
                   AND qc_scanned = FALSE
                   AND id NOT IN (SELECT qa_id FROM qa_cluster_items)
                   ORDER BY id ASC
                   LIMIT 10"#
            )
            .bind(tenant_id)
            .fetch_all(pool).await?;

            if qas.len() < 2 {
                info!("Finished clustering loop: Not enough unclustered QA pairs remaining.");
                break;
            }

            let current_ids: Vec<i64> = qas.iter().map(|q| q.get::<i64, _>("id")).collect();

            // Use TenantConfig to resolve clustering model (using pipeline_evaluator slot)
            let router = crate::services::llm_router::LlmRouter::new(pool.clone(), tenant_id).await?;
            let (client, model) = router.resolve_client("pipeline_evaluator")?;
            
            let preamble = "You are a Data Quality AI. Analyze the list of Q/A pairs. \
Find 1 conflict (contradicting info) AND 1 duplicate (similar info) if possible.\n\
Return a STRICT JSON list: \n\
[\n\
  {\n\
    \"type\": \"CONFLICT\" | \"DUPLICATE\",\n\
    \"topic\": \"The common topic\",\n\
    \"reasoning\": \"Why they conflict or duplicate\",\n\
    \"qa_id_1\": ID1,\n\
    \"qa_id_2\": ID2,\n\
    \"golden_answer\": \"(Provide a merged answer ONLY IF type is DUPLICATE, else act as if None)\"\n\
  }\n\
]";

            let mut input_text = String::from("QA Pairs:\n");
            for qa in &qas {
                input_text.push_str(&format!("ID: {} | Q: {} | A: {}\n", qa.get::<i64, _>("id"), qa.get::<String, _>("question"), qa.get::<String, _>("answer")));
            }

            let resp = client.prompt(&model, preamble, &input_text, 2048, 0.3).await;
            
            match resp {
                Ok(json_str) => {
                    if let Ok(results) = Self::parse_gemini_clustering_output(&json_str) {
                        for r in results {
                            let c_type = r["type"].as_str().unwrap_or("DUPLICATE");
                            let topic = r["topic"].as_str().unwrap_or("Unknown Topic");
                            let reasoning = r["reasoning"].as_str().unwrap_or("");
                            let golden = r["golden_answer"].as_str();
                            let id1 = r["qa_id_1"].as_i64().unwrap_or(0);
                            let id2 = r["qa_id_2"].as_i64().unwrap_or(0);

                            if id1 > 0 && id2 > 0 {
                                let cluster_id = Uuid::new_v4().to_string();
                                sqlx::query(
                                    "INSERT INTO qa_clusters (id, tenant_id, topic, reasoning, cluster_type, golden_answer, status) VALUES (?, ?, ?, ?, ?, ?, 'PENDING')"
                                )
                                .bind(&cluster_id)
                                .bind(tenant_id)
                                .bind(topic)
                                .bind(reasoning)
                                .bind(c_type)
                                .bind(golden)
                                .execute(pool).await?;

                                sqlx::query("INSERT INTO qa_cluster_items (cluster_id, qa_id, source_label) VALUES (?, ?, 'A')")
                                    .bind(&cluster_id)
                                    .bind(id1)
                                    .execute(pool)
                                    .await?;
                                sqlx::query("INSERT INTO qa_cluster_items (cluster_id, qa_id, source_label) VALUES (?, ?, 'B')")
                                    .bind(&cluster_id)
                                    .bind(id2)
                                    .execute(pool)
                                    .await?;
                                
                                info!("Created Cluster: {}", cluster_id);
                            }
                        }
                    } else {
                        warn!("Failed to parse LLM output as JSON: {}", json_str);
                    }
                },
                Err(e) => error!("LLM prompt failed: {}", e)
            }
            
            // Mark these IDs as scanned
            if !current_ids.is_empty() {
                let params = current_ids.iter().map(|_| "?").collect::<Vec<&str>>().join(",");
                let query_str = format!("UPDATE qa_results SET qc_scanned = TRUE WHERE id IN ({})", params);
                let mut db_query = sqlx::query(&query_str);
                for id in &current_ids {
                    db_query = db_query.bind(id);
                }
                if let Err(e) = db_query.execute(pool).await {
                    error!("Failed to update qc_scanned flags: {}", e);
                }
            }
            
            // Advance progress count, capped at total
            let new_processed = PROCESSED_COUNT.load(Ordering::SeqCst) + qas.len();
            let total = TOTAL_COUNT.load(Ordering::SeqCst);
            PROCESSED_COUNT.store(new_processed.min(total), Ordering::SeqCst);
        }

        Ok(())
    }

    /// Generate QA pairs for a specific chunk's content using Gemini
    pub async fn generate_qa_for_content(pool: &MySqlPool, tenant_id: &str, chunk_id: i64, content: &str) -> Result<()> {
        if content.trim().len() < 50 {
            info!("Chunk {} content too short for QA generation, skipping", chunk_id);
            return Ok(());
        }

        let router = crate::services::llm_router::LlmRouter::new(pool.clone(), tenant_id).await?;
        let (client, model) = router.resolve_client("pipeline_generator")?;
        
        let preamble = "You are a QA Generator. Given the following text content, generate 2-3 high-quality question-answer pairs that test understanding of the material. Return STRICT JSON list:\n\
[\n\
  {\"question\": \"...\", \"answer\": \"...\"}\n\
]\n\
Keep answers concise and factual. Only generate questions that can be directly answered from the given text.";

        let prompt_text = format!("Generate QA pairs from this content:\n\n{}", content);

        let resp = client.prompt(&model, preamble, &prompt_text, 2048, 0.7).await;

        match resp {
            Ok(json_str) => {
                let cleaned = json_str.trim_start_matches("```json").trim_end_matches("```").trim();
                if let Ok(qa_pairs) = serde_json::from_str::<Vec<serde_json::Value>>(cleaned) {
                    // Create a mock pipeline run/step for these QA results
                    let run_id = Uuid::new_v4().to_string();
                    let _ = sqlx::query("INSERT INTO pipeline_runs (id, status, provider, model) VALUES (?, 'COMPLETED', ?, ?)")
                        .bind(&run_id).bind(client.provider_name()).bind(&model).execute(pool).await;
                    let _ = sqlx::query("INSERT INTO pipeline_steps (run_id, file_name, status, step_type) VALUES (?, ?, 'COMPLETED', 'GENERATE')")
                        .bind(&run_id).bind(format!("chunk_{}", chunk_id)).execute(pool).await;

                    let step_record = sqlx::query!("SELECT id FROM pipeline_steps WHERE run_id = ? LIMIT 1", run_id)
                        .fetch_one(pool).await?;

                    for qa in &qa_pairs {
                        let question = qa["question"].as_str().unwrap_or("");
                        let answer = qa["answer"].as_str().unwrap_or("");
                        if !question.is_empty() && !answer.is_empty() {
                            let _ = sqlx::query(
                                "INSERT INTO qa_results (step_id, question, answer, context, tenant_id) VALUES (?, ?, ?, ?, ?)"
                            )
                            .bind(step_record.id)
                            .bind(question)
                            .bind(answer)
                            .bind(&content[..content.len().min(500)])
                            .bind(tenant_id)
                            .execute(pool).await;
                        }
                    }
                    info!("Generated {} QA pairs for chunk {}", qa_pairs.len(), chunk_id);
                } else {
                    warn!("Failed to parse LLM QA output for chunk {}: {}", chunk_id, json_str);
                }
            }
            Err(e) => {
                error!("LLM QA generation failed for chunk {}: {}", chunk_id, e);
                return Err(e.into());
            }
        }

        Ok(())
    }

    /// Extracted helper to parse Gemini JSON output
    pub fn parse_gemini_clustering_output(json_str: &str) -> std::result::Result<Vec<serde_json::Value>, serde_json::Error> {
        let cleaned = json_str.trim_start_matches("```json").trim_end_matches("```").trim();
        serde_json::from_str::<Vec<serde_json::Value>>(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini_clean_json() {
        let json_input = r#"[
            {
                "type": "CONFLICT",
                "topic": "Combat",
                "reasoning": "Contradicting advice",
                "qa_id_1": 1,
                "qa_id_2": 2,
                "golden_answer": null
            }
        ]"#;

        let res = ClusteringService::parse_gemini_clustering_output(json_input).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0]["type"], "CONFLICT");
        assert_eq!(res[0]["qa_id_1"], 1);
    }

    #[test]
    fn test_parse_gemini_markdown_wrapped_json() {
        let json_input = r#"```json
[
    {
        "type": "DUPLICATE",
        "topic": "Movement",
        "reasoning": "Same info",
        "qa_id_1": 4,
        "qa_id_2": 5,
        "golden_answer": "Just walk"
    }
]
```"#;

        let res = ClusteringService::parse_gemini_clustering_output(json_input).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0]["type"], "DUPLICATE");
        assert_eq!(res[0]["qa_id_1"], 4);
        assert_eq!(res[0]["golden_answer"], "Just walk");
    }

    #[test]
    fn test_parse_gemini_invalid_json() {
        let json_input = "Not a json response";
        let res = ClusteringService::parse_gemini_clustering_output(json_input);
        assert!(res.is_err());
    }
}
