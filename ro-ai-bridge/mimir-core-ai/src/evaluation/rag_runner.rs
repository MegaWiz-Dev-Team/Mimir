//! RAG Evaluation Runner
//!
//! Evaluates retrieval quality (hit_rate, MRR, NDCG, precision@k, recall@k)
//! and generation quality (faithfulness, answer relevancy) for the RAG pipeline.
//!
//! Flow per query:
//!   1. Embed query via Heimdall
//!   2. Search Qdrant for top-k documents
//!   3. Calculate retrieval metrics against expected_titles
//!   4. Generate answer using retrieved context + LLM
//!   5. Judge faithfulness and answer relevancy via LLM-as-Judge
//!   6. Persist per-query results and aggregate into rag_eval_runs

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::services::db::DbPool;
use crate::services::llm_router::LlmRouter;
use crate::services::qdrant::QdrantService;

// ─── Public API ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RagEvalParams {
    pub tenant_id: String,
    pub dataset_id: String,
    pub top_k: Option<usize>,
    pub collection: Option<String>,
    pub search_provider: Option<String>,
    pub search_model: Option<String>,
    pub generation_provider: Option<String>,
    pub generation_model: Option<String>,
}

/// Start a RAG evaluation run. Returns `run_id` immediately.
pub async fn start_rag_eval_run(pool: DbPool, params: RagEvalParams) -> Result<String> {
    let run_id = Uuid::new_v4().to_string();
    let top_k = params.top_k.unwrap_or(5);

    sqlx::query(
        "INSERT INTO rag_eval_runs
             (id, tenant_id, name, status,
              search_provider, search_model, generation_provider, generation_model, dataset_id)
         VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(&params.tenant_id)
    .bind(format!(
        "RAG Eval {} ({})",
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        &params.tenant_id
    ))
    .bind(params.search_provider.as_deref().unwrap_or("heimdall"))
    .bind(params.search_model.as_deref().unwrap_or("default"))
    .bind(params.generation_provider.as_deref().unwrap_or("heimdall"))
    .bind(params.generation_model.as_deref().unwrap_or("default"))
    .bind(&params.dataset_id)
    .execute(&pool)
    .await?;

    let run_id_clone = run_id.clone();
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        if let Err(e) =
            run_rag_eval_task(pool_clone.clone(), run_id_clone.clone(), params, top_k).await
        {
            error!("RAG eval task {} failed: {}", run_id_clone, e);
            let _ = sqlx::query(
                "UPDATE rag_eval_runs SET status = 'failed', finished_at = NOW() WHERE id = ?",
            )
            .bind(&run_id_clone)
            .execute(&pool_clone)
            .await;
        }
    });

    Ok(run_id)
}

// ─── Dataset item ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct EvalItem {
    question: String,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    expected_titles: Vec<String>,
}

// ─── Main task ────────────────────────────────────────────────────────────────

async fn run_rag_eval_task(
    pool: DbPool,
    run_id: String,
    params: RagEvalParams,
    top_k: usize,
) -> Result<()> {
    info!("🚀 RAG eval run {} started", run_id);

    sqlx::query("UPDATE rag_eval_runs SET status = 'running' WHERE id = ?")
        .bind(&run_id)
        .execute(&pool)
        .await?;

    let router = LlmRouter::new(pool.clone(), &params.tenant_id)
        .await
        .context("Failed to build LlmRouter")?;

    let qdrant = QdrantService::new();

    // Load dataset
    let (eval_set_json, dataset_name): (String, String) = sqlx::query_as(
        "SELECT eval_set, name FROM rag_eval_datasets WHERE id = ? AND tenant_id = ?",
    )
    .bind(&params.dataset_id)
    .bind(&params.tenant_id)
    .fetch_one(&pool)
    .await
    .context("Dataset not found")?;

    let items: Vec<EvalItem> =
        serde_json::from_str(&eval_set_json).context("Failed to parse eval_set JSON")?;

    info!("📋 Dataset '{}': {} items, top_k={}", dataset_name, items.len(), top_k);

    let collection = params
        .collection
        .as_deref()
        .unwrap_or(crate::rag_engine::COLLECTION_WIKI_QA);

    let mut total_hit = 0.0_f64;
    let mut total_rr = 0.0_f64;
    let mut total_ndcg = 0.0_f64;
    let mut total_precision = 0.0_f64;
    let mut total_recall = 0.0_f64;
    let mut total_faithfulness = 0.0_f64;
    let mut total_relevancy = 0.0_f64;
    let mut total_prompt_tokens: i64 = 0;
    let mut total_completion_tokens: i64 = 0;

    for item in &items {
        match eval_single_query(
            &pool, &router, &qdrant, &run_id, &params.tenant_id,
            item, collection, top_k,
        )
        .await
        {
            Ok(metrics) => {
                total_hit += metrics.hit as i32 as f64;
                total_rr += metrics.reciprocal_rank;
                total_ndcg += metrics.ndcg;
                total_precision += metrics.precision_at_k;
                total_recall += metrics.recall_at_k;
                total_faithfulness += metrics.faithfulness;
                total_relevancy += metrics.answer_relevancy;
                total_prompt_tokens += metrics.prompt_tokens as i64;
                total_completion_tokens += metrics.completion_tokens as i64;
            }
            Err(e) => warn!("Query eval failed for '{}': {}", item.question, e),
        }
    }

    let n = items.len() as f64;
    if n > 0.0 {
        sqlx::query(
            "UPDATE rag_eval_runs
             SET status = 'completed', finished_at = NOW(),
                 dataset_name     = ?,
                 hit_rate         = ?,
                 mrr              = ?,
                 ndcg             = ?,
                 precision_at_k   = ?,
                 recall_at_k      = ?,
                 avg_faithfulness = ?,
                 avg_answer_relevancy = ?,
                 total_prompt_tokens     = ?,
                 total_completion_tokens = ?
             WHERE id = ?",
        )
        .bind(&dataset_name)
        .bind(total_hit / n)
        .bind(total_rr / n)
        .bind(total_ndcg / n)
        .bind(total_precision / n)
        .bind(total_recall / n)
        .bind(total_faithfulness / n)
        .bind(total_relevancy / n)
        .bind(total_prompt_tokens as i32)
        .bind(total_completion_tokens as i32)
        .bind(&run_id)
        .execute(&pool)
        .await?;
    }

    info!(
        "✅ RAG eval {} done — hit_rate={:.3} MRR={:.3} NDCG={:.3}",
        run_id,
        total_hit / n.max(1.0),
        total_rr / n.max(1.0),
        total_ndcg / n.max(1.0)
    );
    Ok(())
}

// ─── Per-query evaluation ─────────────────────────────────────────────────────

struct QueryMetrics {
    hit: bool,
    reciprocal_rank: f64,
    ndcg: f64,
    precision_at_k: f64,
    recall_at_k: f64,
    faithfulness: f64,
    answer_relevancy: f64,
    prompt_tokens: i32,
    completion_tokens: i32,
}

async fn eval_single_query(
    pool: &DbPool,
    router: &LlmRouter,
    qdrant: &QdrantService,
    run_id: &str,
    tenant_id: &str,
    item: &EvalItem,
    collection: &str,
    top_k: usize,
) -> Result<QueryMetrics> {
    // 1. Embed query
    let t_retrieval = Instant::now();
    let embeddings = router
        .embed_query(&item.question)
        .await
        .context("Embedding failed")?;

    // 2. Retrieve top-k from Qdrant
    let search_result = qdrant
        .search(collection, embeddings, top_k, tenant_id, false)
        .await
        .context("Qdrant search failed")?;

    let retrieved_titles = extract_titles(&search_result);
    let retrieved_snippets = extract_snippets(&search_result);
    let retrieval_latency = t_retrieval.elapsed().as_millis() as i32;

    // 3. Retrieval metrics
    let metrics = compute_retrieval_metrics(&item.expected_titles, &retrieved_titles, top_k);

    // 4. Generate answer
    let t_gen = Instant::now();
    let context = retrieved_snippets.join("\n\n");
    let gen_prompt = format!(
        "Using the following context, answer the question.\n\nContext:\n{}\n\nQuestion: {}",
        context, item.question
    );
    let (client, model) = router.resolve_client("rag")?;
    let generated_answer = client
        .prompt(&model, "You are a helpful assistant. Answer based on context only.", &gen_prompt, 1024, 0.1)
        .await
        .unwrap_or_else(|e| format!("[GEN ERROR] {}", e));
    let gen_latency = t_gen.elapsed().as_millis() as i32;

    // 5. LLM judge: faithfulness + answer relevancy
    let (faithfulness, answer_relevancy, judge_reasoning, pt, ct) =
        judge_rag_response(router, &item.question, &item.answer, &generated_answer, &context)
            .await
            .unwrap_or((0.0, 0.0, "[JUDGE ERROR]".to_string(), 0, 0));

    // 6. Persist
    sqlx::query(
        "INSERT INTO rag_eval_queries
             (run_id, tenant_id, query, expected_titles, expected_content,
              hit, reciprocal_rank, ndcg_score,
              generated_answer, faithfulness, answer_relevancy,
              judge_reasoning,
              retrieval_latency_ms, generation_latency_ms, total_latency_ms,
              prompt_tokens, completion_tokens)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(run_id)
    .bind(tenant_id)
    .bind(&item.question)
    .bind(serde_json::to_string(&item.expected_titles).unwrap_or_default())
    .bind(&item.answer)
    .bind(metrics.hit)
    .bind(metrics.reciprocal_rank)
    .bind(metrics.ndcg)
    .bind(&generated_answer)
    .bind(faithfulness)
    .bind(answer_relevancy)
    .bind(&judge_reasoning)
    .bind(retrieval_latency)
    .bind(gen_latency)
    .bind(retrieval_latency + gen_latency)
    .bind(pt)
    .bind(ct)
    .execute(pool)
    .await?;

    Ok(QueryMetrics {
        hit: metrics.hit,
        reciprocal_rank: metrics.reciprocal_rank,
        ndcg: metrics.ndcg,
        precision_at_k: metrics.precision_at_k,
        recall_at_k: metrics.recall_at_k,
        faithfulness,
        answer_relevancy,
        prompt_tokens: pt,
        completion_tokens: ct,
    })
}

// ─── Retrieval metrics ────────────────────────────────────────────────────────

struct RetrievalMetrics {
    hit: bool,
    reciprocal_rank: f64,
    ndcg: f64,
    precision_at_k: f64,
    recall_at_k: f64,
}

fn compute_retrieval_metrics(
    expected: &[String],
    retrieved: &[String],
    k: usize,
) -> RetrievalMetrics {
    if expected.is_empty() {
        return RetrievalMetrics {
            hit: false,
            reciprocal_rank: 0.0,
            ndcg: 0.0,
            precision_at_k: 0.0,
            recall_at_k: 0.0,
        };
    }

    let expected_lower: Vec<String> = expected.iter().map(|s| s.to_lowercase()).collect();
    let top_k: Vec<String> = retrieved.iter().take(k).map(|s| s.to_lowercase()).collect();

    // Hit: at least one expected in top-k
    let hit = top_k.iter().any(|r| expected_lower.iter().any(|e| r.contains(e.as_str()) || e.contains(r.as_str())));

    // Reciprocal rank: 1 / rank of first relevant result
    let rr = top_k
        .iter()
        .enumerate()
        .find(|(_, r)| expected_lower.iter().any(|e| r.contains(e.as_str()) || e.contains(r.as_str())))
        .map(|(i, _)| 1.0 / (i + 1) as f64)
        .unwrap_or(0.0);

    // NDCG
    let dcg: f64 = top_k
        .iter()
        .enumerate()
        .filter(|(_, r)| expected_lower.iter().any(|e| r.contains(e.as_str()) || e.contains(r.as_str())))
        .map(|(i, _)| 1.0 / (i as f64 + 2.0).log2())
        .sum();
    let ideal_hits = expected_lower.len().min(k);
    let idcg: f64 = (0..ideal_hits)
        .map(|i| 1.0 / (i as f64 + 2.0).log2())
        .sum();
    let ndcg = if idcg > 0.0 { dcg / idcg } else { 0.0 };

    // Precision@k
    let relevant_in_topk = top_k
        .iter()
        .filter(|r| expected_lower.iter().any(|e| r.contains(e.as_str()) || e.contains(r.as_str())))
        .count();
    let precision_at_k = relevant_in_topk as f64 / k as f64;
    let recall_at_k = relevant_in_topk as f64 / expected_lower.len() as f64;

    RetrievalMetrics { hit, reciprocal_rank: rr, ndcg, precision_at_k, recall_at_k }
}

// ─── RAG judge ────────────────────────────────────────────────────────────────

const RAG_JUDGE_SYSTEM: &str = r#"You are a RAG evaluation judge. Score the generated answer on:
- faithfulness (0.0–1.0): Is the answer grounded in the provided context? Penalise hallucinations.
- answer_relevancy (0.0–1.0): Does the answer address the question?

Return ONLY valid JSON, no markdown:
{"faithfulness": <float>, "answer_relevancy": <float>, "reasoning": "<1 sentence>"}"#;

async fn judge_rag_response(
    router: &LlmRouter,
    question: &str,
    expected: &str,
    generated: &str,
    context: &str,
) -> Result<(f64, f64, String, i32, i32)> {
    let (client, model) = router.resolve_client("judge")?;
    let input = format!(
        "Question: {}\nExpected: {}\nContext:\n{}\nGenerated answer: {}",
        question,
        expected,
        &context[..context.len().min(2000)],
        generated
    );

    let raw = client.prompt(&model, RAG_JUDGE_SYSTEM, &input, 256, 0.0).await?;

    let json_str = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let v: serde_json::Value = serde_json::from_str(json_str)?;

    let faithfulness = v["faithfulness"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
    let relevancy = v["answer_relevancy"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
    let reasoning = v["reasoning"].as_str().unwrap_or("").to_string();

    // Rough token estimate
    let pt = ((input.len()) / 4) as i32;
    let ct = (raw.len() / 4) as i32;

    Ok((faithfulness, relevancy, reasoning, pt, ct))
}

// ─── Qdrant result parsing ────────────────────────────────────────────────────

fn extract_titles(result: &serde_json::Value) -> Vec<String> {
    result["result"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|pt| {
            pt["payload"]["title"]
                .as_str()
                .or_else(|| pt["payload"]["source_id"].as_str())
                .or_else(|| pt["payload"]["name"].as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

fn extract_snippets(result: &serde_json::Value) -> Vec<String> {
    result["result"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|pt| {
            pt["payload"]["content"]
                .as_str()
                .or_else(|| pt["payload"]["text"].as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

// ─── LlmRouter embed helper ───────────────────────────────────────────────────

impl LlmRouter {
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        let texts = vec![text.to_string()];
        let mut embeddings = self.embed_texts_strict(&texts).await?;
        embeddings.pop().context("Embedding returned empty result")
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hit_rate_found() {
        let expected = vec!["Ragnarok Online".to_string()];
        let retrieved = vec!["Ragnarok Online Wiki".to_string(), "Other".to_string()];
        let m = compute_retrieval_metrics(&expected, &retrieved, 5);
        assert!(m.hit);
        assert!((m.reciprocal_rank - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_hit_rate_miss() {
        let expected = vec!["Ragnarok Online".to_string()];
        let retrieved = vec!["Something else".to_string()];
        let m = compute_retrieval_metrics(&expected, &retrieved, 5);
        assert!(!m.hit);
        assert_eq!(m.reciprocal_rank, 0.0);
    }

    #[test]
    fn test_mrr_second_rank() {
        let expected = vec!["Target".to_string()];
        let retrieved = vec!["Wrong".to_string(), "Target Doc".to_string(), "Other".to_string()];
        let m = compute_retrieval_metrics(&expected, &retrieved, 5);
        assert!((m.reciprocal_rank - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_ndcg_perfect() {
        let expected = vec!["A".to_string()];
        let retrieved = vec!["A doc".to_string()];
        let m = compute_retrieval_metrics(&expected, &retrieved, 5);
        assert!((m.ndcg - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_empty_expected() {
        let m = compute_retrieval_metrics(&[], &["A".to_string()], 5);
        assert!(!m.hit);
        assert_eq!(m.ndcg, 0.0);
    }
}
