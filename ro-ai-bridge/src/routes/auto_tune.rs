//! Auto-tune agent parameters using a frontier model (configured via `app_settings.auto_tune_model`).
//!
//! Flow:
//!   1. Caller provides agent_id + run_id (or defaults to most recent run for that agent).
//!   2. Backend gathers: full agent config + eval scores (with judge_reasoning) + summary.
//!   3. Sends a structured prompt to the auto-tune model (e.g., `gemini-3.1-pro-preview`).
//!   4. Returns suggested changes (system_prompt, temperature, top_k, tools, etc.) + rationale.
//!
//! The caller can then apply changes via PUT /api/v1/agents/{id}.

use axum::{extract::{Path, State}, http::HeaderMap, Json, Router, routing::post};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::gemini_helper::{
    self, DEFAULT_AUTO_TUNE_MODEL, GeminiCallConfig,
};
use serde::{Deserialize, Serialize};

use crate::routes::app_settings::get_setting_value;
use crate::routes::tenant::extract_tenant_id;

#[derive(Debug, Deserialize)]
pub struct AutoTuneRequest {
    /// Optional: specific run to analyze. Defaults to most recent for the agent.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Optional override for the auto-tune model. Falls back to app_settings value.
    #[serde(default)]
    pub auto_tune_model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AutoTuneResponse {
    pub run_id: String,
    pub auto_tune_model: String,
    pub current_config: serde_json::Value,
    pub current_metrics: serde_json::Value,
    pub suggestions: serde_json::Value,
    pub rationale: String,
    pub raw_response: String,
}

pub fn auto_tune_routes() -> Router<DbPool> {
    Router::new().route("/agents/{id}/auto-tune", post(auto_tune_agent))
}

async fn auto_tune_agent(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(agent_id): Path<i64>,
    Json(req): Json<AutoTuneRequest>,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    // 1. Load agent config
    let agent_row: Option<(
        String, String, String, String, Option<f64>, Option<i32>, Option<i32>,
        Option<bool>, Option<bool>, Option<String>,
    )> = sqlx::query_as(
        "SELECT name, system_prompt, model_id, provider,
                CAST(temperature AS DOUBLE) AS temperature,
                max_tokens, top_k,
                use_rag, use_knowledge_graph, tools
         FROM agent_configs
         WHERE id = ? AND tenant_id = ?",
    )
    .bind(agent_id)
    .bind(&tenant_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let Some((name, system_prompt, model_id, provider, temperature, max_tokens, top_k,
              use_rag, use_kg, tools_str)) = agent_row else {
        return Json(serde_json::json!({"error": "agent not found"}));
    };

    // 2. Resolve run_id: provided or most recent for this agent
    let run_id: Option<String> = if let Some(rid) = req.run_id {
        Some(rid)
    } else {
        let r: Option<(String,)> = sqlx::query_as(
            "SELECT s.run_id FROM eval_summary s
             JOIN eval_runs r ON s.run_id = r.id
             WHERE s.agent_name = ? AND r.tenant_id = ? AND r.status = 'COMPLETED'
             ORDER BY r.started_at DESC LIMIT 1",
        )
        .bind(&name)
        .bind(&tenant_id)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();
        r.map(|(s,)| s)
    };

    let Some(run_id) = run_id else {
        return Json(serde_json::json!({"error": "no completed eval run found for this agent"}));
    };

    // 3. Load summary + scores
    let summary: Option<(f32, f32, f32, Option<f32>, i32, f32)> = sqlx::query_as(
        "SELECT avg_accuracy, avg_completeness, avg_relevance,
                avg_safety_score, unsafe_count, avg_latency_ms
         FROM eval_summary WHERE run_id = ? AND agent_name = ? LIMIT 1",
    )
    .bind(&run_id)
    .bind(&name)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let scores: Vec<(String, String, String, Option<i8>, Option<i8>, Option<i8>, Option<i32>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT question, expected_answer, actual_answer,
                accuracy_score, completeness_score, relevance_score,
                safety_score, judge_reasoning, tags
         FROM eval_scores
         WHERE run_id = ? AND agent_name = ?
         ORDER BY id LIMIT 20",
    )
    .bind(&run_id)
    .bind(&name)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if scores.is_empty() {
        return Json(serde_json::json!({"error": "no scores found for run"}));
    }

    // 4. Build prompt
    let metrics_json = match summary {
        Some((a, c, r, s, uc, lat)) => serde_json::json!({
            "avg_accuracy": a, "avg_completeness": c, "avg_relevance": r,
            "avg_safety_score": s, "unsafe_count": uc, "avg_latency_ms": lat,
            "total_questions": scores.len(),
        }),
        None => serde_json::json!({"total_questions": scores.len()}),
    };

    let tools_parsed: serde_json::Value = tools_str
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    let current_config = serde_json::json!({
        "agent_id": agent_id, "name": name, "model_id": model_id, "provider": provider,
        "temperature": temperature, "max_tokens": max_tokens, "top_k": top_k,
        "use_rag": use_rag, "use_knowledge_graph": use_kg, "tools": tools_parsed,
    });

    let mut score_lines = String::new();
    for (i, (q, exp, act, acc, comp, rel, safety, reasoning, tags)) in scores.iter().enumerate() {
        // Only show low-scoring or unsafe items in detail (token budget)
        let acc_v = acc.unwrap_or(0);
        let comp_v = comp.unwrap_or(0);
        let rel_v = rel.unwrap_or(0);
        let safety_v = safety.unwrap_or(0);
        let is_problem = acc_v < 4 || comp_v < 4 || rel_v < 4 || safety_v < 0;
        if !is_problem && i > 5 { continue; }
        score_lines.push_str(&format!(
            "\n--- Item {} ({}) ---\nQ: {}\nReference: {}\nActual: {}\nScores: acc={} comp={} rel={} safety={}\nJudge: {}\n",
            i + 1,
            tags.as_deref().unwrap_or("{}"),
            &q[..q.len().min(400)],
            &exp[..exp.len().min(400)],
            &act[..act.len().min(800)],
            acc_v, comp_v, rel_v, safety_v,
            &reasoning.as_deref().unwrap_or("")[..reasoning.as_deref().unwrap_or("").len().min(300)]
        ));
    }

    let user_prompt = format!(
        "You are an expert AI prompt engineer optimizing a medical AI agent.\n\
        Analyze the agent's evaluation results and suggest specific, actionable improvements \
        to its system prompt and parameters.\n\n\
        ## Current Agent Config\n{}\n\n\
        ## Current System Prompt\n```\n{}\n```\n\n\
        ## Evaluation Metrics\n{}\n\n\
        ## Per-Question Results (low-scoring items shown in detail)\n{}\n\n\
        ## Required Output\n\
        Return ONLY valid JSON, no markdown fences:\n\
        {{\n  \"system_prompt\": \"<improved prompt or null if no change>\",\n  \
        \"temperature\": <new value 0.0-1.0 or null>,\n  \
        \"max_tokens\": <new value or null>,\n  \
        \"top_k\": <new value or null>,\n  \
        \"add_tools\": [<tool names>],\n  \
        \"remove_tools\": [<tool names>],\n  \
        \"use_rag\": <bool or null>,\n  \
        \"use_knowledge_graph\": <bool or null>,\n  \
        \"rationale\": \"<2-4 sentences explaining each change and how it addresses observed weaknesses>\",\n  \
        \"expected_improvements\": [\"<specific predicted improvement>\", ...]\n}}\n\n\
        Focus on:\n\
        - Completeness if avg_completeness < 3 → request more thorough answers in prompt\n\
        - Accuracy if avg_accuracy < 3 → tighten grounding to KB, require citations\n\
        - Safety if unsafe_count > 0 → strengthen safety instructions\n\
        - Latency if avg_latency_ms > 12000 → reduce max_tokens or top_k\n\
        - Tool relevance: if KB tools never get hits, remove them\n\
        Be concrete. Keep system_prompt similar in structure but improved.",
        serde_json::to_string_pretty(&current_config).unwrap_or_default(),
        system_prompt,
        serde_json::to_string_pretty(&metrics_json).unwrap_or_default(),
        score_lines,
    );

    // 5. Resolve auto-tune model: explicit override → app_settings → constant default
    let model = req.auto_tune_model.unwrap_or_default();
    let model = if !model.is_empty() {
        model
    } else {
        let from_settings = get_setting_value(&pool, "auto_tune_model", "AUTO_TUNE_MODEL").await;
        if from_settings.is_empty() { DEFAULT_AUTO_TUNE_MODEL.to_string() } else { from_settings }
    };

    // 6. Call Gemini via shared helper
    let cfg = GeminiCallConfig {
        temperature: 0.3,
        max_output_tokens: 4096,
        force_json: true,
        timeout_secs: 120,
    };
    let result = match gemini_helper::call_text(&model, &user_prompt, &cfg).await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({"error": e.to_string()})),
    };
    let raw = result.text;

    // 7. Extract JSON suggestion
    let suggestions_json = gemini_helper::extract_json_object(&raw)
        .unwrap_or(serde_json::Value::Null);

    let rationale = suggestions_json.get("rationale").and_then(|v| v.as_str()).unwrap_or("").to_string();

    Json(serde_json::json!({
        "run_id": run_id,
        "auto_tune_model": model,
        "current_config": current_config,
        "current_metrics": metrics_json,
        "suggestions": suggestions_json,
        "rationale": rationale,
        "raw_response": raw,
    }))
}
