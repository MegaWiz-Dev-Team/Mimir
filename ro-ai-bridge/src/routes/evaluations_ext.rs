//! Extended Evaluation API — Model performance evaluation and comparison
//!
//! Endpoints:
//! - POST   /api/v1/evaluations/run              — run evaluation batch
//! - POST   /api/v1/evaluations/generate-set      — AI-generate evaluation set
//! - GET    /api/v1/evaluations/results           — get evaluation results
//! - GET    /api/v1/evaluations/compare           — A/B model comparison
//! - GET    /api/v1/evaluations/feedback-summary  — user feedback aggregation

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::agents::eval::{evaluate_agent, judge_response};
use mimir_core_ai::services::db::DbPool;

// ─── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RunEvalRequest {
    pub agent_name: Option<String>,
    pub agent_config_id: Option<i64>,
    pub models: Vec<String>,
    pub questions: Vec<EvalQuestion>,
    pub judge_model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EvalQuestion {
    pub question: String,
    pub expected_answer: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EvalReportRow {
    pub id: i64,
    pub tenant_id: String,
    pub agent_config_id: Option<i64>,
    pub model_id: String,
    pub question: String,
    pub expected_answer: Option<String>,
    pub actual_answer: Option<String>,
    pub accuracy: Option<i32>,
    pub completeness: Option<i32>,
    pub relevance: Option<i32>,
    pub reasoning: Option<String>,
    pub latency_ms: Option<i32>,
    pub batch_id: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct ResultsQuery {
    pub batch_id: Option<String>,
    pub model_id: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    pub model_a: String,
    pub model_b: String,
    pub batch_id: Option<String>,
}

// ─── Routes ─────────────────────────────────────────────────────────────────────

pub fn evaluations_ext_routes() -> Router<DbPool> {
    Router::new()
        .route("/run", post(run_evaluation_batch))
        .route("/generate-set", post(generate_eval_set))
        .route("/results", get(get_eval_results))
        .route("/compare", get(compare_models))
        .route("/feedback-summary", get(feedback_summary))
        // Sprint 27: Extraction & RAG evaluation
        .route("/extraction-summary", get(extraction_summary))
        .route("/extraction-compare", get(extraction_compare))
        .route("/retrieval-summary", get(retrieval_summary))
        // Sprint 28: E2E Pipeline evaluation
        .route("/pipeline-scorecard", get(pipeline_scorecard))
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// POST /api/v1/evaluations/run — Run evaluation batch
async fn run_evaluation_batch(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<RunEvalRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let batch_id = Uuid::new_v4().to_string();

    // Resolve judge model from tenant config (llm_config.judge slot)
    let iam = mimir_core_ai::services::iam::IamService::new_with_env(pool.clone());
    let tenant_config = iam.get_tenant_config(&tenant_id).await.ok();
    let llm_config = tenant_config
        .as_ref()
        .and_then(|c| c.llm_config.as_ref())
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let default_p = tenant_config.as_ref().map(|c| c.default_provider.as_str());
    let default_m = tenant_config.as_ref().map(|c| c.default_model.as_str());
    let judge_slot = llm_config.resolve_slot("judge", default_p, default_m);
    let judge_model = payload.judge_model.unwrap_or(judge_slot.model);
    let agent_name = payload.agent_name.unwrap_or_else(|| "oracle_rag".into());

    info!(
        "Starting evaluation batch {} with {} models x {} questions",
        batch_id,
        payload.models.len(),
        payload.questions.len()
    );

    let mut results: Vec<Value> = vec![];
    let mut total_scored = 0;

    for model_id in &payload.models {
        for q in &payload.questions {
            // Run evaluation
            let eval_result = evaluate_agent(
                &agent_name,
                model_id,
                &q.question,
                Some(&pool),
                None, // No Qdrant in batch mode
            )
            .await;

            let (actual_answer, latency_ms, error_msg) = match eval_result {
                Ok(r) => (Some(r.answer), r.latency_ms as i32, None),
                Err(e) => {
                    warn!(
                        "Eval failed for model {} question '{}': {}",
                        model_id, q.question, e
                    );
                    (None, 0, Some(e.to_string()))
                }
            };

            // Judge response if we have an answer and expected answer
            let (accuracy, completeness, relevance, reasoning) = if let (
                Some(ref answer),
                Some(ref expected),
            ) =
                (&actual_answer, &q.expected_answer)
            {
                match judge_response(&q.question, expected, answer, &judge_model, None).await {
                    Ok(scores) => {
                        total_scored += 1;
                        (
                            scores.accuracy as i32,
                            scores.completeness as i32,
                            scores.relevance as i32,
                            Some(scores.reasoning),
                        )
                    }
                    Err(e) => {
                        warn!("Judge failed: {}", e);
                        (0, 0, 0, Some(format!("Judge error: {}", e)))
                    }
                }
            } else {
                (0, 0, 0, error_msg.map(|e| format!("Eval error: {}", e)))
            };

            // Insert into evaluation_reports
            let _ = sqlx::query(
                r#"INSERT INTO evaluation_reports
                    (tenant_id, agent_config_id, model_id, question, expected_answer, actual_answer,
                     accuracy, completeness, relevance, reasoning, latency_ms, batch_id)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(tenant_id)
            .bind(payload.agent_config_id)
            .bind(model_id)
            .bind(&q.question)
            .bind(&q.expected_answer)
            .bind(&actual_answer)
            .bind(accuracy)
            .bind(completeness)
            .bind(relevance)
            .bind(&reasoning)
            .bind(latency_ms)
            .bind(&batch_id)
            .execute(&pool)
            .await;

            results.push(json!({
                "model_id": model_id,
                "question": q.question,
                "accuracy": accuracy,
                "completeness": completeness,
                "relevance": relevance,
                "latency_ms": latency_ms
            }));
        }
    }

    info!(
        "Evaluation batch {} completed: {} results, {} scored",
        batch_id,
        results.len(),
        total_scored
    );

    Ok(Json(json!({
        "batch_id": batch_id,
        "total_evaluations": results.len(),
        "total_scored": total_scored,
        "results": results
    })))
}

// ─── AI Evaluation Set Generator ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GenerateEvalSetRequest {
    pub prompt: String,
    pub count: Option<usize>,
    pub source_ids: Option<Vec<i64>>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
}

/// POST /api/v1/evaluations/generate-set — AI-generate evaluation set
///
/// Fetches real document titles from data_sources, then prompts an LLM
/// to generate evaluation questions grounded in those titles for valid
/// Hit Rate and MRR benchmarking.
async fn generate_eval_set(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<GenerateEvalSetRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let count = payload.count.unwrap_or(5).min(20);

    // 1. Fetch available source titles (filtered if source_ids specified)
    let sources: Vec<(i64, String)> = if let Some(ref ids) = payload.source_ids {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, name FROM data_sources WHERE tenant_id = ? AND id IN ({})",
            placeholders
        );
        let mut q = sqlx::query_as::<_, (i64, String)>(&query).bind(tenant_id);
        for id in ids {
            q = q.bind(id);
        }
        q.fetch_all(&pool).await.unwrap_or_default()
    } else {
        sqlx::query_as("SELECT id, name FROM data_sources WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
    };

    if sources.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No data sources found for this tenant"})),
        ));
    }

    let titles: Vec<String> = sources.iter().map(|(_, name)| name.clone()).collect();
    let titles_list = titles.join(", ");

    // 2. Resolve LLM provider/model for generation
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

    let model_id = payload.model_id.unwrap_or(slot.model.clone());
    let provider_name = payload.provider.unwrap_or(slot.provider.clone());

    // Resolve credentials from tenant config using the same pattern as agent chat
    let api_base = crate::routes::sources::infer_api_base(&provider_name);
    let api_key = llm_config
        .heimdall_api_key
        .clone()
        .unwrap_or_else(|| std::env::var("LLM_API_KEY").unwrap_or_else(|_| "no-key".to_string()));

    // 3. Build the generation prompt
    let system_prompt = format!(
        r#"You are an evaluation set generator for a RAG system.
You MUST generate exactly {count} evaluation questions in JSON format.

Available document titles in the knowledge base:
{titles_list}

The output MUST be a JSON object with a "questions" key containing an array of objects.
Each object must follow this EXACT schema:
{{
  "query": "A natural question that a user might ask",
  "expected_titles": ["Exact document title from the list above"]
}}

Rules:
1. Every "expected_titles" value MUST be an exact match from the document titles listed above.
2. Questions should be diverse and test different retrieval strategies.
3. Output ONLY valid JSON, no markdown formatting blocks, no explanations.
4. The user's additional instructions: {prompt}"#,
        count = count,
        titles_list = titles_list,
        prompt = payload.prompt,
    );

    // 4. Call the LLM
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model_id,
            "messages": [
                {"role": "system", "content": "You output only valid JSON objects containing the questions array."},
                {"role": "user", "content": system_prompt}
            ],
            "max_tokens": 4096,
            "temperature": 0.3,
            "response_format": { "type": "json_object" }
        }))
        .send()
        .await
        .map_err(|e| {
            error!("Generate eval set LLM call failed: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("LLM call failed: {}", e)})),
            )
        })?;

    let resp_json: Value = resp.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Parse failed: {}", e)})),
        )
    })?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[]")
        .to_string();

    // 5. Try to parse the response as JSON
    // Define a helper to extract the array from either direct array or { "questions": [...] }
    let extract_array = |v: Value| -> Value {
        if v.is_array() {
            v
        } else if v.is_object() {
            v.get("questions").cloned().unwrap_or(json!([]))
        } else {
            json!([])
        }
    };

    let eval_set: Value = match serde_json::from_str(&content) {
        Ok(parsed) => extract_array(parsed),
        Err(e) => {
            tracing::warn!("Failed direct JSON parse: {}. Content: {}", e, content);
            // Fallback 1: Extract anything between [ and ]
            let mut extracted_arr = None;
            if let (Some(start), Some(end)) = (content.find('['), content.rfind(']')) {
                if start <= end {
                    let extracted = &content[start..=end];
                    if let Ok(parsed) = serde_json::from_str(extracted) {
                         extracted_arr = Some(parsed);
                    }
                }
            }

            if let Some(arr) = extracted_arr {
                arr
            } else {
                // Fallback 2: Extract anything between { and } in case it's an object containing the array
                let mut extracted_obj = None;
                if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
                    if start <= end {
                        let extracted = &content[start..=end];
                        if let Ok(parsed) = serde_json::from_str::<Value>(extracted) {
                             extracted_obj = Some(extract_array(parsed));
                        }
                    }
                }

                if let Some(arr) = extracted_obj {
                    arr
                } else {
                    // Fallback 3: remove markdown ticks
                    let cleaned = content
                        .trim()
                        .trim_start_matches("```json")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();
                    serde_json::from_str(cleaned)
                        .map(extract_array)
                        .unwrap_or_else(|_| {
                            tracing::error!("All JSON parsing fallbacks failed for content.");
                            json!([])
                        })
                }
            }
        }
    };

    info!(
        event = "eval_set_generated",
        count = count,
        model = %model_id,
        titles = titles.len(),
        "AI evaluation set generated"
    );

    Ok(Json(json!({
        "eval_set": eval_set,
        "model_used": model_id,
        "available_titles": titles,
        "count_requested": count
    })))
}

/// GET /api/v1/evaluations/results — Get evaluation results
async fn get_eval_results(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<ResultsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).min(100);
    let offset = (page - 1) * per_page;

    let mut conditions = vec!["tenant_id = ?".to_string()];
    if params.batch_id.is_some() {
        conditions.push("batch_id = ?".to_string());
    }
    if params.model_id.is_some() {
        conditions.push("model_id = ?".to_string());
    }

    let query = format!(
        "SELECT * FROM evaluation_reports WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
        conditions.join(" AND ")
    );

    let mut q = sqlx::query_as::<_, EvalReportRow>(&query).bind(tenant_id);
    if let Some(ref batch) = params.batch_id {
        q = q.bind(batch);
    }
    if let Some(ref model) = params.model_id {
        q = q.bind(model);
    }

    let results = q
        .bind(per_page)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    // Model performance summary
    let summary: Vec<(String, f64, f64, f64, i64)> = sqlx::query_as(
        r#"SELECT
            model_id,
            AVG(accuracy) as avg_accuracy,
            AVG(completeness) as avg_completeness,
            AVG(relevance) as avg_relevance,
            COUNT(*) as total_evals
        FROM evaluation_reports WHERE tenant_id = ?
        GROUP BY model_id
        ORDER BY (AVG(accuracy) + AVG(completeness) + AVG(relevance)) / 3 DESC"#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "results": results,
        "summary": summary.iter().map(|(model, acc, comp, rel, count)| json!({
            "model_id": model,
            "avg_accuracy": acc,
            "avg_completeness": comp,
            "avg_relevance": rel,
            "total_evals": count,
        })).collect::<Vec<_>>(),
        "page": page,
        "per_page": per_page
    })))
}

/// GET /api/v1/evaluations/compare — A/B model comparison
async fn compare_models(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<CompareQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let mut base_query = "SELECT model_id, AVG(accuracy) as avg_accuracy, AVG(completeness) as avg_completeness, AVG(relevance) as avg_relevance, AVG(latency_ms) as avg_latency, COUNT(*) as total FROM evaluation_reports WHERE tenant_id = ? AND model_id IN (?, ?)".to_string();

    if let Some(ref _batch) = params.batch_id {
        base_query.push_str(" AND batch_id = ?");
    }
    base_query.push_str(" GROUP BY model_id");

    let mut q = sqlx::query_as::<_, (String, f64, f64, f64, f64, i64)>(&base_query)
        .bind(tenant_id)
        .bind(&params.model_a)
        .bind(&params.model_b);

    if let Some(ref batch) = params.batch_id {
        q = q.bind(batch);
    }

    let results = q.fetch_all(&pool).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let models: Vec<Value> = results
        .iter()
        .map(|(model, acc, comp, rel, lat, total)| {
            json!({
                "model_id": model,
                "avg_accuracy": acc,
                "avg_completeness": comp,
                "avg_relevance": rel,
                "avg_latency_ms": lat,
                "total_evaluations": total,
                "overall_score": (acc + comp + rel) / 3.0
            })
        })
        .collect();

    Ok(Json(json!({
        "comparison": models,
        "model_a": params.model_a,
        "model_b": params.model_b,
    })))
}

/// GET /api/v1/evaluations/feedback-summary — Aggregate user feedback
async fn feedback_summary(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Per-agent feedback
    let by_agent: Vec<(Option<i64>, String, i64)> = sqlx::query_as(
        r#"SELECT agent_config_id, feedback, COUNT(*) as count
        FROM agent_conversations
        WHERE tenant_id = ? AND feedback IS NOT NULL
        GROUP BY agent_config_id, feedback"#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Per-model feedback
    let by_model: Vec<(Option<String>, String, i64)> = sqlx::query_as(
        r#"SELECT model_id, feedback, COUNT(*) as count
        FROM agent_conversations
        WHERE tenant_id = ? AND feedback IS NOT NULL
        GROUP BY model_id, feedback"#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "by_agent": by_agent.iter().map(|(agent_id, fb, count)| json!({
            "agent_config_id": agent_id,
            "feedback": fb,
            "count": count
        })).collect::<Vec<_>>(),
        "by_model": by_model.iter().map(|(model_id, fb, count)| json!({
            "model_id": model_id,
            "feedback": fb,
            "count": count
        })).collect::<Vec<_>>()
    })))
}

// ─── Sprint 27: Extraction Evaluation ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExtractionQuery {
    pub source_id: Option<i64>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub run_label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtractionCompareQuery {
    pub provider_a: String,
    pub model_a: String,
    pub provider_b: String,
    pub model_b: String,
    pub source_id: Option<i64>,
}

/// GET /api/v1/evaluations/extraction-summary
/// Returns Provider × Model matrix with KG count, QA count, avg latency
async fn extraction_summary(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(_params): Query<ExtractionQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // QA summary by provider × model
    let qa_stats: Vec<(String, String, String, i64, f64)> = sqlx::query_as(
        r#"SELECT COALESCE(provider, 'unknown'), COALESCE(model, 'unknown'), COALESCE(prompt_version, 'v1.0'),
           COUNT(*) as qa_count, AVG(COALESCE(latency_ms, 0)) as avg_latency
           FROM qa_results WHERE tenant_id = ?
           GROUP BY provider, model, prompt_version
           ORDER BY qa_count DESC"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // KG summary by provider × model
    let kg_stats: Vec<(String, String, String, i64, f64)> = sqlx::query_as(
        r#"SELECT COALESCE(provider, 'unknown'), COALESCE(model, 'unknown'), COALESCE(prompt_version, 'v1.0'),
           COUNT(*) as entity_count, AVG(COALESCE(latency_ms, 0)) as avg_latency
           FROM kg_entities WHERE tenant_id = ?
           GROUP BY provider, model, prompt_version
           ORDER BY entity_count DESC"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Relation counts by provider × model
    let rel_stats: Vec<(String, String, i64)> = sqlx::query_as(
        r#"SELECT COALESCE(provider, 'unknown'), COALESCE(model, 'unknown'), COUNT(*) as rel_count
           FROM kg_relations WHERE tenant_id = ?
           GROUP BY provider, model"#,
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Unique providers and models
    let providers: Vec<String> = qa_stats
        .iter()
        .map(|r| r.0.clone())
        .chain(kg_stats.iter().map(|r| r.0.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let models: Vec<String> = qa_stats
        .iter()
        .map(|r| r.1.clone())
        .chain(kg_stats.iter().map(|r| r.1.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    Ok(Json(json!({
        "qa_stats": qa_stats.iter().map(|(p, m, pv, count, lat)| json!({
            "provider": p, "model": m, "prompt_version": pv,
            "qa_count": count, "avg_latency_ms": lat
        })).collect::<Vec<_>>(),
        "kg_stats": kg_stats.iter().map(|(p, m, pv, count, lat)| json!({
            "provider": p, "model": m, "prompt_version": pv,
            "entity_count": count, "avg_latency_ms": lat
        })).collect::<Vec<_>>(),
        "relation_stats": rel_stats.iter().map(|(p, m, count)| json!({
            "provider": p, "model": m, "relation_count": count
        })).collect::<Vec<_>>(),
        "providers": providers,
        "models": models
    })))
}

/// GET /api/v1/evaluations/extraction-compare
/// Side-by-side comparison of two provider+model combinations
async fn extraction_compare(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Query(params): Query<ExtractionCompareQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    let fetch_stats = |provider: &str, model: &str| {
        let p = provider.to_string();
        let m = model.to_string();
        let t = tenant_id.to_string();
        let pool = pool.clone();
        async move {
            let qa: (i64, f64) = sqlx::query_as(
                "SELECT COUNT(*), AVG(COALESCE(latency_ms, 0)) FROM qa_results WHERE tenant_id = ? AND provider = ? AND model = ?"
            ).bind(&t).bind(&p).bind(&m).fetch_one(&pool).await.unwrap_or((0, 0.0));

            let kg: (i64, f64) = sqlx::query_as(
                "SELECT COUNT(*), AVG(COALESCE(latency_ms, 0)) FROM kg_entities WHERE tenant_id = ? AND provider = ? AND model = ?"
            ).bind(&t).bind(&p).bind(&m).fetch_one(&pool).await.unwrap_or((0, 0.0));

            let rels: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM kg_relations WHERE tenant_id = ? AND provider = ? AND model = ?"
            ).bind(&t).bind(&p).bind(&m).fetch_one(&pool).await.unwrap_or((0,));

            json!({
                "provider": p, "model": m,
                "qa_count": qa.0, "qa_avg_latency_ms": qa.1,
                "entity_count": kg.0, "kg_avg_latency_ms": kg.1,
                "relation_count": rels.0
            })
        }
    };

    let (a, b) = tokio::join!(
        fetch_stats(&params.provider_a, &params.model_a),
        fetch_stats(&params.provider_b, &params.model_b)
    );

    Ok(Json(json!({
        "model_a": a,
        "model_b": b
    })))
}

/// GET /api/v1/evaluations/retrieval-summary
/// RAG retrieval quality metrics
async fn retrieval_summary(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Source coverage
    let source_stats: Vec<(i64, String, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT d.id, d.name,
           (SELECT COUNT(*) FROM chunks WHERE source_id = d.id) as chunk_count,
           (SELECT COUNT(*) FROM qa_results q JOIN pipeline_steps ps ON q.step_id = ps.id WHERE ps.file_name LIKE CONCAT('chunk_%') AND q.tenant_id = d.tenant_id) as qa_count,
           (SELECT COUNT(*) FROM kg_entities WHERE source_id = d.id) as entity_count
           FROM data_sources d WHERE d.tenant_id = ?
           ORDER BY d.name"#
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Collection stats from Qdrant (HTTP check)
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let client = reqwest::Client::new();

    let mut collections = vec![];
    for col_name in &["source_chunks", "golden_qa"] {
        let resp = client
            .get(format!("{}/collections/{}", qdrant_url, col_name))
            .send()
            .await;
        let count = match resp {
            Ok(r) if r.status().is_success() => {
                let body: Value = r.json().await.unwrap_or_default();
                body["result"]["points_count"].as_u64().unwrap_or(0)
            }
            _ => 0,
        };
        collections.push(json!({ "name": col_name, "points_count": count }));
    }

    Ok(Json(json!({
        "sources": source_stats.iter().map(|(id, name, chunks, qa, ents)| json!({
            "source_id": id, "name": name,
            "chunk_count": chunks, "qa_count": qa, "entity_count": ents
        })).collect::<Vec<_>>(),
        "qdrant_collections": collections
    })))
}

// ─── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_question_deserialization() {
        let json = r#"{"question":"test?","expected_answer":"yes"}"#;
        let q: EvalQuestion = serde_json::from_str(json).unwrap();
        assert_eq!(q.question, "test?");
        assert_eq!(q.expected_answer, Some("yes".to_string()));
    }

    #[test]
    fn test_compare_query() {
        let q = CompareQuery {
            model_a: "llama3.2".into(),
            model_b: "gemini-2.0-flash".into(),
            batch_id: None,
        };
        assert_eq!(q.model_a, "llama3.2");
    }
}

// ─── Sprint 28: E2E Pipeline Scorecard ──────────────────────────────────────────

/// GET /api/v1/evaluations/pipeline-scorecard
/// Returns per-source pipeline completion scorecard with step-level status
async fn pipeline_scorecard(
    headers: HeaderMap,
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // Get all sources with their pipeline coverage
    let sources: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, source_type FROM data_sources WHERE tenant_id = ? ORDER BY name",
    )
    .bind(tenant_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut scorecards = Vec::new();

    for (source_id, name, source_type) in &sources {
        // Count chunks
        let chunk_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM chunks WHERE source_id = ? AND tenant_id = ?")
                .bind(source_id)
                .bind(tenant_id)
                .fetch_one(&pool)
                .await
                .unwrap_or((0,));

        // Count KG entities
        let entity_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM kg_entities WHERE source_id = ? AND tenant_id = ?",
        )
        .bind(source_id)
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        // Count KG relations
        let relation_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM kg_relations WHERE source_id = ? AND tenant_id = ?",
        )
        .bind(source_id)
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

        // Count QA results
        let qa_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM qa_results WHERE tenant_id = ? AND chunk_id IN (SELECT id FROM chunks WHERE source_id = ?)"
        ).bind(tenant_id).bind(source_id).fetch_one(&pool).await.unwrap_or((0,));

        // Check embedding status
        let embed_step: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pipeline_steps WHERE source_id = ? AND step_name = 'embedding' AND status = 'completed'"
        ).bind(source_id).fetch_one(&pool).await.unwrap_or((0,));

        // Check QA generation status
        let qa_step: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM pipeline_steps WHERE source_id = ? AND step_name = 'qa_generation'"
        ).bind(source_id).fetch_one(&pool).await.unwrap_or((0,));

        // Latest pipeline run status
        let latest_run: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, status, provider, model FROM pipeline_runs WHERE source_id = ? AND tenant_id = ? ORDER BY started_at DESC LIMIT 1"
        ).bind(source_id).bind(tenant_id).fetch_optional(&pool).await.unwrap_or(None);

        let has_chunks = chunk_count.0 > 0;
        let has_embed = embed_step.0 > 0;
        let has_kg = entity_count.0 > 0;
        let has_qa = qa_count.0 > 0;
        let has_qa_index = qa_step.0 > 0;

        let steps_done = [has_chunks, has_embed, has_kg, has_qa, has_qa_index]
            .iter()
            .filter(|&&x| x)
            .count();

        scorecards.push(json!({
            "source_id": source_id,
            "name": name,
            "source_type": source_type,
            "steps": {
                "chunks": { "done": has_chunks, "count": chunk_count.0 },
                "embedded": { "done": has_embed, "count": chunk_count.0 },
                "kg_entities": { "done": has_kg, "count": entity_count.0 },
                "kg_relations": { "count": relation_count.0 },
                "qa_pairs": { "done": has_qa, "count": qa_count.0 },
                "qa_indexed": { "done": has_qa_index },
            },
            "completion": format!("{}/5", steps_done),
            "completion_pct": (steps_done as f64 / 5.0 * 100.0) as i32,
            "latest_run": latest_run.as_ref().map(|(id, status, provider, model)| json!({
                "run_id": id, "status": status,
                "provider": provider, "model": model,
            })),
        }));
    }

    // Summary totals
    let total_sources = sources.len();
    let fully_complete = scorecards
        .iter()
        .filter(|s| s["completion_pct"] == 100)
        .count();

    Ok(Json(json!({
        "total_sources": total_sources,
        "fully_complete": fully_complete,
        "completion_rate": if total_sources > 0 { (fully_complete as f64 / total_sources as f64 * 100.0) as i32 } else { 0 },
        "sources": scorecards,
    })))
}
