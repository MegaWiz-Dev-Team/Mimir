use anyhow::{Result, Context};
use sqlx::{MySqlPool, Row};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use uuid::Uuid;

use rig::providers::gemini;
use rig::completion::Prompt;
use std::env;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// Global flag to prevent multiple clustering jobs executing concurrently
pub static IS_CLUSTERING_RUNNING: AtomicBool = AtomicBool::new(false);
pub static PROCESSED_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static TOTAL_COUNT: AtomicUsize = AtomicUsize::new(0);

struct ClusteringGuard;
impl Drop for ClusteringGuard {
    fn drop(&mut self) {
        IS_CLUSTERING_RUNNING.store(false, Ordering::SeqCst);
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
    /// Fetch pending clusters for a tenant
    pub async fn get_clusters(pool: &MySqlPool, tenant_id: &str, status: Option<&str>) -> Result<Vec<ClusterDTO>> {
        let status_filter = status.unwrap_or("PENDING");
        
        let clusters = sqlx::query(
            r#"SELECT id, tenant_id, topic, reasoning, cluster_type, golden_answer, status 
               FROM qa_clusters 
               WHERE tenant_id = ? AND status = ?"#
        )
        .bind(tenant_id)
        .bind(status_filter)
        .fetch_all(pool)
        .await?;

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

        loop {
            // Fetch batch of 10 unclustered QA results
            let qas = sqlx::query(
                r#"SELECT id, question, answer 
                   FROM qa_results 
                   WHERE tenant_id = ? 
                   AND id NOT IN (SELECT qa_id FROM qa_cluster_items)
                   LIMIT 10"#
            )
            .bind(tenant_id)
            .fetch_all(pool).await?;

            if qas.len() < 2 {
                info!("Finished clustering loop: Not enough unclustered QA pairs remaining.");
                break;
            }

            // Use Gemini to group and detect conflicts
            let client = gemini::Client::new(&env::var("GEMINI_API_KEY").unwrap_or_default());
            let agent = client.agent("gemini-2.5-flash")
                .preamble("You are a Data Quality AI. Analyze the list of Q/A pairs. 
Find 1 conflict (contradicting info) AND 1 duplicate (similar info) if possible.
Return a STRICT JSON list: 
[
  {
    \"type\": \"CONFLICT\" | \"DUPLICATE\",
    \"topic\": \"The common topic\",
    \"reasoning\": \"Why they conflict or duplicate\",
    \"qa_id_1\": ID1,
    \"qa_id_2\": ID2,
    \"golden_answer\": \"(Provide a merged answer ONLY IF type is DUPLICATE, else act as if None)\"
  }
]")
                .build();

            let mut input_text = String::from("QA Pairs:\n");
            for qa in &qas {
                input_text.push_str(&format!("ID: {} | Q: {} | A: {}\n", qa.get::<i64, _>("id"), qa.get::<String, _>("question"), qa.get::<String, _>("answer")));
            }

            let resp = agent.prompt(input_text).await;
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
                        warn!("Failed to parse Gemini output as JSON: {}", json_str);
                    }
                },
                Err(e) => error!("Gemini prompt failed: {}", e)
            }
            
            // Advance progress count by the number of QA items processed in this iteration
            PROCESSED_COUNT.fetch_add(qas.len(), Ordering::SeqCst);
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
