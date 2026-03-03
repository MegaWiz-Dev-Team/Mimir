//! Quality Control API routes
//!
//! Provides REST endpoints for the QC dashboard:
//! - GET  /api/v1/qc/clusters       — List all QA clusters (optionally filtered by status)
//! - POST /api/v1/qc/resolve/:id    — Resolve a cluster with a golden answer
//! - POST /api/v1/qc/generate       — Trigger background generation of new clusters

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::qa_qc::clustering::{ClusteringService, ClusterDTO, ResolveClusterRequest};
use axum::http::HeaderMap;
use crate::routes::tenant::extract_tenant_id;

#[derive(Debug, Deserialize)]
pub struct SeedData {
    pub question: String,
    pub answer: String,
    pub context: String,
}

#[derive(Debug, Deserialize)]
pub struct QcQuery {
    pub status: Option<String>,
}

// ─── Router ────────────────────────────────────────────────────────────

pub fn qc_routes() -> Router<DbPool> {
    Router::new()
        .route("/clusters", get(list_clusters))
        .route("/resolve/{id}", post(resolve_cluster))
        .route("/generate", post(trigger_generate))
        .route("/status", get(get_qc_status))
        .route("/seed", post(seed_qa_data))
}

// ─── Handlers ──────────────────────────────────────────────────────────

/// GET /api/v1/qc/clusters — List QA clusters
async fn list_clusters(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<QcQuery>,
) -> Json<Vec<ClusterDTO>> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    let status_filter = q.status.as_deref().filter(|s| !s.is_empty()).unwrap_or("PENDING");
    
    match ClusteringService::get_clusters(&pool, &tenant_id, Some(status_filter)).await {
        Ok(clusters) => Json(clusters),
        Err(e) => {
            tracing::error!("Failed to fetch clusters: {}", e);
            Json(vec![]) // Consider better error handling mapped to HTTP status codes
        }
    }
}

/// POST /api/v1/qc/resolve/:id — Resolve a cluster
async fn resolve_cluster(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
    Json(req): Json<ResolveClusterRequest>,
) -> Json<serde_json::Value> {
    match ClusteringService::resolve_cluster(&pool, &id, req).await {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "Cluster resolved successfully"
        })),
        Err(e) => {
            tracing::error!("Failed to resolve cluster {}: {}", id, e);
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// POST /api/v1/qc/generate — Trigger QC cluster generation
async fn trigger_generate(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers).to_string();

    // Note: In a real system, you might want to fire and forget this or use a background worker queue.
    // For now, we await it or spawn it. Spawning is safer to not block the HTTP response if Gemini takes long.
    
    let pool_clone = pool.clone();
    
    tokio::spawn(async move {
        if let Err(e) = ClusteringService::trigger_clustering(&pool_clone, &tenant_id).await {
            tracing::error!("Background clustering failed for tenant {}: {}", tenant_id, e);
        }
    });

    Json(serde_json::json!({
        "success": true,
        "message": "QC clustering job started in background"
    }))
}

/// POST /api/v1/qc/seed — Temporary DB seeding for testing
async fn seed_qa_data(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    axum::Json(payload): axum::Json<Vec<SeedData>>,
) -> Json<serde_json::Value> {
    let tenant_id = extract_tenant_id(&headers);
    let mut success_count = 0;
    
    // Create mock run
    let run_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query("INSERT INTO pipeline_runs (id, status, provider, model) VALUES (?, 'COMPLETED', 'mock', 'mock')")
        .bind(&run_id).execute(&pool).await;
        
    // Create mock step
    let _ = sqlx::query("INSERT INTO pipeline_steps (run_id, file_name, status, step_type) VALUES (?, 'mock_file.txt', 'COMPLETED', 'GENERATE')")
        .bind(&run_id).execute(&pool).await;
        
    let step_record = sqlx::query!("SELECT id FROM pipeline_steps WHERE run_id = ? LIMIT 1", run_id).fetch_one(&pool).await;
    
    if let Ok(step_record) = step_record {
        for item in payload {
            let res = sqlx::query(
                r#"
                INSERT INTO qa_results (step_id, question, answer, context, tenant_id)
                VALUES (?, ?, ?, ?, ?)
                "#)
                .bind(step_record.id)
                .bind(item.question)
                .bind(item.answer)
                .bind(item.context)
                .bind(tenant_id)
                .execute(&pool).await;
            
            if res.is_ok() {
                success_count += 1;
            } else if let Err(e) = res {
                 tracing::error!("Seed failed insert: {}", e);
            }
        }
    }
    
    Json(serde_json::json!({ "inserted": success_count }))
}

/// GET /api/v1/qc/status — Check if background clustering is running
async fn get_qc_status() -> Json<serde_json::Value> {
    let is_running = mimir_core_ai::qa_qc::clustering::IS_CLUSTERING_RUNNING.load(std::sync::atomic::Ordering::SeqCst);
    let processed = mimir_core_ai::qa_qc::clustering::PROCESSED_COUNT.load(std::sync::atomic::Ordering::SeqCst);
    let total = mimir_core_ai::qa_qc::clustering::TOTAL_COUNT.load(std::sync::atomic::Ordering::SeqCst);
    
    Json(serde_json::json!({
        "is_generating": is_running,
        "processed_count": processed,
        "total_count": total
    }))
}
