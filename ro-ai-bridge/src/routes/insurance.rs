use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;
use mimir_core_ai::services::db::DbPool;

pub fn insurance_routes() -> Router<DbPool> {
    Router::new()
        .route("/rag/search", post(rag_search))
        .route("/report/generate", post(generate_report))
}

#[derive(Deserialize)]
pub struct RagSearchRequest {
    pub query: String,
}

#[derive(Serialize)]
pub struct RagSearchResponse {
    pub status: String,
    pub results: Vec<Value>,
}

/// Mock RAG search for Insurance underwriting policies
async fn rag_search(Json(payload): Json<RagSearchRequest>) -> Json<RagSearchResponse> {
    info!("Insurance RAG Search Query: {}", payload.query);

    let query_lower = payload.query.to_lowercase();
    let mut results = vec![];

    if query_lower.contains("hba1c") || query_lower.contains("diabetes") {
        results.push(json!({
            "source": "Underwriting Guidelines 2026 - Endocrinology",
            "content": "For applicants with HbA1c levels > 6.5%, the application must be flagged for Human-in-the-Loop review. If HbA1c is between 6.0% and 6.5%, apply a standard +15% premium load.",
            "relevance_score": 0.95
        }));
    } else if query_lower.contains("blood pressure") || query_lower.contains("hypertension") {
        results.push(json!({
            "source": "Underwriting Guidelines 2026 - Cardiovascular",
            "content": "Systolic BP > 140 or Diastolic > 90 requires an additional echocardiogram report. If history of hypertension > 5 years, apply +10% premium load.",
            "relevance_score": 0.88
        }));
    } else {
        results.push(json!({
            "source": "General Underwriting Policy",
            "content": "Standard application process applies. No specific red flags detected for standard risk pool.",
            "relevance_score": 0.50
        }));
    }

    Json(RagSearchResponse {
        status: "success".into(),
        results,
    })
}

#[derive(Deserialize)]
pub struct GenerateReportRequest {
    pub decision: String,
    pub reasoning: String,
    pub conditions: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct GenerateReportResponse {
    pub status: String,
    pub report_id: String,
    pub recorded_decision: String,
}

/// Mock core system to generate a formal underwriting decision report (eBao stub)
async fn generate_report(Json(payload): Json<GenerateReportRequest>) -> Json<GenerateReportResponse> {
    info!(
        "Generating Underwriting Report. Decision: {} - Reasoning: {} - Conditions: {:?}",
        payload.decision, payload.reasoning, payload.conditions
    );

    let report_id = Uuid::new_v4().to_string();

    Json(GenerateReportResponse {
        status: "report_generated".into(),
        report_id,
        recorded_decision: payload.decision,
    })
}
