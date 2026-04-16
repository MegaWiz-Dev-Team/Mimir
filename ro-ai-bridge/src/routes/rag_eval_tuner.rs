use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use mimir_core_ai::services::db::DbPool;
use rig::completion::Prompt;
use schemars::JsonSchema;

use crate::routes::tenant::extract_tenant_id;
use super::rag_eval::{RagEvalItem, RagEvalParams, execute_evaluation_run, RagEvalRunRequest};

#[derive(Debug, Deserialize)]
pub struct AutoTuneRequest {
    pub eval_set: Vec<RagEvalItem>,
    pub base_params: RagEvalParams,
    pub iterations: i32,
    pub target_metric: Option<String>,
    pub judge_model: Option<String>,
    pub judge_provider: Option<String>,
    pub tuner_model: Option<String>,
    pub tuner_provider: Option<String>,
    pub dataset_id: Option<String>,
    pub dataset_name: Option<String>,
    pub max_token_budget: Option<u64>,
    pub min_accuracy: Option<f64>,
    pub max_latency: Option<u64>,
    pub max_tokens_per_run: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SuggestedParams {
    pub weight_vector: f64,
    pub weight_tree: f64,
    pub weight_graph: f64,
    pub top_k: usize,
    pub vector_alpha: f64,
    pub vector_threshold: f64,
    /// Optional: number of graph hops (1-3). Default: 2.
    #[serde(default)]
    pub graph_hops: Option<i32>,
    /// Optional: rerank strategy ("weighted", "rrf", "cross-encoder"). Default: "weighted".
    #[serde(default)]
    pub rerank_strategy: Option<String>,
    /// Optional: generation temperature (0.0 - 1.0).
    #[serde(default)]
    pub generation_temperature: Option<f64>,
    /// Optional: max tokens for generation.
    #[serde(default)]
    pub generation_max_tokens: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TunerIterationResult {
    pub reasoning: String,
    pub suggested_params: SuggestedParams,
}

/// POST /api/v1/rag-eval/auto-tune
pub async fn run_auto_tune(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<AutoTuneRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let job_id = Uuid::new_v4().to_string();

    let target_metric = payload.target_metric.clone().unwrap_or_else(|| "ndcg".to_string());
    
    let _ = sqlx::query(
        "INSERT INTO rag_auto_tuner_jobs (id, tenant_id, target_metric, iterations, current_iteration, dataset_id, dataset_name) VALUES (?, ?, ?, ?, 0, ?, ?)"
    )
    .bind(&job_id)
    .bind(&tenant_id)
    .bind(&target_metric)
    .bind(payload.iterations)
    .bind(&payload.dataset_id)
    .bind(&payload.dataset_name)
    .execute(&pool)
    .await;

    // Spawn background worker
    let tenant_id_clone = tenant_id.to_string();
    let pool_clone = pool.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        let _ = tuning_loop(job_id_clone, tenant_id_clone, pool_clone, payload).await;
    });

    Ok(Json(json!({
        "job_id": job_id,
        "status": "started"
    })))
}

async fn tuning_loop(
    job_id: String,
    tenant_id: String,
    pool: DbPool,
    req: AutoTuneRequest,
) -> Result<(), anyhow::Error> {
    let mut current_params = req.base_params.clone();
    let mut best_score = -1.0;
    let mut best_run_id = None;
    let mut accumulated_tokens: u64 = 0;

    let target = req.target_metric.clone().unwrap_or_else(|| "ndcg".to_string());
    
    // Resolve tuner model config
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
        
    let slot = llm_config.resolve_slot("generation", req.tuner_provider.as_deref(), req.tuner_model.as_deref());
    let api_key = llm_config
        .heimdall_api_key
        .clone()
        .unwrap_or_else(|| std::env::var("HEIMDALL_API_KEY").unwrap_or_default());

    let endpoint = std::env::var("HEIMDALL_API_URL").unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
    let client = rig::providers::openai::Client::from_url(&api_key, &endpoint);

    let system_prompt = format!(
        "You are the Asgard Auto-Tuner Expert. Your goal is to optimize RAG retrieval parameters to maximize '{}'.\n\
        You will receive evaluation results from the previous iteration including scores and specific queries that failed or succeeded.\n\
        Analyze the failures (e.g. vector didn't contribute, or relevant match was ranked low). \n\
        Output MUST be EXACTLY a JSON object matching this schema:\n{}",
        target,
        serde_json::to_string_pretty(&schemars::schema_for!(TunerIterationResult)).unwrap()
    );

    let agent = client.agent(&slot.model).preamble(&system_prompt).build();

    let mut no_improvement_count = 0;
    const EARLY_STOP_PATIENCE: i32 = 2;

    for it in 1..=req.iterations {
        // 1. Run Evaluation
        let run_name = format!("AutoTune Job {} - Iteration {}", &job_id[..8], it);
        let eval_req = RagEvalRunRequest {
            name: Some(run_name.clone()),
            eval_set: req.eval_set.clone(),
            params: current_params.clone(),
            judge_model: req.judge_model.clone(),
            judge_provider: req.judge_provider.clone(),
            evaluate_generation: true,
            dataset_id: req.dataset_id.clone(),
            dataset_name: req.dataset_name.clone(),
        };

        if let Ok(eval_result) = execute_evaluation_run(uuid::Uuid::new_v4().to_string(), tenant_id.clone(), pool.clone(), eval_req).await {
            let run_id = eval_result["run_id"].as_str().unwrap_or_default().to_string();
            let current_score = eval_result[&target].as_f64().unwrap_or(0.0);
            
            let run_tokens = eval_result["total_prompt_tokens"].as_u64().unwrap_or(0) +
                             eval_result["total_completion_tokens"].as_u64().unwrap_or(0) +
                             eval_result["total_thinking_tokens"].as_u64().unwrap_or(0);
            let run_latency = eval_result["avg_latency_ms"].as_f64().unwrap_or(0.0) as u64;

            accumulated_tokens += run_tokens;

            // Mark base run id
            if it == 1 {
                let _ = sqlx::query("UPDATE rag_auto_tuner_jobs SET base_run_id = ? WHERE id = ?")
                    .bind(&run_id).bind(&job_id).execute(&pool).await;
            }

            // Constraints check
            let meets_min_accuracy = req.min_accuracy.map(|min| current_score >= min).unwrap_or(true);
            let meets_max_latency = req.max_latency.map(|max| run_latency <= max).unwrap_or(true);
            let meets_max_tokens = req.max_tokens_per_run.map(|max| run_tokens <= max).unwrap_or(true);

            // Update best
            if meets_min_accuracy && meets_max_latency && meets_max_tokens {
                if current_score > best_score {
                    best_score = current_score;
                    best_run_id = Some(run_id.clone());
                    no_improvement_count = 0;
                } else {
                    no_improvement_count += 1;
                }
            } else {
                no_improvement_count += 1;
            }

            // Budget exhaustion check
            let budget_exhausted = req.max_token_budget.map(|max| accumulated_tokens >= max).unwrap_or(false);

            // Update progress
            let _ = sqlx::query("UPDATE rag_auto_tuner_jobs SET current_iteration = ?, best_run_id = ? WHERE id = ?")
                .bind(it).bind(&best_run_id).bind(&job_id).execute(&pool).await;

            if budget_exhausted {
                tracing::warn!(job_id = %job_id, "🛑 Auto-Tuner budget exhausted! (Tokens: {})", accumulated_tokens);
                let _ = sqlx::query("UPDATE rag_auto_tuner_jobs SET status = 'budget_exhausted', finished_at = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(&job_id).execute(&pool).await;
                return Ok(());
            }

            // Early stopping: no improvement for EARLY_STOP_PATIENCE iterations
            if no_improvement_count >= EARLY_STOP_PATIENCE {
                tracing::info!(
                    job_id = %job_id,
                    iteration = it,
                    best_score = best_score,
                    "🛑 Auto-Tuner early stopping: no improvement for {} consecutive iterations",
                    EARLY_STOP_PATIENCE
                );
                break;
            }

            if it == req.iterations {
                break; // Last iteration
            }

            // 2. Build Prompt for Next Iteration
            let mut prompt_content = format!(
                "Iteration {} Results:\nTarget Metric {}: {}\nHit Rate: {}\nMRR: {}\n\n",
                it, target, current_score,
                eval_result["hit_rate"].as_f64().unwrap_or(0.0),
                eval_result["mrr"].as_f64().unwrap_or(0.0)
            );

            prompt_content.push_str("Diagnostic Queries:\n");
            if let Some(queries) = eval_result["per_query"].as_array() {
                let mut worst = queries.clone();
                // sort by reciprocal_rank ascending (worst first)
                worst.sort_by(|a, b| {
                    let a_rr = a["reciprocal_rank"].as_f64().unwrap_or(0.0);
                    let b_rr = b["reciprocal_rank"].as_f64().unwrap_or(0.0);
                    a_rr.partial_cmp(&b_rr).unwrap()
                });
                for q in worst.iter().take(5) {
                    prompt_content.push_str(&format!(
                        "- Query: '{}' | Hit: {} | RR: {} | VectorContrib: {} | TreeContrib: {} | GraphContrib: {}\n",
                        q["query"].as_str().unwrap_or(""),
                        q["hit"].as_bool().unwrap_or(false),
                        q["reciprocal_rank"].as_f64().unwrap_or(0.0),
                        q["vector_contributed"].as_bool().unwrap_or(false),
                        q["tree_contributed"].as_bool().unwrap_or(false),
                        q["graph_contributed"].as_bool().unwrap_or(false)
                    ));
                    if let Some(reasoning) = q["judge_reasoning"].as_str() {
                        prompt_content.push_str(&format!("  Judge: {}\n", reasoning));
                    }
                }
            }

            if let Ok(response) = agent.prompt(prompt_content.as_str()).await {
                // Heuristic estimation for tuner agent tokens (since rig::Agent::prompt doesn't expose usage directly):
                let prompt_tokens = (prompt_content.len() + system_prompt.len()) / 4;
                let completion_tokens = response.len() / 4;
                accumulated_tokens += (prompt_tokens + completion_tokens) as u64;

                let json_part = response
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                    
                if let Ok(suggestion) = serde_json::from_str::<TunerIterationResult>(json_part) {
                    tracing::info!("Tuner suggested new params: {:?}", suggestion);
                    current_params.weights.vector = suggestion.suggested_params.weight_vector as f32;
                    current_params.weights.tree = suggestion.suggested_params.weight_tree as f32;
                    current_params.weights.graph = suggestion.suggested_params.weight_graph as f32;
                    current_params.top_k = suggestion.suggested_params.top_k;
                    current_params.vector_alpha = suggestion.suggested_params.vector_alpha;
                    current_params.vector_threshold = suggestion.suggested_params.vector_threshold;
                    // Apply extended params
                    if let Some(hops) = suggestion.suggested_params.graph_hops {
                        current_params.graph_hops = hops.clamp(1, 3);
                    }
                    if let Some(ref strategy) = suggestion.suggested_params.rerank_strategy {
                        if let Some(ref mut rerank) = current_params.rerank {
                            rerank.strategy = strategy.clone();
                            rerank.enabled = true;
                        } else {
                            current_params.rerank = Some(super::rag_eval::RerankConfig {
                                enabled: true,
                                strategy: strategy.clone(),
                                model: None,
                                final_top_k: 5,
                            });
                        }
                    }
                    if let Some(temp) = suggestion.suggested_params.generation_temperature {
                        current_params.generation_temperature = temp.clamp(0.0, 1.0);
                    }
                    if let Some(tokens) = suggestion.suggested_params.generation_max_tokens {
                        current_params.generation_max_tokens = tokens;
                    }
                } else {
                    tracing::warn!("Tuner agent failed to return JSON schema. Reusing current params.");
                }
            }
        }
    }

    let _ = sqlx::query("UPDATE rag_auto_tuner_jobs SET status = 'completed', finished_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&job_id).execute(&pool).await;

    Ok(())
}

/// GET /api/v1/rag-eval/auto-tune/:job_id
pub async fn get_auto_tune_job(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let job: Option<(String, i32, i32, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT status, iterations, current_iteration, base_run_id, best_run_id FROM rag_auto_tuner_jobs WHERE id = ? AND tenant_id = ?"
    )
    .bind(&job_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if let Some((status, iterations, current, base, best)) = job {
        Ok(Json(json!({
            "status": status,
            "iterations": iterations,
            "current_iteration": current,
            "base_run_id": base,
            "best_run_id": best
        })))
    } else {
        Err((StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"}))))
    }
}

// ─── Chat with Overseer ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AutoTuneChatRequest {
    pub message: String,
    pub tuner_provider: Option<String>,
    pub tuner_model: Option<String>,
}

/// POST /api/v1/rag-eval/auto-tune/:job_id/chat
pub async fn auto_tune_chat(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(job_id): Path<String>,
    Json(payload): Json<AutoTuneChatRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    use mimir_core_ai::services::iam::IamService;
    
    let tenant_id = extract_tenant_id(&headers);

    // Get current job state
    let job: Option<(String, i32, i32)> = sqlx::query_as(
        "SELECT status, iterations, current_iteration FROM rag_auto_tuner_jobs WHERE id = ? AND tenant_id = ?"
    )
    .bind(&job_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let (status, total_iters, current_iter) = match job {
        Some(j) => j,
        None => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"})))),
    };

    // Get the latest run (simplified context)
    let latest_run_name = format!("AutoTune Job {}%", &job_id[..8]);
    let latest_run: Option<(String, f64)> = sqlx::query_as(
        "SELECT id, ndcg FROM rag_eval_runs WHERE name LIKE ? ORDER BY created_at DESC LIMIT 1"
    )
    .bind(&latest_run_name)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let mut context_msg = format!("Status: {}. Iteration: {}/{}. ", status, current_iter, total_iters);
    if let Some((_, ndcg)) = latest_run {
        context_msg.push_str(&format!("Latest NDCG: {:.4}.", ndcg));
    }

    // Resolve LLM Client
    let iam = IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    
    let slot = llm_config.resolve_slot("judge", payload.tuner_provider.as_deref(), payload.tuner_model.as_deref());
    
    // We'll use the LlmRouter directly via a UniversalClient if possible, or build basic Rig client
    let provider = slot.provider;
    let model = slot.model;
    
    let api_key = llm_config.heimdall_api_key.unwrap_or_else(|| std::env::var("HEIMDALL_API_KEY").unwrap_or_default());
    let api_base = std::env::var("HEIMDALL_API_URL").unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
    
    let system_prompt = format!(
        r#"You are The Overseer, an autonomous meta-agent tuning a medical RAG pipeline.
The user is asking you a question about the active tuning job.
Current job state: {}
Answer concisely, directly, and use bullet points where helpful.
You cannot change parameters directly via chat yet, but you can advise the user on what strategies work best based on your tuning search so far."#,
        context_msg
    );

    // Call LLM using raw reqwest for simplicity and consistency with our async environment
    let client = reqwest::Client::new();
    let messages = vec![
        json!({"role": "system", "content": system_prompt}),
        json!({"role": "user", "content": payload.message})
    ];

    let resp = client
        .post(format!("{}/chat/completions", api_base.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": messages,
            "temperature": 0.5,
            "max_tokens": 1024
        }))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(json!({"error": e.to_string()}))))?;

    let json_resp: Value = resp.json().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    
    let reply = json_resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No reply generated.")
        .to_string();

    Ok(Json(json!({ "reply": reply })))
}
