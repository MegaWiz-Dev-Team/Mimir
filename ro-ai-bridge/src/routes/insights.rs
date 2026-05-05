//! Wave 2 — AI Analysis endpoints
//!
//! - GET  /api/v1/eval/runs/:id/insights              → cached run-level summary + failures + recs
//! - POST /api/v1/eval/runs/:id/insights/regenerate   → force re-summarize
//! - POST /api/v1/eval/scores/:id/diagnose            → per-item failure diagnosis
//! - POST /api/v1/eval/scores/:id/explain-retrieval   → RAG drill-down explanation
//!
//! Each endpoint:
//!   1. Reads structured eval data (scores, traces, summaries)
//!   2. Calls a Gemini model (configured via app_settings.insight_model)
//!   3. Caches the result in `experiment_insights`
//!   4. Returns structured JSON

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::gemini_helper::{
    self, DEFAULT_INSIGHT_MODEL, GeminiCallConfig,
};
use serde::Serialize;

use crate::routes::app_settings::get_setting_value;
use crate::routes::tenant::extract_tenant_id;

#[derive(Debug, Serialize)]
pub struct InsightResponse {
    pub run_id: String,
    pub insight_type: String,
    pub model_used: String,
    pub cost_usd: f64,
    pub content: String,
    pub structured: serde_json::Value,
    pub cached: bool,
    pub created_at: String,
}

pub fn insights_routes() -> Router<DbPool> {
    Router::new()
        .route("/eval/runs/{id}/insights", get(get_run_insights))
        .route("/eval/runs/{id}/insights/regenerate", post(regenerate_run_insights))
        .route("/eval/scores/{id}/diagnose", post(diagnose_score))
        .route("/eval/scores/{id}/explain-retrieval", post(explain_retrieval))
}

// ═══ Run-level insights (summary + patterns + recs) ════════════════════════════

async fn get_run_insights(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    // Try cached first
    if let Some(cached) = fetch_cached(&pool, &run_id, "summary", None).await {
        return Json(cached);
    }
    match generate_run_summary(&pool, &tenant_id, &run_id).await {
        Ok(v) => Json(v),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn regenerate_run_insights(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers).to_string();
    // Drop cached
    let _ = sqlx::query("DELETE FROM experiment_insights WHERE run_id = ? AND insight_type = 'summary'")
        .bind(&run_id)
        .execute(&pool)
        .await;
    match generate_run_summary(&pool, &tenant_id, &run_id).await {
        Ok(v) => Json(v),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn generate_run_summary(
    pool: &DbPool,
    tenant_id: &str,
    run_id: &str,
) -> Result<serde_json::Value, String> {
    // 1. Fetch run + summary + scores
    let run: Option<(Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>)> = sqlx::query_as(
        "SELECT name, hypothesis, variable_under_test, expected_change, CAST(total_cost_usd AS DOUBLE)
         FROM eval_runs WHERE id = ? AND tenant_id = ?",
    )
    .bind(run_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some((name, hypothesis, var, expected, cost)) = run else {
        return Err(format!("run {} not found in tenant {}", run_id, tenant_id));
    };

    let summaries: Vec<(String, String, i32, Option<f32>, Option<f32>, Option<f32>, Option<f32>, i32, Option<f32>, Option<f32>)> = sqlx::query_as(
        "SELECT agent_name, model_id, total_questions,
                avg_accuracy, avg_completeness, avg_relevance,
                avg_safety_score, unsafe_count, avg_latency_ms, overall_score
         FROM eval_summary WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Fetch low-scoring items in detail
    let scores: Vec<(i64, String, Option<i8>, Option<i8>, Option<i8>, Option<i32>, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, question, accuracy_score, completeness_score, relevance_score, safety_score,
                judge_reasoning, retrieval_trace, tags, actual_answer
         FROM eval_scores
         WHERE run_id = ? AND tenant_id = ?
         ORDER BY (COALESCE(accuracy_score,0)+COALESCE(completeness_score,0)+COALESCE(relevance_score,0)) ASC
         LIMIT 12",
    )
    .bind(run_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if summaries.is_empty() && scores.is_empty() {
        return Err("no data to analyze".to_string());
    }

    // 2. Build prompt
    let mut summary_text = String::new();
    for (agent, model, n, acc, comp, rel, safety, unsafe_n, lat, overall) in &summaries {
        summary_text.push_str(&format!(
            "\n• {}/{}  n={}  acc={:.2} comp={:.2} rel={:.2} safety={:.2} unsafe={} lat={:.0}ms overall={:.2}",
            agent, model, n,
            acc.unwrap_or(0.0), comp.unwrap_or(0.0), rel.unwrap_or(0.0),
            safety.unwrap_or(0.0), unsafe_n, lat.unwrap_or(0.0), overall.unwrap_or(0.0),
        ));
    }

    let mut detail_text = String::new();
    for (i, (_, q, a, c, r, s, judge, trace, tags, _ans)) in scores.iter().enumerate() {
        detail_text.push_str(&format!(
            "\n--- Item {} ---\n  tags: {}\n  Q: {}\n  scores: acc={} comp={} rel={} safety={}\n  judge: {}\n  retrieval: {}\n",
            i + 1,
            tags.as_deref().unwrap_or("{}"),
            &q[..q.len().min(250)],
            a.unwrap_or(0), c.unwrap_or(0), r.unwrap_or(0), s.unwrap_or(0),
            &judge.as_deref().unwrap_or("")[..judge.as_deref().unwrap_or("").len().min(180)],
            trace.as_deref().unwrap_or("(none)"),
        ));
    }

    let prompt = format!(
        "You are an AI evaluation analyst reviewing a medical agent's benchmark run.\n\n\
        ## Run\n  Name: {}\n  Hypothesis: {}\n  Variable: {}\n  Expected: {}\n  Cost: ${:.4}\n\n\
        ## Aggregate Summary{}\n\n\
        ## Worst-scoring items (top 12){}\n\n\
        Produce a JSON report (no markdown, no preamble):\n\
        {{\n\
          \"executive_summary\": \"<3-4 sentences: what happened, what scored well, what didn't>\",\n\
          \"failure_patterns\": [\n\
            {{\"pattern\": \"<short label>\", \"count\": <int>, \"example_item_indexes\": [<1-based int>], \"explanation\": \"<1 sentence>\"}}\n\
          ],\n\
          \"retrieval_health\": {{\"good\": <int>, \"empty\": <int>, \"observation\": \"<sentence about RAG retrieval quality>\"}},\n\
          \"recommendations\": [\n\
            {{\"action\": \"<concrete action>\", \"target\": \"system_prompt|temperature|tools|rag_weights|other\", \"why\": \"<1 sentence>\", \"priority\": \"high|medium|low\"}}\n\
          ],\n\
          \"next_hypothesis\": \"<1 specific testable hypothesis to try next>\"\n\
        }}",
        name.unwrap_or_default(),
        hypothesis.unwrap_or_default(),
        var.unwrap_or_default(),
        expected.unwrap_or_default(),
        cost.unwrap_or(0.0),
        summary_text,
        detail_text,
    );

    // 3. Call Gemini
    let model = get_setting_value(pool, "insight_model", "INSIGHT_MODEL").await;
    let model = if model.is_empty() { DEFAULT_INSIGHT_MODEL.to_string() } else { model };
    let (raw, in_tok, out_tok) = call_gemini(&model, &prompt, 4096).await?;
    let cost_usd = estimate_cost(pool, &model, in_tok, out_tok).await;

    // 4. Parse structured JSON
    let structured = extract_json(&raw).unwrap_or(serde_json::json!({"executive_summary": raw.clone()}));
    let exec_summary = structured.get("executive_summary").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // 5. Cache
    let _ = sqlx::query(
        "INSERT INTO experiment_insights (run_id, insight_type, content, structured, model_used, cost_usd)
         VALUES (?, 'summary', ?, ?, ?, ?)",
    )
    .bind(run_id)
    .bind(if exec_summary.is_empty() { &raw } else { &exec_summary })
    .bind(structured.to_string())
    .bind(&model)
    .bind(cost_usd)
    .execute(pool)
    .await;

    Ok(serde_json::json!({
        "run_id": run_id,
        "insight_type": "summary",
        "model_used": model,
        "cost_usd": cost_usd,
        "content": if exec_summary.is_empty() { raw } else { exec_summary },
        "structured": structured,
        "cached": false,
    }))
}

// ═══ Per-item diagnosis ════════════════════════════════════════════════════════

async fn diagnose_score(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(score_id): Path<i64>,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    if let Some(cached) = fetch_cached(&pool, "", "item_diagnosis", Some(&score_id.to_string())).await {
        return Json(cached);
    }

    let row: Option<(String, String, String, Option<String>, Option<i8>, Option<i8>, Option<i8>, Option<i32>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT run_id, question, expected_answer, actual_answer,
                accuracy_score, completeness_score, relevance_score, safety_score,
                judge_reasoning, retrieval_trace, tags
         FROM eval_scores WHERE id = ? AND tenant_id = ?",
    )
    .bind(score_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();
    let Some((run_id, q, exp, actual, acc, comp, rel, safety, judge, trace, tags)) = row else {
        return Json(serde_json::json!({"error": "score not found"}));
    };

    let prompt = format!(
        "You are diagnosing why a medical AI agent gave a low-quality answer.\n\n\
        Question: {}\n\nReference answer: {}\n\nAgent's actual answer: {}\n\n\
        Scores: accuracy={} completeness={} relevance={} safety={}\n\
        Judge reasoning: {}\n\
        Retrieval trace: {}\n\
        Item tags: {}\n\n\
        Output JSON (no markdown):\n\
        {{\n\
          \"root_cause\": \"retrieval_miss|wrong_model|prompt_gap|hallucination|language_mismatch|safety_breach|other\",\n\
          \"explanation\": \"<2-3 sentences pinpointing what went wrong>\",\n\
          \"fix\": {{\"target\": \"system_prompt|tools|rag_weights|temperature|model|other\", \"action\": \"<specific change>\"}},\n\
          \"confidence\": \"high|medium|low\"\n\
        }}",
        &q[..q.len().min(500)],
        &exp[..exp.len().min(500)],
        &actual.as_deref().unwrap_or("")[..actual.as_deref().unwrap_or("").len().min(800)],
        acc.unwrap_or(0), comp.unwrap_or(0), rel.unwrap_or(0), safety.unwrap_or(0),
        &judge.as_deref().unwrap_or("")[..judge.as_deref().unwrap_or("").len().min(300)],
        trace.as_deref().unwrap_or("(none)"),
        tags.as_deref().unwrap_or("{}"),
    );

    let model = get_setting_value(&pool, "insight_model", "INSIGHT_MODEL").await;
    let model = if model.is_empty() { DEFAULT_INSIGHT_MODEL.to_string() } else { model };
    let (raw, in_tok, out_tok) = match call_gemini(&model, &prompt, 4096).await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({"error": e})),
    };
    let cost_usd = estimate_cost(&pool, &model, in_tok, out_tok).await;
    let structured = extract_json(&raw).unwrap_or(serde_json::json!({"explanation": raw.clone()}));

    let _ = sqlx::query(
        "INSERT INTO experiment_insights (run_id, insight_type, target_id, content, structured, model_used, cost_usd)
         VALUES (?, 'item_diagnosis', ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(score_id.to_string())
    .bind(structured.get("explanation").and_then(|v| v.as_str()).unwrap_or(&raw))
    .bind(structured.to_string())
    .bind(&model)
    .bind(cost_usd)
    .execute(&pool)
    .await;

    Json(serde_json::json!({
        "score_id": score_id,
        "run_id": run_id,
        "model_used": model,
        "cost_usd": cost_usd,
        "structured": structured,
        "cached": false,
    }))
}

// ═══ Retrieval explanation (RAG drill-down) ══════════════════════════════════

async fn explain_retrieval(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(score_id): Path<i64>,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    if let Some(cached) = fetch_cached(&pool, "", "retrieval_explanation", Some(&score_id.to_string())).await {
        return Json(cached);
    }

    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT run_id, question, retrieval_trace FROM eval_scores
         WHERE id = ? AND tenant_id = ?",
    )
    .bind(score_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();
    let Some((run_id, q, trace)) = row else {
        return Json(serde_json::json!({"error": "score not found"}));
    };
    let Some(trace_str) = trace else {
        return Json(serde_json::json!({"error": "no retrieval trace captured for this item"}));
    };

    let prompt = format!(
        "You are a retrieval (RAG) analyst. Explain whether the retrieval was sufficient for the question.\n\n\
        Question: {}\n\n\
        Retrieval trace (sources × counts):\n{}\n\n\
        Output JSON only:\n\
        {{\n\
          \"verdict\": \"sufficient|partial|insufficient\",\n\
          \"observation\": \"<1-2 sentences on what was retrieved vs what was needed>\",\n\
          \"missing\": [\"<source/topic that should have been retrieved>\"],\n\
          \"suggested_change\": {{\"target\": \"rag_weights|tools|threshold|other\", \"value\": \"<recommendation>\"}}\n\
        }}",
        &q[..q.len().min(800)], trace_str,
    );

    let model = get_setting_value(&pool, "insight_model", "INSIGHT_MODEL").await;
    let model = if model.is_empty() { DEFAULT_INSIGHT_MODEL.to_string() } else { model };
    let (raw, in_tok, out_tok) = match call_gemini(&model, &prompt, 1500).await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({"error": e})),
    };
    let cost_usd = estimate_cost(&pool, &model, in_tok, out_tok).await;
    let structured = extract_json(&raw).unwrap_or(serde_json::json!({"observation": raw.clone()}));

    let _ = sqlx::query(
        "INSERT INTO experiment_insights (run_id, insight_type, target_id, content, structured, model_used, cost_usd)
         VALUES (?, 'retrieval_explanation', ?, ?, ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(score_id.to_string())
    .bind(structured.get("observation").and_then(|v| v.as_str()).unwrap_or(&raw))
    .bind(structured.to_string())
    .bind(&model)
    .bind(cost_usd)
    .execute(&pool)
    .await;

    Json(serde_json::json!({
        "score_id": score_id,
        "model_used": model,
        "cost_usd": cost_usd,
        "structured": structured,
        "cached": false,
    }))
}

// ═══ Helpers ═══════════════════════════════════════════════════════════════════

async fn fetch_cached(
    pool: &DbPool,
    run_id: &str,
    insight_type: &str,
    target_id: Option<&str>,
) -> Option<serde_json::Value> {
    let row: Option<(String, String, String, f64, chrono::DateTime<chrono::Utc>)> = if let Some(tid) = target_id {
        sqlx::query_as(
            "SELECT content, structured, model_used, CAST(cost_usd AS DOUBLE), created_at
             FROM experiment_insights
             WHERE insight_type = ? AND target_id = ?
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(insight_type)
        .bind(tid)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    } else {
        sqlx::query_as(
            "SELECT content, structured, model_used, CAST(cost_usd AS DOUBLE), created_at
             FROM experiment_insights
             WHERE run_id = ? AND insight_type = ? AND target_id IS NULL
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(run_id)
        .bind(insight_type)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    };

    row.map(|(content, structured, model, cost, created)| {
        let s: serde_json::Value = serde_json::from_str(&structured).unwrap_or(serde_json::Value::Null);
        serde_json::json!({
            "run_id": run_id,
            "insight_type": insight_type,
            "target_id": target_id,
            "model_used": model,
            "cost_usd": cost,
            "content": content,
            "structured": s,
            "cached": true,
            "created_at": created.to_rfc3339(),
        })
    })
}

/// Wrapper: call Gemini with JSON mime type forced. Returns (text, in_tokens, out_tokens).
async fn call_gemini(model: &str, prompt: &str, max_tokens: u32) -> Result<(String, i64, i64), String> {
    let cfg = GeminiCallConfig {
        temperature: 0.2,
        max_output_tokens: max_tokens,
        force_json: true,
        timeout_secs: 60,
    };
    gemini_helper::call_text(model, prompt, &cfg)
        .await
        .map(|r| (r.text, r.input_tokens, r.output_tokens))
        .map_err(|e| e.to_string())
}

#[inline]
fn extract_json(text: &str) -> Option<serde_json::Value> {
    gemini_helper::extract_json_object(text)
}

async fn estimate_cost(pool: &DbPool, model_id: &str, input_tok: i64, output_tok: i64) -> f64 {
    let pricing: Option<(f64, f64)> = sqlx::query_as(
        "SELECT CAST(input_per_1m_usd AS DOUBLE), CAST(output_per_1m_usd AS DOUBLE)
         FROM model_pricing WHERE model_id = ?",
    )
    .bind(model_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    if let Some((i, o)) = pricing {
        (input_tok as f64) / 1_000_000.0 * i + (output_tok as f64) / 1_000_000.0 * o
    } else {
        0.0
    }
}
