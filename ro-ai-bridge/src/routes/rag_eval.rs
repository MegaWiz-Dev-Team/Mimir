//! RAG Evaluation System — Comprehensive parameter-aware evaluation
//!
//! Endpoints:
//! - POST   /api/v1/rag-eval/run               — Run full eval (retrieval + generation)
//! - GET    /api/v1/rag-eval/runs               — List all runs for comparison
//! - GET    /api/v1/rag-eval/runs/:id           — Get run detail + per-query results
//! - POST   /api/v1/rag-eval/runs/:id/deploy    — Deploy winning config to Agent
//! - POST   /api/v1/rag-eval/generate-set       — AI-generate eval set from golden QA
//!
//! Sprint 32: Full Retrieval + Generation evaluation with parameter snapshot

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::retrieval::qdrant::RetrievalResult;
use crate::retrieval::EnsembleWeights;
use crate::routes::search::{run_parallel_search_filtered, SearchFilters};
use crate::routes::search_benchmark::{evaluate_query_results, title_matches};
use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;

// ─── Request / Response Types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RagEvalRunRequest {
    /// Human-readable name for this evaluation run.
    pub name: Option<String>,

    /// Evaluation set: inline items.
    pub eval_set: Vec<RagEvalItem>,

    /// Full parameter snapshot.
    pub params: RagEvalParams,

    /// LLM-as-Judge configuration.
    pub judge_model: Option<String>,
    pub judge_provider: Option<String>,

    /// Whether to also generate answers and judge them.
    #[serde(default = "default_true")]
    pub evaluate_generation: bool,

    /// Optional reference to the saved dataset this was run from
    pub dataset_id: Option<String>,
    pub dataset_name: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalItem {
    pub query: String,
    pub expected_titles: Vec<String>,
    #[serde(default)]
    pub expected_content: Option<String>,
    /// Previous conversation turns for multi-turn context.
    #[serde(default)]
    pub context: Option<Vec<ConversationTurn>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String,   // "user" or "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalParams {
    /// Ensemble weights.
    pub weights: EnsembleWeights,
    /// Top-K results to retrieve.
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Hybrid search alpha (dense vs sparse balance).
    #[serde(default = "default_alpha")]
    pub vector_alpha: f64,
    /// Minimum similarity threshold.
    #[serde(default = "default_threshold")]
    pub vector_threshold: f64,
    /// Knowledge graph neighbor expansion depth.
    #[serde(default = "default_hops")]
    pub graph_hops: i32,
    /// Re-ranking configuration.
    #[serde(default)]
    pub rerank: Option<RerankConfig>,
    /// Source-level filtering.
    #[serde(default)]
    pub filters: Option<SearchFilters>,
    /// Qdrant collections to search.
    #[serde(default)]
    pub collections: Option<Vec<String>>,
}

fn default_top_k() -> usize { 10 }
fn default_alpha() -> f64 { 0.7 }
fn default_threshold() -> f64 { 0.3 }
fn default_hops() -> i32 { 2 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankConfig {
    pub enabled: bool,
    #[serde(default = "default_strategy")]
    pub strategy: String,
    pub model: Option<String>,
    #[serde(default = "default_final_k")]
    pub final_top_k: usize,
}

fn default_strategy() -> String { "rrf".to_string() }
fn default_final_k() -> usize { 5 }

#[derive(Debug, Serialize)]
pub struct RagEvalRunResponse {
    pub run_id: String,
    pub status: String,
    pub total_queries: usize,
    pub hit_rate: f64,
    pub mrr: f64,
    pub ndcg: f64,
    pub precision_at_k: f64,
    pub recall_at_k: f64,
    pub avg_latency_ms: f64,
    pub avg_faithfulness: Option<f64>,
    pub avg_answer_relevancy: Option<f64>,
    pub vector_hit_rate: f64,
    pub tree_hit_rate: f64,
    pub graph_hit_rate: f64,
    pub per_query: Vec<RagEvalQueryResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagEvalQueryResult {
    pub query: String,
    pub hit: bool,
    pub reciprocal_rank: f64,
    pub ndcg_score: f64,
    pub precision: f64,
    pub recall: f64,
    pub matched_at_rank: Option<usize>,
    pub vector_contributed: bool,
    pub tree_contributed: bool,
    pub graph_contributed: bool,
    pub top_results: Vec<TopResultEntry>,
    pub generated_answer: Option<String>,
    pub faithfulness: Option<f64>,
    pub answer_relevancy: Option<f64>,
    pub context_precision: Option<f64>,
    pub judge_reasoning: Option<String>,
    pub retrieval_latency_ms: u64,
    pub generation_latency_ms: Option<u64>,
    pub total_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopResultEntry {
    pub title: String,
    pub score: f32,
    pub source_type: String,
}

/// Query params for listing runs.
#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
}

// ─── Generate Set Request ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GenerateEvalSetV2Request {
    /// Additional prompt instructions for generation.
    pub prompt: String,
    /// Number of questions to generate.
    #[serde(default = "default_count")]
    pub count: usize,
    /// Whether to generate multi-turn conversation contexts.
    #[serde(default)]
    pub multi_turn: bool,
    /// Number of turns per conversation (default: 2)
    #[serde(default = "default_turns")]
    pub turns_per_conversation: usize,
    /// Limit to specific source IDs.
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// LLM to use for generation.
    pub provider: Option<String>,
    pub model_id: Option<String>,
}

fn default_count() -> usize { 5 }
fn default_turns() -> usize { 2 }

// ─── Route Registration ────────────────────────────────────────────────────────

pub fn rag_eval_routes() -> Router<DbPool> {
    Router::new()
        .route("/run", post(run_rag_eval))
        .route("/runs", get(list_rag_eval_runs))
        .route("/runs/{id}", get(get_rag_eval_run).delete(delete_eval_run))
        .route("/runs/{id}/deploy", post(deploy_eval_config))
        .route("/generate-set", post(generate_eval_set_v2))
        .route("/auto-tune", post(super::rag_eval_tuner::run_auto_tune))
        .route("/auto-tune/{job_id}", get(super::rag_eval_tuner::get_auto_tune_job))
        .route("/auto-tune/{job_id}/chat", post(super::rag_eval_tuner::auto_tune_chat))
        .route("/datasets", post(super::rag_eval_dataset::create_dataset).get(super::rag_eval_dataset::list_datasets))
        .route("/datasets/{id}", axum::routing::delete(super::rag_eval_dataset::delete_dataset))
}

// ─── Scoring Functions ─────────────────────────────────────────────────────────

/// Calculate NDCG@K for a single query.
/// Uses binary relevance: 1 if title matches, 0 otherwise.
fn calculate_ndcg_single(results: &[RetrievalResult], expected_titles: &[String], k: usize) -> f64 {
    let top_k = results.iter().take(k).collect::<Vec<_>>();
    if top_k.is_empty() {
        return 0.0;
    }

    // DCG: ∑ rel_i / log2(i+1)
    let dcg: f64 = top_k.iter().enumerate().map(|(i, r)| {
        let rel = if title_matches(&r.title, expected_titles) { 1.0 } else { 0.0 };
        rel / (i as f64 + 2.0).log2()
    }).sum();

    // Ideal DCG: all relevant at top
    let relevant_count = expected_titles.len().min(k);
    let idcg: f64 = (0..relevant_count).map(|i| {
        1.0 / (i as f64 + 2.0).log2()
    }).sum();

    if idcg == 0.0 { 0.0 } else { dcg / idcg }
}

/// Calculate Precision@K: relevant docs in top-K / K
fn calculate_precision(results: &[RetrievalResult], expected_titles: &[String], k: usize) -> f64 {
    let top_k = results.iter().take(k).collect::<Vec<_>>();
    if top_k.is_empty() {
        return 0.0;
    }
    let relevant = top_k.iter().filter(|r| title_matches(&r.title, expected_titles)).count();
    relevant as f64 / k as f64
}

/// Calculate Recall@K: relevant docs found / total relevant docs expected
fn calculate_recall(results: &[RetrievalResult], expected_titles: &[String], k: usize) -> f64 {
    if expected_titles.is_empty() {
        return 0.0;
    }
    let top_k = results.iter().take(k).collect::<Vec<_>>();
    let found_relevant = expected_titles.iter().filter(|exp| {
        top_k.iter().any(|r| title_matches(&r.title, &[exp.to_string()]))
    }).count();
    found_relevant as f64 / expected_titles.len() as f64
}

/// Per-source contribution: did this source type return a matching result?
fn source_contributed(results: &[RetrievalResult], expected_titles: &[String], source_type: &str) -> bool {
    results.iter()
        .filter(|r| r.source_type == source_type)
        .any(|r| title_matches(&r.title, expected_titles))
}

// ─── LLM-as-Judge ──────────────────────────────────────────────────────────────

/// Generate an answer using the RAG context, then judge it.
async fn generate_and_judge(
    query: &str,
    context: &[RetrievalResult],
    expected_content: Option<&str>,
    conversation_context: Option<&[ConversationTurn]>,
    api_base: &str,
    api_key: &str,
    model: &str,
) -> (Option<String>, Option<f64>, Option<f64>, Option<f64>, Option<String>) {
    let context_text: String = context.iter()
        .map(|r| format!("[{}] {}", r.title, r.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    // Build messages with optional multi-turn context
    let mut messages = vec![
        json!({"role": "system", "content": format!(
            "You are a helpful medical assistant. Answer based ONLY on the following context:\n\n{}", context_text
        )}),
    ];

    // Add multi-turn conversation history if present
    if let Some(turns) = conversation_context {
        for turn in turns {
            messages.push(json!({"role": turn.role, "content": turn.content}));
        }
    }

    messages.push(json!({"role": "user", "content": query}));

    // 1. Generate answer
    let client = reqwest::Client::new();
    let gen_resp = client
        .post(format!("{}chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": messages,
            "max_tokens": 1024,
            "temperature": 0.1
        }))
        .send()
        .await;

    let answer = match gen_resp {
        Ok(resp) => {
            let body: Value = resp.json().await.unwrap_or_default();
            body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string()
        }
        Err(e) => {
            warn!("Answer generation failed: {}", e);
            return (None, None, None, None, Some(format!("Generation error: {}", e)));
        }
    };

    if answer.is_empty() {
        return (Some(answer), None, None, None, Some("Empty answer".into()));
    }

    // 2. Judge the answer
    let expected_ref = expected_content
        .map(|c| format!("\n\nExpected answer reference:\n{}", c))
        .unwrap_or_default();

    let judge_prompt = format!(
        r#"You are an expert evaluator for a RAG (Retrieval-Augmented Generation) system.

Evaluate the following answer on these criteria. Score each 0-10.

Question: {query}
Retrieved Context: {context_text}
Generated Answer: {answer}{expected_ref}

Score these dimensions:
1. **Faithfulness** (0-10): Is the answer ONLY based on the retrieved context? No hallucination?
2. **Answer Relevancy** (0-10): Does the answer directly and completely address the question?
3. **Context Precision** (0-10): Were the most relevant context chunks used/cited?

Respond ONLY as JSON:
{{"faithfulness": X, "answer_relevancy": X, "context_precision": X, "reasoning": "..."}}"#
    );

    let judge_resp = client
        .post(format!("{}chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": [
                {"role": "system", "content": "You output only valid JSON."},
                {"role": "user", "content": judge_prompt}
            ],
            "max_tokens": 512,
            "temperature": 0.0
        }))
        .send()
        .await;

    match judge_resp {
        Ok(resp) => {
            let body: Value = resp.json().await.unwrap_or_default();
            let content = body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("{}")
                .to_string();
            let scores: Value = serde_json::from_str(&content).unwrap_or_else(|_| {
                if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
                    if start <= end {
                        let extracted = &content[start..=end];
                        if let Ok(parsed) = serde_json::from_str(extracted) {
                            return parsed;
                        }
                    }
                }
                let cleaned = content
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                serde_json::from_str(cleaned).unwrap_or(json!({}))
            });
            (
                Some(answer),
                scores["faithfulness"].as_f64(),
                scores["answer_relevancy"].as_f64(),
                scores["context_precision"].as_f64(),
                scores["reasoning"].as_str().map(String::from),
            )
        }
        Err(e) => {
            warn!("Judge call failed: {}", e);
            (Some(answer), None, None, None, Some(format!("Judge error: {}", e)))
        }
    }
}

// ─── Handlers ──────────────────────────────────────────────────────────────────

/// POST /api/v1/rag-eval/run — Run full RAG evaluation
pub async fn execute_evaluation_run(
    run_id: String,
    tenant_id: String,
    pool: DbPool,
    payload: RagEvalRunRequest,
) -> Result<Value, String> {
    let start = Instant::now();
    let params = &payload.params;

    // Resolve LLM credentials for judge
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let default_p = tenant_config.as_ref().map(|c| c.default_provider.as_str());
    let default_m = tenant_config.as_ref().map(|c| c.default_model.as_str());
    let slot = llm_config.resolve_slot("judge", default_p, default_m);

    let judge_model = payload.judge_model.unwrap_or(slot.model.clone());
    let judge_provider = payload.judge_provider.unwrap_or(slot.provider.clone());
    let api_base = crate::routes::sources::infer_api_base(&judge_provider);
    let api_key = match judge_provider.to_lowercase().as_str() {
        "google" | "gemini" => llm_config.google_api_key.clone(),
        "openai" => llm_config.openai_api_key.clone(),
        "azure" => llm_config.azure_api_key.clone(),
        _ => llm_config.heimdall_api_key.clone(),
    }
    .unwrap_or_else(|| std::env::var("LLM_API_KEY").unwrap_or_else(|_| "no-key".into()));

    let embed_model = llm_config.resolve_slot("embedding", None, None).model;

    // Insert run record (status = running)
    let rerank_enabled = params.rerank.as_ref().map_or(false, |r| r.enabled);
    let rerank_strategy = params.rerank.as_ref().map(|r| r.strategy.clone()).unwrap_or_default();
    let rerank_model = params.rerank.as_ref().and_then(|r| r.model.clone());
    let rerank_final_k = params.rerank.as_ref().map(|r| r.final_top_k as i32).unwrap_or(5);

    let source_filter_json = params.filters.as_ref()
        .map(|f| serde_json::to_string(f).unwrap_or_default());
    let collections_json = params.collections.as_ref()
        .map(|c| serde_json::to_string(c).unwrap_or_default());

    let _ = sqlx::query(
        r#"INSERT INTO rag_eval_runs
            (id, tenant_id, name, status,
             weight_vector, weight_tree, weight_graph,
             top_k, vector_alpha, vector_threshold, graph_hops,
             rerank_enabled, rerank_strategy, rerank_model, rerank_final_k,
             source_filter, collections, embed_model, judge_model, judge_provider,
             total_queries, dataset_id, dataset_name)
        VALUES (?, ?, ?, 'running',
                ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?)"#
    )
    .bind(&run_id)
    .bind(&tenant_id)
    .bind(&payload.name)
    .bind(params.weights.vector)
    .bind(params.weights.tree)
    .bind(params.weights.graph)
    .bind(params.top_k as i32)
    .bind(params.vector_alpha)
    .bind(params.vector_threshold)
    .bind(params.graph_hops)
    .bind(rerank_enabled as i8)
    .bind(&rerank_strategy)
    .bind(&rerank_model)
    .bind(rerank_final_k)
    .bind(&source_filter_json)
    .bind(&collections_json)
    .bind(&embed_model)
    .bind(&judge_model)
    .bind(&judge_provider)
    .bind(payload.eval_set.len() as i32)
    .bind(&payload.dataset_id)
    .bind(&payload.dataset_name)
    .execute(&pool)
    .await;

    info!(
        event = "rag_eval_start",
        run_id = %run_id,
        queries = payload.eval_set.len(),
        evaluate_generation = payload.evaluate_generation,
        "📊 Starting RAG evaluation run"
    );

    // Run queries in parallel with bounded concurrency for speed
    use futures::stream::{self, StreamExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let filters = params.filters.clone().unwrap_or_default();
    let top_k = params.top_k;

    let total_vector_hits = Arc::new(AtomicU64::new(0));
    let total_tree_hits = Arc::new(AtomicU64::new(0));
    let total_graph_hits = Arc::new(AtomicU64::new(0));
    let total_vector_queries = Arc::new(AtomicU64::new(0));
    let total_tree_queries = Arc::new(AtomicU64::new(0));
    let total_graph_queries = Arc::new(AtomicU64::new(0));

    const EVAL_CONCURRENCY: usize = 5;

    let per_query_results: Vec<RagEvalQueryResult> = stream::iter(payload.eval_set.clone().into_iter())
        .map(|item| {
            let pool = pool.clone();
            let tenant_id = tenant_id.clone();
            let run_id = run_id.clone();
            let filters = filters.clone();
            let weights = params.weights.clone();
            let rerank = params.rerank.clone();
            let api_base = api_base.clone();
            let api_key = api_key.clone();
            let judge_model = judge_model.clone();
            let evaluate_generation = payload.evaluate_generation;

            let vh = total_vector_hits.clone();
            let th = total_tree_hits.clone();
            let gh = total_graph_hits.clone();
            let vq = total_vector_queries.clone();
            let tq = total_tree_queries.clone();
            let gq = total_graph_queries.clone();

            async move {
                let query_start = Instant::now();

                // 1. Retrieval
                let search_results = run_parallel_search_filtered(
                    &pool,
                    &item.query,
                    &tenant_id,
                    &weights,
                    top_k,
                    &filters,
                    rerank.as_ref(),
                )
                .await;

                let retrieval_latency = query_start.elapsed().as_millis() as u64;

                // 2. Retrieval metrics
                let (hit, rr, matched_rank) = evaluate_query_results(&search_results, &item.expected_titles);
                let ndcg = calculate_ndcg_single(&search_results, &item.expected_titles, top_k);
                let precision = calculate_precision(&search_results, &item.expected_titles, top_k);
                let recall = calculate_recall(&search_results, &item.expected_titles, top_k);

                // 3. Per-source contribution
                let v_contrib = source_contributed(&search_results, &item.expected_titles, "vector");
                let t_contrib = source_contributed(&search_results, &item.expected_titles, "tree");
                let g_contrib = source_contributed(&search_results, &item.expected_titles, "graph");

                // Track per-source hit rates (atomic)
                let has_vector = search_results.iter().any(|r| r.source_type == "vector");
                let has_tree = search_results.iter().any(|r| r.source_type == "tree");
                let has_graph = search_results.iter().any(|r| r.source_type == "graph");
                if has_vector { vq.fetch_add(1, Ordering::Relaxed); if v_contrib { vh.fetch_add(1, Ordering::Relaxed); } }
                if has_tree { tq.fetch_add(1, Ordering::Relaxed); if t_contrib { th.fetch_add(1, Ordering::Relaxed); } }
                if has_graph { gq.fetch_add(1, Ordering::Relaxed); if g_contrib { gh.fetch_add(1, Ordering::Relaxed); } }

                // 4. Top results snapshot
                let top_results: Vec<TopResultEntry> = search_results.iter().take(top_k).map(|r| {
                    TopResultEntry {
                        title: r.title.clone(),
                        score: r.score,
                        source_type: r.source_type.clone(),
                    }
                }).collect();

                // 5. Generation + Judge (if enabled)
                let (gen_answer, faithfulness, answer_rel, ctx_prec, judge_reasoning, gen_latency) =
                    if evaluate_generation {
                        let gen_start = Instant::now();
                        let (answer, faith, rel, prec, reasoning) = generate_and_judge(
                            &item.query,
                            &search_results,
                            item.expected_content.as_deref(),
                            item.context.as_deref(),
                            &api_base,
                            &api_key,
                            &judge_model,
                        ).await;
                        let gen_lat = gen_start.elapsed().as_millis() as u64;
                        (answer, faith, rel, prec, reasoning, Some(gen_lat))
                    } else {
                        (None, None, None, None, None, None)
                    };

                let total_latency = query_start.elapsed().as_millis() as u64;

                // 6. Persist per-query result
                let top_results_json = serde_json::to_string(&top_results).unwrap_or_default();
                let _ = sqlx::query(
                    r#"INSERT INTO rag_eval_queries
                        (run_id, tenant_id, query, expected_titles, expected_content,
                         hit, reciprocal_rank, ndcg_score, precision_score, recall_score, matched_at_rank,
                         vector_contributed, tree_contributed, graph_contributed,
                         top_results, generated_answer,
                         faithfulness, answer_relevancy, context_precision, judge_reasoning,
                         retrieval_latency_ms, generation_latency_ms, total_latency_ms)
                    VALUES (?, ?, ?, ?, ?,
                            ?, ?, ?, ?, ?, ?,
                            ?, ?, ?,
                            ?, ?,
                            ?, ?, ?, ?,
                            ?, ?, ?)"#
                )
                .bind(&run_id)
                .bind(&tenant_id)
                .bind(&item.query)
                .bind(serde_json::to_string(&item.expected_titles).unwrap_or_default())
                .bind(&item.expected_content)
                .bind(hit as i8)
                .bind(rr)
                .bind(ndcg)
                .bind(precision)
                .bind(recall)
                .bind(matched_rank.map(|r| r as i32))
                .bind(v_contrib as i8)
                .bind(t_contrib as i8)
                .bind(g_contrib as i8)
                .bind(&top_results_json)
                .bind(&gen_answer)
                .bind(faithfulness)
                .bind(answer_rel)
                .bind(ctx_prec)
                .bind(&judge_reasoning)
                .bind(retrieval_latency as i32)
                .bind(gen_latency.map(|l| l as i32))
                .bind(total_latency as i32)
                .execute(&pool)
                .await;

                RagEvalQueryResult {
                    query: item.query.clone(),
                    hit,
                    reciprocal_rank: rr,
                    ndcg_score: ndcg,
                    precision,
                    recall,
                    matched_at_rank: matched_rank,
                    vector_contributed: v_contrib,
                    tree_contributed: t_contrib,
                    graph_contributed: g_contrib,
                    top_results,
                    generated_answer: gen_answer,
                    faithfulness,
                    answer_relevancy: answer_rel,
                    context_precision: ctx_prec,
                    judge_reasoning,
                    retrieval_latency_ms: retrieval_latency,
                    generation_latency_ms: gen_latency,
                    total_latency_ms: total_latency,
                }
            }
        })
        .buffer_unordered(EVAL_CONCURRENCY)
        .collect()
        .await;

    // Aggregate metrics
    let n = per_query_results.len() as f64;
    let hit_rate = per_query_results.iter().filter(|r| r.hit).count() as f64 / n;
    let mrr = per_query_results.iter().map(|r| r.reciprocal_rank).sum::<f64>() / n;
    let ndcg = per_query_results.iter().map(|r| r.ndcg_score).sum::<f64>() / n;
    let prec = per_query_results.iter().map(|r| r.precision).sum::<f64>() / n;
    let recall = per_query_results.iter().map(|r| r.recall).sum::<f64>() / n;
    let avg_lat = per_query_results.iter().map(|r| r.total_latency_ms as f64).sum::<f64>() / n;

    let avg_faith = if payload.evaluate_generation {
        let vals: Vec<f64> = per_query_results.iter().filter_map(|r| r.faithfulness).collect();
        if vals.is_empty() { None } else { Some(vals.iter().sum::<f64>() / vals.len() as f64) }
    } else { None };

    let avg_ans_rel = if payload.evaluate_generation {
        let vals: Vec<f64> = per_query_results.iter().filter_map(|r| r.answer_relevancy).collect();
        if vals.is_empty() { None } else { Some(vals.iter().sum::<f64>() / vals.len() as f64) }
    } else { None };

    let tvh = total_vector_hits.load(Ordering::Relaxed);
    let tvq = total_vector_queries.load(Ordering::Relaxed);
    let tth = total_tree_hits.load(Ordering::Relaxed);
    let ttq = total_tree_queries.load(Ordering::Relaxed);
    let tgh = total_graph_hits.load(Ordering::Relaxed);
    let tgq = total_graph_queries.load(Ordering::Relaxed);
    let v_hr = if tvq > 0 { tvh as f64 / tvq as f64 } else { 0.0 };
    let t_hr = if ttq > 0 { tth as f64 / ttq as f64 } else { 0.0 };
    let g_hr = if tgq > 0 { tgh as f64 / tgq as f64 } else { 0.0 };

    // Update run record with aggregate scores
    let _ = sqlx::query(
        r#"UPDATE rag_eval_runs SET
            status = 'completed',
            hit_rate = ?, mrr = ?, ndcg = ?,
            precision_at_k = ?, recall_at_k = ?,
            avg_latency_ms = ?,
            avg_faithfulness = ?, avg_answer_relevancy = ?,
            vector_hit_rate = ?, tree_hit_rate = ?, graph_hit_rate = ?,
            finished_at = NOW()
        WHERE id = ?"#
    )
    .bind(hit_rate).bind(mrr).bind(ndcg)
    .bind(prec).bind(recall)
    .bind(avg_lat)
    .bind(avg_faith).bind(avg_ans_rel)
    .bind(v_hr).bind(t_hr).bind(g_hr)
    .bind(&run_id)
    .execute(&pool)
    .await;

    // Persist metrics for trend charts
    let metrics = vec![
        ("hit_rate", hit_rate, "overall"),
        ("mrr", mrr, "overall"),
        ("ndcg", ndcg, "overall"),
        ("precision_at_k", prec, "overall"),
        ("recall_at_k", recall, "overall"),
        ("avg_latency_ms", avg_lat, "overall"),
        ("hit_rate", v_hr, "vector"),
        ("hit_rate", t_hr, "tree"),
        ("hit_rate", g_hr, "graph"),
    ];

    for (name, value, dim) in &metrics {
        let _ = sqlx::query(
            "INSERT INTO rag_eval_metrics (run_id, tenant_id, metric_name, metric_value, dimension) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&run_id).bind(&tenant_id).bind(name).bind(value).bind(dim)
        .execute(&pool)
        .await;
    }

    if let Some(f) = avg_faith {
        let _ = sqlx::query("INSERT INTO rag_eval_metrics (run_id, tenant_id, metric_name, metric_value, dimension) VALUES (?, ?, 'faithfulness', ?, 'overall')")
            .bind(&run_id).bind(&tenant_id).bind(f).execute(&pool).await;
    }
    if let Some(ar) = avg_ans_rel {
        let _ = sqlx::query("INSERT INTO rag_eval_metrics (run_id, tenant_id, metric_name, metric_value, dimension) VALUES (?, ?, 'answer_relevancy', ?, 'overall')")
            .bind(&run_id).bind(&tenant_id).bind(ar).execute(&pool).await;
    }

    let elapsed = start.elapsed().as_millis() as u64;
    info!(
        event = "rag_eval_complete",
        run_id = %run_id,
        hit_rate = hit_rate,
        mrr = mrr,
        ndcg = ndcg,
        total_ms = elapsed,
        "✅ RAG evaluation completed"
    );

    Ok(json!({
        "run_id": run_id,
        "status": "completed",
        "total_queries": per_query_results.len(),
        "hit_rate": hit_rate,
        "mrr": mrr,
        "ndcg": ndcg,
        "precision_at_k": prec,
        "recall_at_k": recall,
        "avg_latency_ms": avg_lat,
        "avg_faithfulness": avg_faith,
        "avg_answer_relevancy": avg_ans_rel,
        "vector_hit_rate": v_hr,
        "tree_hit_rate": t_hr,
        "graph_hit_rate": g_hr,
        "per_query": per_query_results,
        "elapsed_ms": elapsed
    }))
}

/// POST /api/v1/rag-eval/run — Run full RAG evaluation
pub async fn run_rag_eval(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<RagEvalRunRequest>,
) -> impl axum::response::IntoResponse {
    let tenant_id = extract_tenant_id(&headers).to_string();
    let run_id = Uuid::new_v4().to_string();
    
    // Spawn evaluation in background
    let run_id_clone = run_id.clone();
    tokio::spawn(async move {
        if let Err(e) = execute_evaluation_run(run_id_clone.clone(), tenant_id, pool.clone(), payload).await {
            tracing::error!("Background evaluation run failed: {}", e);
            let _ = sqlx::query("UPDATE rag_eval_runs SET status = 'error' WHERE id = ?")
                .bind(run_id_clone)
                .execute(&pool)
                .await;
        }
    });

    (StatusCode::ACCEPTED, Json(json!({
        "run_id": run_id,
        "status": "running",
        "message": "Evaluation started in background"
    })))
}

/// GET /api/v1/rag-eval/runs — List all evaluation runs for comparison
async fn list_rag_eval_runs(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<ListRunsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query(
        r#"SELECT id, name, status,
            weight_vector, weight_tree, weight_graph,
            hit_rate, mrr, ndcg,
            precision_at_k, recall_at_k,
            top_k, avg_latency_ms,
            avg_faithfulness, avg_answer_relevancy,
            vector_hit_rate, tree_hit_rate, graph_hit_rate,
            total_queries, vector_alpha, vector_threshold, graph_hops,
            started_at, finished_at, dataset_id, dataset_name
        FROM rag_eval_runs WHERE tenant_id = ?
        ORDER BY started_at DESC LIMIT ? OFFSET ?"#
    )
    .bind(tenant_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    use sqlx::Row;
    let run_list: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id": r.try_get::<String, _>("id").unwrap_or_default(),
            "name": r.try_get::<Option<String>, _>("name").unwrap_or(None),
            "status": r.try_get::<String, _>("status").unwrap_or_default(),
            "params": {
                "weights": {
                    "vector": r.try_get::<f32, _>("weight_vector").unwrap_or(0.0) as f64,
                    "tree": r.try_get::<f32, _>("weight_tree").unwrap_or(0.0) as f64,
                    "graph": r.try_get::<f32, _>("weight_graph").unwrap_or(0.0) as f64
                },
                "top_k": r.try_get::<i32, _>("top_k").unwrap_or(10),
                "vector_alpha": r.try_get::<Option<f32>, _>("vector_alpha").unwrap_or(None).map(|v| v as f64),
                "vector_threshold": r.try_get::<Option<f32>, _>("vector_threshold").unwrap_or(None).map(|v| v as f64),
                "graph_hops": r.try_get::<Option<i32>, _>("graph_hops").unwrap_or(None)
            },
            "scores": {
                "hit_rate": r.try_get::<Option<f32>, _>("hit_rate").unwrap_or(None).map(|v| v as f64).unwrap_or(0.0),
                "mrr": r.try_get::<Option<f32>, _>("mrr").unwrap_or(None).map(|v| v as f64).unwrap_or(0.0),
                "ndcg": r.try_get::<Option<f32>, _>("ndcg").unwrap_or(None).map(|v| v as f64).unwrap_or(0.0),
                "precision_at_k": r.try_get::<Option<f32>, _>("precision_at_k").unwrap_or(None).map(|v| v as f64).unwrap_or(0.0),
                "recall_at_k": r.try_get::<Option<f32>, _>("recall_at_k").unwrap_or(None).map(|v| v as f64).unwrap_or(0.0),
                "avg_latency_ms": r.try_get::<Option<f32>, _>("avg_latency_ms").unwrap_or(None).map(|v| v as f64).unwrap_or(0.0),
                "faithfulness": r.try_get::<Option<f32>, _>("avg_faithfulness").unwrap_or(None).map(|v| v as f64),
                "answer_relevancy": r.try_get::<Option<f32>, _>("avg_answer_relevancy").unwrap_or(None).map(|v| v as f64),
                "vector_hit_rate": r.try_get::<Option<f32>, _>("vector_hit_rate").unwrap_or(None).map(|v| v as f64),
                "tree_hit_rate": r.try_get::<Option<f32>, _>("tree_hit_rate").unwrap_or(None).map(|v| v as f64),
                "graph_hit_rate": r.try_get::<Option<f32>, _>("graph_hit_rate").unwrap_or(None).map(|v| v as f64)
            },
            "total_queries": r.try_get::<Option<i32>, _>("total_queries").unwrap_or(None),
            "started_at": r.try_get::<Option<chrono::NaiveDateTime>, _>("started_at").unwrap_or(None),
            "finished_at": r.try_get::<Option<chrono::NaiveDateTime>, _>("finished_at").unwrap_or(None),
            "dataset_id": r.try_get::<Option<String>, _>("dataset_id").unwrap_or(None),
            "dataset_name": r.try_get::<Option<String>, _>("dataset_name").unwrap_or(None)
        })
    }).collect();

    Ok(Json(json!({
        "runs": run_list,
        "page": page,
        "per_page": per_page
    })))
}

/// GET /api/v1/rag-eval/runs/:id — Get run detail + per-query results
async fn get_rag_eval_run(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(run_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    use sqlx::Row;

    // Get run metadata
    let run_row = sqlx::query(
        r#"SELECT id, name, status,
            weight_vector, weight_tree, weight_graph,
            top_k, vector_alpha, vector_threshold, graph_hops,
            rerank_enabled, rerank_strategy, rerank_model, rerank_final_k,
            hit_rate, mrr, ndcg,
            precision_at_k, recall_at_k,
            total_queries, COALESCE(avg_latency_ms, 0) as avg_latency_ms,
            avg_faithfulness, avg_answer_relevancy, avg_context_precision,
            embed_model, judge_model,
            started_at, finished_at, dataset_id, dataset_name
        FROM rag_eval_runs WHERE id = ? AND tenant_id = ?"#
    )
    .bind(&run_id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let r = run_row.ok_or((
        StatusCode::NOT_FOUND,
        Json(json!({"error": "Run not found"})),
    ))?;

    // Get per-query results
    let query_rows = sqlx::query(
        r#"SELECT query, expected_titles, expected_content, hit, reciprocal_rank, ndcg_score, precision_score, recall_score, matched_at_rank,
            vector_contributed, tree_contributed, graph_contributed,
            top_results, generated_answer,
            faithfulness, answer_relevancy, context_precision, judge_reasoning,
            retrieval_latency_ms, generation_latency_ms, total_latency_ms
        FROM rag_eval_queries WHERE run_id = ? AND tenant_id = ?
        ORDER BY id"#
    )
    .bind(&run_id)
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let per_query: Vec<Value> = query_rows.iter().map(|q| {
        let top_results_str: Option<String> = q.try_get("top_results").unwrap_or(None);
        let top_results: Value = top_results_str
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(json!([]));
        let expected_titles_str: Option<String> = q.try_get("expected_titles").unwrap_or(None);
        let expected_titles: Value = expected_titles_str
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(json!([]));
        json!({
            "query": q.try_get::<String, _>("query").unwrap_or_default(),
            "expected_titles": expected_titles,
            "expected_content": q.try_get::<Option<String>, _>("expected_content").unwrap_or(None),
            "hit": q.try_get::<Option<i8>, _>("hit").unwrap_or(Some(0)).unwrap_or(0) != 0,
            "reciprocal_rank": q.try_get::<Option<f32>, _>("reciprocal_rank").unwrap_or(Some(0.0)).unwrap_or(0.0) as f64,
            "ndcg_score": q.try_get::<Option<f32>, _>("ndcg_score").unwrap_or(Some(0.0)).unwrap_or(0.0) as f64,
            "precision": q.try_get::<Option<f32>, _>("precision_score").unwrap_or(Some(0.0)).unwrap_or(0.0) as f64,
            "recall": q.try_get::<Option<f32>, _>("recall_score").unwrap_or(Some(0.0)).unwrap_or(0.0) as f64,
            "matched_at_rank": q.try_get::<Option<i32>, _>("matched_at_rank").unwrap_or(None),
            "vector_contributed": q.try_get::<Option<i8>, _>("vector_contributed").unwrap_or(Some(0)).unwrap_or(0) != 0,
            "tree_contributed": q.try_get::<Option<i8>, _>("tree_contributed").unwrap_or(Some(0)).unwrap_or(0) != 0,
            "graph_contributed": q.try_get::<Option<i8>, _>("graph_contributed").unwrap_or(Some(0)).unwrap_or(0) != 0,
            "top_results": top_results,
            "generated_answer": q.try_get::<Option<String>, _>("generated_answer").unwrap_or(None),
            "faithfulness": q.try_get::<Option<f32>, _>("faithfulness").unwrap_or(None).map(|v| v as f64),
            "answer_relevancy": q.try_get::<Option<f32>, _>("answer_relevancy").unwrap_or(None).map(|v| v as f64),
            "context_precision": q.try_get::<Option<f32>, _>("context_precision").unwrap_or(None).map(|v| v as f64),
            "judge_reasoning": q.try_get::<Option<String>, _>("judge_reasoning").unwrap_or(None),
            "retrieval_latency_ms": q.try_get::<Option<i32>, _>("retrieval_latency_ms").unwrap_or(None),
            "generation_latency_ms": q.try_get::<Option<i32>, _>("generation_latency_ms").unwrap_or(None),
            "total_latency_ms": q.try_get::<Option<i32>, _>("total_latency_ms").unwrap_or(None),
        })
    }).collect();

    // ── Bootstrap Confidence Intervals ──────────────────────────────────────
    // Compute 95% CI for key metrics using 1000 bootstrap resamples
    let bootstrap_ci = {
        use std::collections::HashMap;

        let hits: Vec<f64> = query_rows.iter().map(|q| {
            if q.try_get::<Option<i8>, _>("hit").unwrap_or(Some(0)).unwrap_or(0) != 0 { 1.0 } else { 0.0 }
        }).collect();
        let rrs: Vec<f64> = query_rows.iter().map(|q| {
            q.try_get::<Option<f32>, _>("reciprocal_rank").unwrap_or(Some(0.0)).unwrap_or(0.0) as f64
        }).collect();
        let ndcgs: Vec<f64> = query_rows.iter().map(|q| {
            q.try_get::<Option<f32>, _>("ndcg_score").unwrap_or(Some(0.0)).unwrap_or(0.0) as f64
        }).collect();

        fn bootstrap_95ci(values: &[f64], n_resamples: usize) -> (f64, f64, f64) {
            if values.is_empty() {
                return (0.0, 0.0, 0.0);
            }
            let n = values.len();
            // Simple LCG pseudo-random for reproducibility without external crate
            let mut rng_state: u64 = 42;
            let mut means = Vec::with_capacity(n_resamples);
            for _ in 0..n_resamples {
                let mut sum = 0.0;
                for _ in 0..n {
                    rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let idx = (rng_state >> 33) as usize % n;
                    sum += values[idx];
                }
                means.push(sum / n as f64);
            }
            means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let lo = means[n_resamples * 25 / 1000]; // 2.5th percentile
            let hi = means[n_resamples * 975 / 1000]; // 97.5th percentile
            let mean = means.iter().sum::<f64>() / means.len() as f64;
            (mean, lo, hi)
        }

        let (hr_mean, hr_lo, hr_hi) = bootstrap_95ci(&hits, 1000);
        let (mrr_mean, mrr_lo, mrr_hi) = bootstrap_95ci(&rrs, 1000);
        let (ndcg_mean, ndcg_lo, ndcg_hi) = bootstrap_95ci(&ndcgs, 1000);

        json!({
            "hit_rate":  { "mean": hr_mean,   "ci_lower": hr_lo,   "ci_upper": hr_hi },
            "mrr":       { "mean": mrr_mean,  "ci_lower": mrr_lo,  "ci_upper": mrr_hi },
            "ndcg":      { "mean": ndcg_mean, "ci_lower": ndcg_lo, "ci_upper": ndcg_hi },
            "n_queries": hits.len(),
            "n_resamples": 1000,
            "confidence_level": 0.95
        })
    };

    Ok(Json(json!({
        "run": {
            "id": r.get::<String, _>("id"),
            "name": r.try_get::<Option<String>, _>("name").unwrap_or(None),
            "status": r.try_get::<String, _>("status").unwrap_or_default(),
            "params": {
                "weights": {
                    "vector": r.try_get::<f32, _>("weight_vector").unwrap_or(0.0) as f64,
                    "tree": r.try_get::<f32, _>("weight_tree").unwrap_or(0.0) as f64,
                    "graph": r.try_get::<f32, _>("weight_graph").unwrap_or(0.0) as f64
                },
                "top_k": r.try_get::<i32, _>("top_k").unwrap_or(10),
                "vector_alpha": r.try_get::<Option<f32>, _>("vector_alpha").unwrap_or(None).map(|v| v as f64),
                "vector_threshold": r.try_get::<Option<f32>, _>("vector_threshold").unwrap_or(None).map(|v| v as f64),
                "graph_hops": r.try_get::<Option<i32>, _>("graph_hops").unwrap_or(None),
                "rerank": {
                    "enabled": r.try_get::<Option<i8>, _>("rerank_enabled").unwrap_or(Some(0)).unwrap_or(0) != 0,
                    "strategy": r.try_get::<Option<String>, _>("rerank_strategy").unwrap_or(None),
                    "model": r.try_get::<Option<String>, _>("rerank_model").unwrap_or(None),
                    "final_top_k": r.try_get::<Option<i32>, _>("rerank_final_k").unwrap_or(None)
                }
            },
            "scores": {
                "hit_rate": r.try_get::<Option<f32>, _>("hit_rate").unwrap_or(None).map(|v| v as f64),
                "mrr": r.try_get::<Option<f32>, _>("mrr").unwrap_or(None).map(|v| v as f64),
                "ndcg": r.try_get::<Option<f32>, _>("ndcg").unwrap_or(None).map(|v| v as f64),
                "precision_at_k": r.try_get::<Option<f32>, _>("precision_at_k").unwrap_or(None).map(|v| v as f64),
                "recall_at_k": r.try_get::<Option<f32>, _>("recall_at_k").unwrap_or(None).map(|v| v as f64),
                "avg_latency_ms": r.try_get::<Option<f32>, _>("avg_latency_ms").unwrap_or(None).map(|v| v as f64),
                "faithfulness": r.try_get::<Option<f32>, _>("avg_faithfulness").unwrap_or(None).map(|v| v as f64),
                "answer_relevancy": r.try_get::<Option<f32>, _>("avg_answer_relevancy").unwrap_or(None).map(|v| v as f64),
                "context_precision": r.try_get::<Option<f32>, _>("avg_context_precision").unwrap_or(None).map(|v| v as f64)
            },
            "bootstrap_ci": bootstrap_ci,
            "total_queries": r.try_get::<Option<i32>, _>("total_queries").unwrap_or(None),
            "embed_model": r.try_get::<Option<String>, _>("embed_model").unwrap_or(None),
            "judge_model": r.try_get::<Option<String>, _>("judge_model").unwrap_or(None),
            "started_at": r.try_get::<Option<chrono::NaiveDateTime>, _>("started_at").unwrap_or(None),
            "finished_at": r.try_get::<Option<chrono::NaiveDateTime>, _>("finished_at").unwrap_or(None),
            "dataset_id": r.try_get::<Option<String>, _>("dataset_id").unwrap_or(None),
            "dataset_name": r.try_get::<Option<String>, _>("dataset_name").unwrap_or(None)
        },
        "per_query": per_query
    })))
}

/// POST /api/v1/rag-eval/runs/:id/deploy — Deploy winning config to Agent
async fn deploy_eval_config(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(run_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Fetch the run's parameters
    let run: Option<(f32, f32, f32, i32, Option<f32>, Option<f32>, Option<i32>, Option<i8>, Option<String>, Option<String>, Option<i32>)> =
        sqlx::query_as(
            r#"SELECT weight_vector, weight_tree, weight_graph,
                top_k, vector_alpha, vector_threshold, graph_hops,
                rerank_enabled, rerank_strategy, rerank_model, rerank_final_k
            FROM rag_eval_runs WHERE id = ? AND tenant_id = ?"#
        )
        .bind(&run_id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

    let run = run.ok_or((
        StatusCode::NOT_FOUND,
        Json(json!({"error": "Run not found"})),
    ))?;

    // Build the rag_params and rerank_config JSON
    let rag_params = json!({
        "weights": {
            "vector": run.0,
            "tree": run.1,
            "graph": run.2
        },
        "advanced": {
            "top_k": run.3,
            "vector_alpha": run.4,
            "vector_threshold": run.5,
            "graph_hops": run.6
        }
    });

    let rerank_config = json!({
        "enabled": run.7.unwrap_or(0) != 0,
        "strategy": run.8,
        "model": run.9,
        "final_top_k": run.10
    });

    info!(
        event = "eval_deploy",
        run_id = %run_id,
        "🚀 Deploying evaluation config to agent_configs"
    );

    Ok(Json(json!({
        "message": "Config ready to deploy",
        "run_id": run_id,
        "rag_params": rag_params,
        "rerank_config": rerank_config,
        "instructions": "Pass these values to the Agent Studio's RAG configuration panel or call PUT /api/v1/agents/:id with these params"
    })))
}

/// POST /api/v1/rag-eval/generate-set — AI-generate eval set from golden QA
/// Supports multi-turn context by randomly sampling golden QA pairs
async fn generate_eval_set_v2(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<GenerateEvalSetV2Request>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let count = payload.count.min(50);

    // 1. Fetch real golden QA pairs for grounding
    let qa_pairs: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT q.id, q.question, q.answer,
            (SELECT d.name FROM data_sources d
             JOIN chunks c ON c.source_id = d.id
             WHERE c.id = q.chunk_id LIMIT 1) as source_name
        FROM qa_results q WHERE q.tenant_id = ?
        ORDER BY RAND() LIMIT ?"#
    )
    .bind(tenant_id)
    .bind((count * 3) as i32) // fetch more for variety
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // 2. Fetch source titles
    let sources: Vec<(i64, String)> = if let Some(ref ids) = payload.source_ids {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let q = format!("SELECT id, name FROM data_sources WHERE tenant_id = ? AND id IN ({})", placeholders);
        let mut query = sqlx::query_as::<_, (i64, String)>(&q).bind(tenant_id);
        for id in ids {
            query = query.bind(id);
        }
        query.fetch_all(&pool).await.unwrap_or_default()
    } else {
        sqlx::query_as("SELECT id, name FROM data_sources WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    };

    if sources.is_empty() && qa_pairs.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No data sources or QA pairs found"})),
        ));
    }

    let titles: Vec<String> = sources.iter().map(|(_, n)| n.clone()).collect();
    let qa_samples: String = qa_pairs.iter().take(10).map(|(_, q, a, src)| {
        format!("Q: {} | A: {} | Source: {}", q, a, src.as_deref().unwrap_or("unknown"))
    }).collect::<Vec<_>>().join("\n");

    // 3. Resolve LLM
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let slot = llm_config.resolve_slot("judge",
        tenant_config.as_ref().map(|c| c.default_provider.as_str()),
        tenant_config.as_ref().map(|c| c.default_model.as_str()),
    );
    let model_id = payload.model_id.unwrap_or(slot.model.clone());
    let provider = payload.provider.unwrap_or(slot.provider.clone());
    let api_base = crate::routes::sources::infer_api_base(&provider);
    let api_key = match provider.to_lowercase().as_str() {
        "google" | "gemini" => llm_config.google_api_key.clone(),
        "openai" => llm_config.openai_api_key.clone(),
        "azure" => llm_config.azure_api_key.clone(),
        _ => llm_config.heimdall_api_key.clone(),
    }
    .unwrap_or_else(|| std::env::var("LLM_API_KEY").unwrap_or_else(|_| "no-key".into()));

    // 4. Build prompt
    let multi_turn_instruction = if payload.multi_turn {
        format!(
            r#"
IMPORTANT: Generate MULTI-TURN conversation evaluation items.
Each item must have a "context" field with {turns} previous conversation turns
that naturally lead to the evaluation question.
Use the golden QA samples below to create realistic follow-up questions.
The context should be a natural conversation where the user asks related questions.

Golden QA samples for reference:
{qa_samples}
"#,
            turns = payload.turns_per_conversation,
            qa_samples = qa_samples,
        )
    } else {
        String::new()
    };

    let system_prompt = format!(
        r#"You are an evaluation set generator for a medical RAG system.
Generate exactly {count} evaluation items as a JSON array.

Available document titles: {titles}
{multi_turn}
Each item MUST follow this structure:
{{
  "query": "Natural question a user would ask",
  "expected_titles": ["Exact title from the list"],
  "expected_content": "Brief expected answer"{context_field}
}}

Rules:
1. expected_titles MUST be exact matches from the document titles.
2. Rephrase questions naturally - don't copy QA pairs verbatim.
3. Questions should test different retrieval strategies (keyword, semantic, multi-hop).
4. User instructions: {prompt}
5. Output ONLY the JSON array."#,
        count = count,
        titles = titles.join(", "),
        multi_turn = multi_turn_instruction,
        prompt = payload.prompt,
        context_field = if payload.multi_turn {
            ",\n  \"context\": [{\"role\": \"user\", \"content\": \"...\"}, {\"role\": \"assistant\", \"content\": \"...\"}]"
        } else { "" },
    );

    // 5. Call LLM
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model_id,
            "messages": [
                {"role": "system", "content": "You output only valid JSON arrays."},
                {"role": "user", "content": system_prompt}
            ],
            "max_tokens": 8192,
            "temperature": 0.3
        }))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("LLM error: {}", e)}))))?;

    let resp_json: Value = resp.json().await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Parse error: {}", e)})),
    ))?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[]")
        .to_string();

    let eval_set: Value = serde_json::from_str(&content).unwrap_or_else(|_| {
        // Try to extract JSON array if there's markdown or conversational wrapper text
        if let (Some(start), Some(end)) = (content.find('['), content.rfind(']')) {
            if start <= end {
                let extracted = &content[start..=end];
                if let Ok(parsed) = serde_json::from_str(extracted) {
                    return parsed;
                }
            }
        }
        
        // Fallback to basic markdown block trimming
        let cleaned = content.trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        serde_json::from_str(cleaned).unwrap_or(json!([]))
    });

    info!(
        event = "eval_set_v2_generated",
        count = count,
        multi_turn = payload.multi_turn,
        model = %model_id,
        "AI evaluation set generated"
    );

    Ok(Json(json!({
        "eval_set": eval_set,
        "model_used": model_id,
        "available_titles": titles,
        "qa_samples_used": qa_pairs.len(),
        "multi_turn": payload.multi_turn,
        "count_requested": count
    })))
}

/// DELETE /api/v1/rag-eval/runs/:id — Delete an evaluation run and its results
pub async fn delete_eval_run(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers);

    match sqlx::query("DELETE FROM rag_eval_runs WHERE id = ? AND tenant_id = ?")
        .bind(&id)
        .bind(&tenant_id)
        .execute(&pool)
        .await
    {
        Ok(_) => {
            (StatusCode::OK, Json(json!({ "message": "Evaluation run deleted successfully" })))
        }
        Err(e) => {
            error!(event = "eval_run_delete_failed", error = %e, run_id = %id, "Failed to delete eval run");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to delete evaluation run: {}", e) })),
            )
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ndcg_perfect_ranking() {
        let results = vec![
            RetrievalResult {
                title: "Drug A".into(), content: "".into(),
                score: 0.9, source_type: "vector".into(), metadata: json!({}),
            },
        ];
        let ndcg = calculate_ndcg_single(&results, &["Drug A".to_string()], 5);
        assert!(ndcg > 0.99, "Perfect hit at rank 1 should give NDCG ≈ 1.0");
    }

    #[test]
    fn test_ndcg_no_match() {
        let results = vec![
            RetrievalResult {
                title: "Drug B".into(), content: "".into(),
                score: 0.9, source_type: "vector".into(), metadata: json!({}),
            },
        ];
        let ndcg = calculate_ndcg_single(&results, &["Drug A".to_string()], 5);
        assert!(ndcg < 0.01, "No match should give NDCG ≈ 0");
    }

    #[test]
    fn test_precision_at_k() {
        let results = vec![
            RetrievalResult { title: "Drug A".into(), content: "".into(), score: 0.9, source_type: "vector".into(), metadata: json!({}) },
            RetrievalResult { title: "Drug B".into(), content: "".into(), score: 0.8, source_type: "vector".into(), metadata: json!({}) },
            RetrievalResult { title: "Drug C".into(), content: "".into(), score: 0.7, source_type: "vector".into(), metadata: json!({}) },
        ];
        // 1 out of 3 relevant
        let prec = calculate_precision(&results, &["Drug A".to_string()], 3);
        assert!((prec - 1.0/3.0).abs() < 0.01);
    }

    #[test]
    fn test_recall_at_k() {
        let results = vec![
            RetrievalResult { title: "Drug A".into(), content: "".into(), score: 0.9, source_type: "vector".into(), metadata: json!({}) },
            RetrievalResult { title: "Drug C".into(), content: "".into(), score: 0.7, source_type: "vector".into(), metadata: json!({}) },
        ];
        // Expected: [Drug A, Drug B], found: [Drug A] → recall = 1/2
        let recall = calculate_recall(&results, &["Drug A".to_string(), "Drug B".to_string()], 5);
        assert!((recall - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_source_contribution() {
        let results = vec![
            RetrievalResult { title: "Drug A".into(), content: "".into(), score: 0.9, source_type: "vector".into(), metadata: json!({}) },
            RetrievalResult { title: "Drug A".into(), content: "".into(), score: 0.5, source_type: "graph".into(), metadata: json!({}) },
        ];
        assert!(source_contributed(&results, &["Drug A".to_string()], "vector"));
        assert!(source_contributed(&results, &["Drug A".to_string()], "graph"));
        assert!(!source_contributed(&results, &["Drug A".to_string()], "tree"));
    }

    #[test]
    fn test_eval_item_deserialization() {
        let json = r#"{"query":"test?","expected_titles":["Doc A"],"context":[{"role":"user","content":"hello"},{"role":"assistant","content":"hi"}]}"#;
        let item: RagEvalItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.query, "test?");
        assert!(item.context.is_some());
        assert_eq!(item.context.unwrap().len(), 2);
    }
}
