//! Extended Evaluation API — Model performance evaluation and comparison
//!
//! Endpoints:
//! - POST   /api/v1/evaluations/run              — run evaluation batch
//! - GET    /api/v1/evaluations/results           — get evaluation results
//! - GET    /api/v1/evaluations/compare           — A/B model comparison
//! - GET    /api/v1/evaluations/feedback-summary  — user feedback aggregation

use axum::{
    routing::{get, post},
    Router, Json,
    extract::{State, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tracing::{info, error, warn};
use uuid::Uuid;

use mimir_core_ai::services::db::DbPool;
use crate::agents::eval::{evaluate_agent, judge_response, JudgeScores};

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
        .route("/results", get(get_eval_results))
        .route("/compare", get(compare_models))
        .route("/feedback-summary", get(feedback_summary))
}

// ─── Handlers ───────────────────────────────────────────────────────────────────

/// POST /api/v1/evaluations/run — Run evaluation batch
async fn run_evaluation_batch(
    State(pool): State<DbPool>,
    Json(payload): Json<RunEvalRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
    let batch_id = Uuid::new_v4().to_string();
    let judge_model = payload.judge_model.unwrap_or_else(|| "gemini-2.0-flash".into());
    let agent_name = payload.agent_name.unwrap_or_else(|| "oracle_rag".into());

    info!("Starting evaluation batch {} with {} models x {} questions",
        batch_id, payload.models.len(), payload.questions.len());

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
            ).await;

            let (actual_answer, latency_ms, error_msg) = match eval_result {
                Ok(r) => (Some(r.answer), r.latency_ms as i32, None),
                Err(e) => {
                    warn!("Eval failed for model {} question '{}': {}", model_id, q.question, e);
                    (None, 0, Some(e.to_string()))
                }
            };

            // Judge response if we have an answer and expected answer
            let (accuracy, completeness, relevance, reasoning) = if let (Some(ref answer), Some(ref expected)) = (&actual_answer, &q.expected_answer) {
                match judge_response(&q.question, expected, answer, &judge_model).await {
                    Ok(scores) => {
                        total_scored += 1;
                        (scores.accuracy as i32, scores.completeness as i32, scores.relevance as i32, Some(scores.reasoning))
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
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
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

    info!("Evaluation batch {} completed: {} results, {} scored", batch_id, results.len(), total_scored);

    Ok(Json(json!({
        "batch_id": batch_id,
        "total_evaluations": results.len(),
        "total_scored": total_scored,
        "results": results
    })))
}

/// GET /api/v1/evaluations/results — Get evaluation results
async fn get_eval_results(
    State(pool): State<DbPool>,
    Query(params): Query<ResultsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";
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

    let results = q.bind(per_page).bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

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
        ORDER BY (AVG(accuracy) + AVG(completeness) + AVG(relevance)) / 3 DESC"#
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
    State(pool): State<DbPool>,
    Query(params): Query<CompareQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";

    let mut base_query = "SELECT model_id, AVG(accuracy) as avg_accuracy, AVG(completeness) as avg_completeness, AVG(relevance) as avg_relevance, AVG(latency_ms) as avg_latency, COUNT(*) as total FROM evaluation_reports WHERE tenant_id = ? AND model_id IN (?, ?)".to_string();

    if let Some(ref batch) = params.batch_id {
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

    let results = q.fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let models: Vec<Value> = results.iter().map(|(model, acc, comp, rel, lat, total)| {
        json!({
            "model_id": model,
            "avg_accuracy": acc,
            "avg_completeness": comp,
            "avg_relevance": rel,
            "avg_latency_ms": lat,
            "total_evaluations": total,
            "overall_score": (acc + comp + rel) / 3.0
        })
    }).collect();

    Ok(Json(json!({
        "comparison": models,
        "model_a": params.model_a,
        "model_b": params.model_b,
    })))
}

/// GET /api/v1/evaluations/feedback-summary — Aggregate user feedback
async fn feedback_summary(
    State(pool): State<DbPool>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let tenant_id = "default_tenant";

    // Per-agent feedback
    let by_agent: Vec<(Option<i64>, String, i64)> = sqlx::query_as(
        r#"SELECT agent_config_id, feedback, COUNT(*) as count
        FROM agent_conversations
        WHERE tenant_id = ? AND feedback IS NOT NULL
        GROUP BY agent_config_id, feedback"#
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
        GROUP BY model_id, feedback"#
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
