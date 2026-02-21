use anyhow::{Result, Context};
use sqlx::{MySqlPool, Row};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use uuid::Uuid;

use rig::providers::gemini;
use rig::completion::Prompt;
use std::env;

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
        info!("Starting Clustering Job for tenant: {}", tenant_id);
        
        // MVP: Fetch recent 10 unclustered QA results
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
            info!("Not enough unclustered QA pairs to form conflicts/duplicates.");
            return Ok(());
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
                let cleaned = json_str.trim_start_matches("```json").trim_end_matches("```").trim();
                if let Ok(results) = serde_json::from_str::<Vec<serde_json::Value>>(cleaned) {
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
                    warn!("Failed to parse Gemini output as JSON: {}", cleaned);
                }
            },
            Err(e) => error!("Gemini prompt failed: {}", e)
        }

        Ok(())
    }
}
