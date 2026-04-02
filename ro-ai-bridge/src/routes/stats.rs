use axum::{http::StatusCode, routing::get, Json, Router};
use serde::Serialize;
use serde_json::{json, Value};

use mimir_core_ai::services::db::DbPool;

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_sources: i64,
    pub total_chunks: i64,
    pub qa_pairs: i64,
    pub vector_coverage: f64,
    pub source_health: SourceHealth,
}

#[derive(Debug, Serialize)]
pub struct SourceHealth {
    pub healthy: i64,
    pub failed: i64,
    pub pending: i64,
    pub running: i64,
}

pub fn stats_routes() -> Router<DbPool> {
    Router::new().route("/stats", get(get_stats))
}

async fn get_stats(
    axum::extract::State(pool): axum::extract::State<DbPool>,
) -> Result<Json<StatsResponse>, (StatusCode, Json<Value>)> {
    // Total sources
    let total_sources: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM data_sources")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    // Total chunks
    let total_chunks: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chunks")
        .fetch_one(&pool)
        .await
        .unwrap_or((0,));

    // QA pairs (count from pipeline_steps where step = qa_generation)
    let qa_pairs: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pipeline_steps WHERE step_name = 'qa_generation'")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    // Source health counts
    let healthy: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM data_sources WHERE last_sync_status = 'COMPLETED'")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    let failed: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM data_sources WHERE last_sync_status = 'FAILED'")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    let pending: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM data_sources WHERE last_sync_status IS NULL OR last_sync_status = 'PENDING'"
    ).fetch_one(&pool).await.unwrap_or((0,));

    let running: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM data_sources WHERE last_sync_status = 'RUNNING'")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    // Vector coverage: (sources with total_chunks > 0) / total_sources * 100
    // Simplified: since we don't have vector indexing status yet, use chunk coverage
    let sources_with_chunks: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM data_sources WHERE total_chunks > 0")
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

    let vector_coverage = if total_sources.0 > 0 {
        (sources_with_chunks.0 as f64 / total_sources.0 as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(StatsResponse {
        total_sources: total_sources.0,
        total_chunks: total_chunks.0,
        qa_pairs: qa_pairs.0,
        vector_coverage,
        source_health: SourceHealth {
            healthy: healthy.0,
            failed: failed.0,
            pending: pending.0,
            running: running.0,
        },
    }))
}
