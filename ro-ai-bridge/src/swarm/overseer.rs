use anyhow::{Result, anyhow};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::llm_router::LlmRouter;
use mimir_core_ai::services::qdrant::QdrantService;
use std::sync::Arc;
use rig::completion::Prompt;
use tracing::{info, instrument};
use serde::{Deserialize, Serialize};

use serde_json::json;

use super::skills::{VectorSearchTool, GraphSearchTool, TreeSearchTool};
use super::souls::OVERSEER_SYSTEM_PROMPT;
use super::privacy::MedicalPrivacyShield;

#[derive(Serialize, Deserialize, Clone, schemars::JsonSchema)]
pub struct ActionApproval {
    pub action_type: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct SwarmResponse {
    pub reasoning: String,
    pub final_answer: String,
    pub action_required: Option<ActionApproval>,
}

pub struct OverseerManager {
    db_pool: DbPool,
    qdrant: Arc<QdrantService>,
    router: Arc<LlmRouter>,
}

impl OverseerManager {
    pub fn new(
        db_pool: DbPool,
        qdrant: Arc<QdrantService>,
        router: Arc<LlmRouter>,
    ) -> Self {
        Self {
            db_pool,
            qdrant,
            router,
        }
    }

    /// Run the full swarm simulation and return the final synthesized answer.
    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn run_swarm(&self, tenant_id: &str, query: &str, session_id: Option<&str>) -> Result<SwarmResponse> {
        let generation_slot = self.router.config.resolve_slot("generation", None, None);
        let api_key = self.router.config.openai_api_key.clone().unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());
        let endpoint = std::env::var("OPENAI_API_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        
        let start_time = std::time::Instant::now();

        let client = rig::providers::openai::Client::from_url(&api_key, &endpoint);
        let generation_model_name = generation_slot.model.clone();

        let embedding_model = self.router.config.resolve_slot("embedding", None, None).model.clone();

        let vector_tool = VectorSearchTool::new(
            self.db_pool.clone(),
            self.qdrant.clone(),
            self.router.clone(),
            embedding_model,
            "mimir_db".to_string(), 
        );

        let graph_tool = GraphSearchTool::new(
            self.db_pool.clone(),
        );

        let tree_tool = TreeSearchTool::new(
            self.db_pool.clone(),
            self.router.clone(),
        );

        // Rig-Guard: Medical Privacy Shield
        let safe_query = MedicalPrivacyShield::scrub(query);

        // Rig-Guard: Structured Output
        let structural_prompt = format!(
            "{}\n\nIMPORTANT: You must return the final answer EXACTLY as a JSON object matching this schema: \n{}",
            OVERSEER_SYSTEM_PROMPT,
            serde_json::to_string_pretty(&schemars::schema_for!(SwarmResponse)).unwrap()
        );

        let overseer_agent = client
            .agent(generation_model_name.as_str())
            .preamble(&structural_prompt)
            .tool(vector_tool)
            .tool(graph_tool)
            .tool(tree_tool)
            .build();

        let mut history_text = String::new();
        let mut new_history = vec![];

        if let Some(sid) = session_id {
            let row: Option<(serde_json::Value,)> = sqlx::query_as(
                "SELECT state_json FROM swarm_checkpoints WHERE session_id = $1 AND tenant_id = $2"
            )
            .bind(sid)
            .bind(tenant_id)
            .fetch_optional(&self.db_pool)
            .await?;

            if let Some((state_json,)) = row {
                if let Some(history) = state_json.get("history").and_then(|h| h.as_array()) {
                    for msg in history {
                        if let (Some(r), Some(c)) = (msg.get("role").and_then(|r| r.as_str()), msg.get("content").and_then(|c| c.as_str())) {
                            history_text.push_str(&format!("{}: {}\n", r, c));
                            new_history.push(msg.clone());
                        }
                    }
                }
            }
        }

        let augmented_query = if history_text.is_empty() {
            format!("Tenant ID: {}\nQuery: {}", tenant_id, safe_query)
        } else {
            format!("Tenant ID: {}\nPrevious Context:\n{}\n\nNew Query: {}", tenant_id, history_text, safe_query)
        };

        // Auto-correction Context Constraint Loop
        let mut final_answer = None;
        let mut current_query = augmented_query.clone();

        let mut total_input_tokens = 0_usize;
        let mut total_output_tokens = 0_usize;
        let mut loop_count = 0;

        info!("Starting Rig-Orchestrator Swarm with {}", generation_model_name);

        for iteration in 1..=3 {
            loop_count += 1;
            total_input_tokens += current_query.len() / 3; // Approximation baseline if tiktoken missing

            let raw_response = overseer_agent.prompt(current_query.clone()).await?;

            total_output_tokens += raw_response.len() / 3;

            // Try to parse out potential JSON blocks
            let json_part = raw_response
                .trim()
                .trim_start_matches("```json")
                .trim_end_matches("```")
                .trim();

            match serde_json::from_str::<SwarmResponse>(json_part) {
                Ok(response) => {
                    final_answer = Some(response);
                    break;
                }
                Err(e) => {
                    info!("Rig-Guard: Overseer JSON Parse error on iteration {}. Forcing self-correction. Error: {}", iteration, e);
                    current_query = format!("{}\n\nYour previous response was not valid according to the schema. Output only JSON format without markdown blocks. Error: {}", augmented_query, e);
                }
            }
        }

        let answer = final_answer.ok_or_else(|| anyhow!("Rig-Guard: Overseer completely failed to return valid structured JSON after 3 iterations."))?;

        // Rig-Flow: Checkpoint Upsert
        if let Some(sid) = session_id {
            new_history.push(json!({"role": "User", "content": safe_query}));
            new_history.push(json!({"role": "Assistant", "content": answer.final_answer}));
            let new_state_json = json!({
                "history": new_history,
                "action_pending": answer.action_required.is_some()
            });

            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO swarm_checkpoints (session_id, tenant_id, state_json)
                VALUES ($1, $2, $3)
                ON CONFLICT (session_id, tenant_id) DO UPDATE 
                SET state_json = EXCLUDED.state_json, updated_at = NOW()
                "#
            )
            .bind(sid)
            .bind(tenant_id)
            .bind(&new_state_json)
            .execute(&self.db_pool)
            .await 
            {
                tracing::error!("Failed to checkpoint Swarm State to database: {}", e);
            }
        }
        
        // Swarm Economics (Phase 3 Hook) - Deducted ONCE at the end of execution to prevent DB IO Lock limitations
        let final_latency = start_time.elapsed().as_millis() as i32;
        if let Err(e) = crate::routes::llm_usage::insert_llm_usage_log(
            &self.db_pool,
            tenant_id,
            &generation_model_name,
            "rig_overseer",
            Some(&endpoint),
            Some(&format!("swarm_iterations:{}", loop_count)),
            total_input_tokens as i32,
            total_output_tokens as i32,
            (total_input_tokens + total_output_tokens) as i32,
            final_latency,
            "success",
            None
        ).await {
            tracing::error!("Swarm Economics Verification Failed: Could not deduct Ledger Budget - {}", e);
        } else {
            info!("💰 Swarm Economics: Successfully collected {} iterations and deducted total tokens in single pass.", loop_count);
        }

        info!("Swarm orchestration complete in {}ms.", final_latency);

        Ok(answer)
    }
}
