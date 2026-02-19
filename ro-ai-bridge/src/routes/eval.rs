//! Evaluation API routes
//!
//! Provides REST endpoints for the evaluation dashboard:
//! - GET  /api/eval/runs           — List all evaluation runs
//! - GET  /api/eval/runs/:id       — Get run detail + summaries
//! - GET  /api/eval/runs/:id/scores — Get individual scores (filterable)
//! - GET  /api/eval/runs/:id/matrix — Get heatmap matrix data
//! - PATCH /api/eval/scores/:id/review — Submit human review

use axum::{
    extract::{Path, Query, State},
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::NaiveDateTime;

use crate::services::db::DbPool;

// ─── Request / Response types ──────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct EvalRun {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub total_combinations: i32,
    pub completed_combinations: i32,
    pub started_at: NaiveDateTime,
    pub finished_at: Option<NaiveDateTime>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EvalScore {
    pub id: i64,
    pub run_id: String,
    pub agent_name: String,
    pub model_id: String,
    pub question: String,
    pub expected_answer: String,
    pub actual_answer: Option<String>,
    pub accuracy_score: Option<i8>,
    pub completeness_score: Option<i8>,
    pub relevance_score: Option<i8>,
    pub latency_ms: Option<i32>,
    pub judge_model: Option<String>,
    pub judge_reasoning: Option<String>,
    pub human_accuracy_score: Option<i8>,
    pub human_completeness_score: Option<i8>,
    pub human_relevance_score: Option<i8>,
    pub human_notes: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EvalSummary {
    pub id: i64,
    pub run_id: String,
    pub agent_name: String,
    pub model_id: String,
    pub total_questions: i32,
    pub avg_accuracy: Option<f32>,
    pub avg_completeness: Option<f32>,
    pub avg_relevance: Option<f32>,
    pub avg_latency_ms: Option<f32>,
    pub overall_score: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct RunDetailResponse {
    pub run: EvalRun,
    pub summaries: Vec<EvalSummary>,
}

#[derive(Debug, Serialize)]
pub struct MatrixCell {
    pub agent_name: String,
    pub model_id: String,
    pub overall_score: Option<f32>,
    pub avg_accuracy: Option<f32>,
    pub avg_completeness: Option<f32>,
    pub avg_relevance: Option<f32>,
    pub avg_latency_ms: Option<f32>,
    pub total_questions: i32,
}

#[derive(Debug, Serialize)]
pub struct MatrixResponse {
    pub agents: Vec<String>,
    pub models: Vec<String>,
    pub cells: Vec<MatrixCell>,
}

#[derive(Debug, Deserialize)]
pub struct ScoresQuery {
    pub agent: Option<String>,
    pub model: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct HumanReviewRequest {
    pub accuracy_score: Option<i8>,
    pub completeness_score: Option<i8>,
    pub relevance_score: Option<i8>,
    pub notes: Option<String>,
    pub reviewed_by: Option<String>,
}

// ─── Router ────────────────────────────────────────────────────────────

pub fn eval_routes() -> Router<DbPool> {
    Router::new()
        .route("/api/eval/runs", get(list_runs))
        .route("/api/eval/runs/{id}", get(get_run_detail))
        .route("/api/eval/runs/{id}/scores", get(get_run_scores))
        .route("/api/eval/runs/{id}/matrix", get(get_run_matrix))
        .route("/api/eval/scores/{id}/review", patch(submit_review))
}

// ─── Handlers ──────────────────────────────────────────────────────────

/// GET /api/eval/runs — List all evaluation runs
async fn list_runs(State(pool): State<DbPool>) -> Json<Vec<EvalRun>> {
    let runs = sqlx::query_as::<_, EvalRun>(
        "SELECT id, name, status, total_combinations, completed_combinations, started_at, finished_at, config FROM eval_runs ORDER BY started_at DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Json(runs)
}

/// GET /api/eval/runs/:id — Get run detail with summaries
async fn get_run_detail(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Json<Option<RunDetailResponse>> {
    let run = sqlx::query_as::<_, EvalRun>(
        "SELECT id, name, status, total_combinations, completed_combinations, started_at, finished_at, config FROM eval_runs WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let Some(run) = run else {
        return Json(None);
    };

    let summaries = sqlx::query_as::<_, EvalSummary>(
        "SELECT id, run_id, agent_name, model_id, total_questions, avg_accuracy, avg_completeness, avg_relevance, avg_latency_ms, overall_score FROM eval_summary WHERE run_id = ? ORDER BY overall_score DESC"
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Json(Some(RunDetailResponse { run, summaries }))
}

/// GET /api/eval/runs/:id/scores — Get individual scores with filtering
async fn get_run_scores(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
    Query(q): Query<ScoresQuery>,
) -> Json<Vec<EvalScore>> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = (page - 1) * limit;

    // Build dynamic query
    let mut query_str = String::from(
        "SELECT id, run_id, agent_name, model_id, question, expected_answer, actual_answer, accuracy_score, completeness_score, relevance_score, latency_ms, judge_model, judge_reasoning, human_accuracy_score, human_completeness_score, human_relevance_score, human_notes, reviewed_by, reviewed_at, created_at FROM eval_scores WHERE run_id = ?"
    );

    if q.agent.is_some() {
        query_str.push_str(" AND agent_name = ?");
    }
    if q.model.is_some() {
        query_str.push_str(" AND model_id = ?");
    }
    query_str.push_str(&format!(" ORDER BY agent_name, model_id, id LIMIT {} OFFSET {}", limit, offset));

    let mut query = sqlx::query_as::<_, EvalScore>(&query_str).bind(&id);

    if let Some(ref agent) = q.agent {
        query = query.bind(agent);
    }
    if let Some(ref model) = q.model {
        query = query.bind(model);
    }

    let scores = query.fetch_all(&pool).await.unwrap_or_default();
    Json(scores)
}

/// GET /api/eval/runs/:id/matrix — Get Agent×Model heatmap data
async fn get_run_matrix(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
) -> Json<MatrixResponse> {
    let summaries = sqlx::query_as::<_, EvalSummary>(
        "SELECT id, run_id, agent_name, model_id, total_questions, avg_accuracy, avg_completeness, avg_relevance, avg_latency_ms, overall_score FROM eval_summary WHERE run_id = ? ORDER BY agent_name, model_id"
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut agents: Vec<String> = summaries.iter().map(|s| s.agent_name.clone()).collect();
    agents.sort();
    agents.dedup();

    let mut models: Vec<String> = summaries.iter().map(|s| s.model_id.clone()).collect();
    models.sort();
    models.dedup();

    let cells: Vec<MatrixCell> = summaries
        .into_iter()
        .map(|s| MatrixCell {
            agent_name: s.agent_name,
            model_id: s.model_id,
            overall_score: s.overall_score,
            avg_accuracy: s.avg_accuracy,
            avg_completeness: s.avg_completeness,
            avg_relevance: s.avg_relevance,
            avg_latency_ms: s.avg_latency_ms,
            total_questions: s.total_questions,
        })
        .collect();

    Json(MatrixResponse { agents, models, cells })
}

/// PATCH /api/eval/scores/:id/review — Submit human review
async fn submit_review(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(body): Json<HumanReviewRequest>,
) -> Json<serde_json::Value> {
    let result = sqlx::query(
        r#"UPDATE eval_scores SET 
            human_accuracy_score = COALESCE(?, human_accuracy_score),
            human_completeness_score = COALESCE(?, human_completeness_score),
            human_relevance_score = COALESCE(?, human_relevance_score),
            human_notes = COALESCE(?, human_notes),
            reviewed_by = COALESCE(?, reviewed_by),
            reviewed_at = NOW()
        WHERE id = ?"#
    )
    .bind(body.accuracy_score)
    .bind(body.completeness_score)
    .bind(body.relevance_score)
    .bind(&body.notes)
    .bind(&body.reviewed_by)
    .bind(id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) => Json(serde_json::json!({
            "success": true,
            "rows_affected": r.rows_affected()
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}
