use anyhow::Result;
use axum::{
    routing::{get, post},
    Router, Json, extract::{State, Path},
    response::IntoResponse,
    http::StatusCode,
};
use dotenvy::dotenv;
use ro_ai_bridge::services::db::{init_db, DbPool};
use ro_ai_bridge::agents::wiki_workshop::pipeline::run_pipeline;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{info, error};
use tokio::net::TcpListener;

#[derive(Deserialize)]
struct RunRequest {
    provider: Option<String>,
    model: Option<String>,
    test_run: Option<bool>,
}

#[derive(Serialize)]
struct RunResponse {
    run_id: String,
}

struct AppState {
    db: DbPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let pool = init_db().await?;
    let state = Arc::new(AppState { db: pool });

    let app = Router::new()
        .route("/api/pipeline/run", post(trigger_run))
        .route("/api/pipeline/runs", get(list_runs))
        .route("/api/pipeline/runs/{id}", get(get_run_details))
        .route("/api/pipeline/steps/{id}/qa", get(get_step_qa))
        .route("/api/pipeline/steps/{id}/report", get(get_step_report))
        .route("/api/pipeline/steps/{id}/retry", post(retry_step_handler))
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state);

    let port = env::var("MONITOR_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    info!("🚀 Monitor API running on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn trigger_run(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RunRequest>,
) -> Json<RunResponse> {
    let provider = payload.provider.unwrap_or_else(|| "ollama".to_string());
    let model = payload.model.unwrap_or_else(|| "llama3.2".to_string());
    let is_test = payload.test_run.unwrap_or(false);

    let db = state.db.clone();
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_id_inner = run_id.clone();
    
    // Run in background
    tokio::spawn(async move {
        if let Err(e) = run_pipeline(&db, run_id_inner, &provider, &model, "data/wiki", is_test).await {
            error!("Background pipeline failed: {}", e);
        }
    });

    Json(RunResponse { run_id })
}

use sqlx::Row;

async fn list_runs(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let runs = sqlx::query("SELECT * FROM pipeline_runs ORDER BY started_at DESC LIMIT 50")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let json_runs = runs.into_iter().map(|r| {
        serde_json::json!({
            "id": r.get::<String, _>("id"),
            "status": r.get::<String, _>("status"),
            "provider": r.get::<String, _>("provider"),
            "model": r.get::<String, _>("model"),
            "started_at": r.get::<chrono::DateTime<chrono::Utc>, _>("started_at"),
            "finished_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at"),
        })
    }).collect();

    Json(json_runs)
}

async fn get_run_details(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let run = sqlx::query("SELECT * FROM pipeline_runs WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or_default();

    if let Some(r) = run {
        let steps = sqlx::query("SELECT * FROM pipeline_steps WHERE run_id = ?")
            .bind(&id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

        let json_steps: Vec<_> = steps.into_iter().map(|s| {
            serde_json::json!({
                "id": s.get::<i64, _>("id"),
                "file_name": s.get::<String, _>("file_name"),
                "chunk_index": s.get::<i64, _>("chunk_index"),
                "status": s.get::<String, _>("status"),
                "step_type": s.get::<String, _>("step_type"),
            })
        }).collect();

        Json(serde_json::json!({
            "id": r.get::<String, _>("id"),
            "status": r.get::<String, _>("status"),
            "provider": r.get::<String, _>("provider"),
            "model": r.get::<String, _>("model"),
            "started_at": r.get::<chrono::DateTime<chrono::Utc>, _>("started_at"),
            "finished_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at"),
            "steps": json_steps
        }))
    } else {
        Json(serde_json::json!({"error": "Not found"}))
    }
}

async fn get_step_qa(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let qa_list = sqlx::query("SELECT * FROM qa_results WHERE step_id = ?")
        .bind(id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let json_list = qa_list.into_iter().map(|q| {
        serde_json::json!({
            "id": q.get::<i64, _>("id"),
            "question": q.get::<String, _>("question"),
            "answer": q.get::<String, _>("answer"),
            "context": q.get::<Option<String>, _>("context"),
        })
    }).collect();

    Json(json_list)
}

async fn get_step_report(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let report = sqlx::query("SELECT * FROM evaluation_reports WHERE step_id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or_default();

    if let Some(r) = report {
        // Helper to safely read string or blob
        let read_string_or_blob = |row: &sqlx::mysql::MySqlRow, col: &str| -> Option<String> {
            // Try reading as String first
            match row.try_get::<Option<String>, _>(col) {
                Ok(s) => s,
                Err(_) => {
                    // Fallback: try reading as Vec<u8> (BLOB) and convert to String
                    row.try_get::<Option<Vec<u8>>, _>(col)
                       .ok()
                       .flatten()
                       .and_then(|bytes| String::from_utf8(bytes).ok())
                }
            }
        };

        let atomic_facts_raw = read_string_or_blob(&r, "atomic_facts");
        let missing_facts_raw = read_string_or_blob(&r, "missing_facts");
        
        let atomic_facts: serde_json::Value = atomic_facts_raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null);

        let missing_facts: serde_json::Value = missing_facts_raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .map(|v: serde_json::Value| {
                // Check if it's an array of objects with "fact" key, and flatten
                if let Some(arr) = v.as_array() {
                    let strings: Vec<String> = arr.iter().filter_map(|item| {
                        if let Some(s) = item.as_str() {
                            Some(s.to_string())
                        } else {
                            item.get("fact").and_then(|f| f.as_str()).map(|s| s.to_string())
                        }
                    }).collect();
                    serde_json::to_value(strings).unwrap_or(serde_json::Value::Null)
                } else {
                    v
                }
            })
            .unwrap_or(serde_json::Value::Null);

        Json(serde_json::json!({
            "id": r.get::<i64, _>("id"),
            "coverage_score": r.get::<f32, _>("coverage_score"),
            "reasoning": r.get::<Option<String>, _>("reasoning"),
            "atomic_facts": atomic_facts,
            "missing_facts": missing_facts,
        })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Report not found"}))
        ).into_response()
    }
}

async fn retry_step_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.clone();
    
    // Update status to RUNNING immediately so frontend sees it on next fetch
    let _ = sqlx::query("UPDATE pipeline_steps SET status = 'RUNNING', error_message = NULL, started_at = NOW() WHERE id = ?")
        .bind(id)
        .execute(&db)
        .await;

    // Run heavy processing in background
    tokio::spawn(async move {
        if let Err(e) = ro_ai_bridge::agents::wiki_workshop::pipeline::retry_step(&db, id).await {
            error!("Background retry Step #{} failed: {}", id, e);
        }
    });

    StatusCode::ACCEPTED
}
