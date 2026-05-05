//! Admin Knowledge Routes — PrimeKG embedding pipeline
//!
//! POST /api/v1/admin/knowledge/primekg/embed        — trigger embedding job
//! GET  /api/v1/admin/knowledge/primekg/embed/status — poll progress

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::neo4j::{Neo4jConfig, Neo4jService};
use mimir_core_ai::services::qdrant::QdrantService;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::{info, warn};

use crate::routes::vector::embed_texts;

// ── Shared state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EmbedState {
    pub status: String,
    pub embedded: u64,
    pub total: u64,
    pub errors: u64,
    pub message: Option<String>,
}

impl Default for EmbedState {
    fn default() -> Self {
        Self {
            status: "idle".into(),
            embedded: 0,
            total: 0,
            errors: 0,
            message: None,
        }
    }
}

static EMBED_STATE: OnceLock<Arc<Mutex<EmbedState>>> = OnceLock::new();

fn embed_state() -> Arc<Mutex<EmbedState>> {
    EMBED_STATE
        .get_or_init(|| Arc::new(Mutex::new(EmbedState::default())))
        .clone()
}

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EmbedRequest {
    pub batch_size: Option<i64>,
    pub dry_run: Option<bool>,
    #[allow(dead_code)]
    pub type_filter: Option<String>,
}

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn admin_knowledge_routes() -> Router<DbPool> {
    Router::new()
        .route("/primekg/embed", post(trigger_embed))
        .route("/primekg/embed/status", get(get_embed_status))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn get_embed_status(
    State(_pool): State<DbPool>,
) -> Json<serde_json::Value> {
    let state = embed_state();
    let s = state.lock().unwrap();
    Json(json!({
        "status": s.status,
        "embedded": s.embedded,
        "total": s.total,
        "errors": s.errors,
        "message": s.message,
    }))
}

async fn trigger_embed(
    State(_pool): State<DbPool>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let state = embed_state();

    // Reject if already running
    {
        let s = state.lock().unwrap();
        if s.status == "running" {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Embedding already in progress"})),
            ));
        }
    }

    let batch_size = req.batch_size.unwrap_or(500).min(1000).max(50);
    let dry_run = req.dry_run.unwrap_or(false);

    // Init Neo4j
    let neo4j = {
        let cfg = Neo4jConfig::from_env();
        match Neo4jService::try_new(&cfg).await {
            Some(svc) => Arc::new(svc),
            None => {
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error": "Neo4j unavailable"})),
                ));
            }
        }
    };

    // Get total count
    let total = neo4j.count_primekg_nodes().await.unwrap_or(0);
    if total == 0 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "No PrimeKG nodes found in Neo4j"})),
        ));
    }

    let embed_model = std::env::var("EMBED_MODEL").unwrap_or_else(|_| "BAAI/bge-m3".to_string());

    // Mark running
    {
        let mut s = state.lock().unwrap();
        *s = EmbedState {
            status: "running".into(),
            embedded: 0,
            total: if dry_run { batch_size as u64 } else { total as u64 },
            errors: 0,
            message: Some(if dry_run {
                format!("Dry run: embedding {} nodes", batch_size)
            } else {
                format!("Embedding {} nodes", total)
            }),
        };
    }

    let state_bg = state.clone();
    tokio::spawn(async move {
        let qdrant = QdrantService::new();

        if let Err(e) = qdrant.init_collection("primekg-entities", 1024).await {
            let mut s = state_bg.lock().unwrap();
            s.status = "failed".into();
            s.message = Some(format!("Failed to init Qdrant collection: {}", e));
            return;
        }

        let max_batches = if dry_run { 1i64 } else { (total / batch_size) + 1 };
        let mut offset = 0i64;
        let mut embedded_total = 0u64;
        let mut error_count = 0u64;

        for _ in 0..max_batches {
            let nodes = match neo4j.stream_primekg_nodes(offset, batch_size).await {
                Ok(n) => n,
                Err(e) => {
                    warn!("PrimeKG batch error at offset {}: {}", offset, e);
                    error_count += 1;
                    break;
                }
            };

            if nodes.is_empty() {
                break;
            }

            let texts: Vec<String> = nodes.iter().map(|n| n.to_embed_text()).collect();

            let vectors = match embed_texts(&texts, &embed_model).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("Embed error at offset {}: {}", offset, e);
                    error_count += 1;
                    offset += batch_size;
                    continue;
                }
            };

            let points: Vec<serde_json::Value> = nodes.iter().zip(vectors.iter()).map(|(node, vec)| {
                let sparse = mimir_core_ai::services::bm25::text_to_sparse_vector(&node.to_embed_text());
                json!({
                    "id": node.entity_index as u64,
                    "vector": {
                        "dense": vec,
                        "bm25": {
                            "indices": sparse.indices,
                            "values": sparse.values,
                        }
                    },
                    "payload": {
                        "entity_index": node.entity_index,
                        "name": node.name,
                        "entity_type": node.entity_type,
                        "source": node.source,
                        "tenant_id": serde_json::Value::Null,
                        "is_active": serde_json::Value::Null,
                    }
                })
            }).collect();

            let body = json!({ "points": points });

            match qdrant.upsert_points("primekg-entities", body).await {
                Ok(_) => {
                    embedded_total += nodes.len() as u64;
                    info!("PrimeKG embed: {} / {} nodes", embedded_total, total);
                }
                Err(e) => {
                    warn!("Qdrant upsert error at offset {}: {}", offset, e);
                    error_count += 1;
                }
            }

            {
                let mut s = state_bg.lock().unwrap();
                s.embedded = embedded_total;
                s.errors = error_count;
            }

            offset += batch_size;
        }

        let mut s = state_bg.lock().unwrap();
        s.status = if error_count == 0 { "completed" } else { "completed" }.into();
        s.embedded = embedded_total;
        s.errors = error_count;
        s.message = Some(format!(
            "{} {} nodes embedded ({} errors)",
            if dry_run { "Dry run:" } else { "Done:" },
            embedded_total,
            error_count
        ));
        info!("PrimeKG embed complete: {} nodes, {} errors", embedded_total, error_count);
    });

    let s = state.lock().unwrap();
    Ok(Json(json!({
        "status": s.status,
        "embedded": s.embedded,
        "total": s.total,
        "errors": s.errors,
        "message": s.message,
    })))
}
