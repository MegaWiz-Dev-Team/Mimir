//! Sprint 47 B-47g — RAG benchmark items (clinician-curated gold)
//!
//! Powers Sprint 47 retrieval metrics (Recall@k, MRR, NDCG@k) by recording
//! "for question X in benchmark Y, these chunks ARE relevant" labeled by
//! clinicians. Once present, eval_runner can compute pure-Rust retrieval
//! metrics against retrieved_chunk_ids (Sprint 47 B-47c).
//!
//! Endpoints:
//!   GET    /api/v1/rag-benchmark/items?benchmark_id=...
//!          — list gold items for a benchmark (paginated, tenant-scoped)
//!   GET    /api/v1/rag-benchmark/items/:question_id/candidates?collection_id=...
//!          — pull recent retrieval_chunks from eval_scores for this question
//!            so the clinician sees concrete chunk IDs to label (rather than
//!            typing them by hand). Sources: most-recent eval run for the
//!            same tenant + question.
//!   POST   /api/v1/rag-benchmark/items
//!          — create gold item: { benchmark_id, question_id, collection_id,
//!            relevant_chunk_ids[], required_topics[], notes? }
//!   PATCH  /api/v1/rag-benchmark/items/:id
//!          — update relevant_chunk_ids / required_topics / notes
//!
//! Tenant scope: tenant_id required (defaults to caller's tenant). Per-tenant
//! gold; future shared baselines via tenant_id NULL (deferred).

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, FromRow)]
pub struct RagBenchmarkItem {
    pub id: String,
    pub benchmark_id: String,
    pub question_id: String,
    pub collection_id: String,
    pub relevant_chunk_ids: JsonValue,
    pub required_topics: Option<JsonValue>,
    pub notes: Option<String>,
    pub tenant_id: String,
    pub curated_by: Option<String>,
    pub curated_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub benchmark_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CandidatesQuery {
    pub collection_id: Option<String>,
    /// Optional: limit number of candidate chunks returned (default 32, max 100).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CandidateChunk {
    pub chunk_id: String,
    pub source: String,
    pub title: String,
    pub score: f64,
    pub content_preview: String,
    /// True if this chunk is already in the gold set for the question.
    pub already_gold: bool,
}

#[derive(Debug, Serialize)]
pub struct CandidatesResponse {
    pub question_id: String,
    pub candidates: Vec<CandidateChunk>,
    pub source_run_id: Option<String>,
    pub source_run_started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateItemRequest {
    pub benchmark_id: String,
    pub question_id: String,
    pub collection_id: String,
    pub relevant_chunk_ids: Vec<String>,
    #[serde(default)]
    pub required_topics: Option<Vec<String>>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateItemRequest {
    #[serde(default)]
    pub relevant_chunk_ids: Option<Vec<String>>,
    #[serde(default)]
    pub required_topics: Option<Vec<String>>,
    #[serde(default)]
    pub notes: Option<String>,
}

// ─── Router ──────────────────────────────────────────────────────────────────

pub fn rag_benchmark_routes() -> Router<DbPool> {
    Router::new()
        .route(
            "/api/v1/rag-benchmark/items",
            get(list_items).post(create_item),
        )
        .route(
            "/api/v1/rag-benchmark/items/{id}",
            axum::routing::patch(update_item),
        )
        .route(
            "/api/v1/rag-benchmark/items/{question_id}/candidates",
            get(list_candidates),
        )
        .layer(axum::middleware::from_fn(tenant_auth_middleware))
}

// ─── Handlers ───────────────────────────────────────────────────────────────

async fn list_items(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Query(req): Query<ListQuery>,
) -> Json<Vec<RagBenchmarkItem>> {
    let limit = req.limit.unwrap_or(50).min(200) as i64;
    let offset = req.offset.unwrap_or(0) as i64;
    let rows = sqlx::query_as::<_, RagBenchmarkItem>(
        "SELECT id, benchmark_id, question_id, collection_id,
                relevant_chunk_ids, required_topics, notes, tenant_id,
                curated_by, curated_at, updated_at
         FROM rag_benchmark_items
         WHERE benchmark_id = ? AND tenant_id = ?
         ORDER BY curated_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(&req.benchmark_id)
    .bind(&tenant.tenant_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(event = "list_rag_gold_failed", tenant = %tenant.tenant_id, error = %e);
        vec![]
    });
    Json(rows)
}

async fn list_candidates(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(question_id): Path<String>,
    Query(req): Query<CandidatesQuery>,
) -> Result<Json<CandidatesResponse>, (StatusCode, String)> {
    let limit = req.limit.unwrap_or(32).min(100);

    // Pull most-recent eval_scores row for this tenant + question with
    // non-empty retrieval_chunks. We surface its retrieval_chunks JSON as
    // candidate set. The clinician then picks which chunk_ids are "gold".
    let row: Option<(String, DateTime<Utc>, Option<String>)> = sqlx::query_as(
        "SELECT s.run_id, r.started_at, s.retrieval_chunks
           FROM eval_scores s
           JOIN eval_runs r ON r.id = s.run_id
          WHERE s.tenant_id = ?
            AND s.benchmark_item_id = ?
            AND s.retrieval_chunks IS NOT NULL
            AND JSON_LENGTH(s.retrieval_chunks) > 0
          ORDER BY r.started_at DESC
          LIMIT 1",
    )
    .bind(&tenant.tenant_id)
    .bind(&question_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;

    let (source_run_id, source_started_at, chunks_json_str) = match row {
        Some((id, ts, j)) => (Some(id), Some(ts), j.unwrap_or_default()),
        None => (None, None, String::new()),
    };

    // Gold set already labeled — flag overlapping chunks.
    let gold_row: Option<(JsonValue,)> = sqlx::query_as(
        "SELECT relevant_chunk_ids FROM rag_benchmark_items
          WHERE tenant_id = ? AND question_id = ?
          ORDER BY curated_at DESC LIMIT 1",
    )
    .bind(&tenant.tenant_id)
    .bind(&question_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("gold lookup: {e}")))?;

    let gold_set: std::collections::HashSet<String> = gold_row
        .as_ref()
        .and_then(|(v,)| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut candidates: Vec<CandidateChunk> = if chunks_json_str.is_empty() {
        vec![]
    } else {
        serde_json::from_str::<Vec<JsonValue>>(&chunks_json_str)
            .unwrap_or_default()
            .into_iter()
            .take(limit as usize)
            .map(|c| {
                let chunk_id = c.get("chunk_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let already_gold = !chunk_id.is_empty() && gold_set.contains(&chunk_id);
                CandidateChunk {
                    chunk_id,
                    source: c.get("source").and_then(|v| v.as_str()).unwrap_or("").into(),
                    title: c.get("title").and_then(|v| v.as_str()).unwrap_or("").into(),
                    score: c.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    content_preview: c.get("content_preview")
                        .and_then(|v| v.as_str()).unwrap_or("").into(),
                    already_gold,
                }
            })
            .collect()
    };

    // Filter by collection_id if requested (source field maps roughly to
    // collection_id — vector / primekg / clinical / graph / tree).
    if let Some(coll) = req.collection_id.as_deref() {
        candidates.retain(|c| c.source == coll);
    }

    Ok(Json(CandidatesResponse {
        question_id,
        candidates,
        source_run_id,
        source_run_started_at: source_started_at,
    }))
}

async fn create_item(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Json(req): Json<CreateItemRequest>,
) -> Result<Json<RagBenchmarkItem>, (StatusCode, String)> {
    if req.relevant_chunk_ids.is_empty() {
        return Err((StatusCode::BAD_REQUEST,
            "relevant_chunk_ids must not be empty".into()));
    }
    let id = Uuid::new_v4().to_string();
    let chunk_ids_json = serde_json::to_string(&req.relevant_chunk_ids)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("encode chunks: {e}")))?;
    let topics_json = req.required_topics
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".into()));

    sqlx::query(
        "INSERT INTO rag_benchmark_items
         (id, benchmark_id, question_id, collection_id,
          relevant_chunk_ids, required_topics, notes,
          tenant_id, curated_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON DUPLICATE KEY UPDATE
           relevant_chunk_ids = VALUES(relevant_chunk_ids),
           required_topics = VALUES(required_topics),
           notes = VALUES(notes),
           updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&id)
    .bind(&req.benchmark_id)
    .bind(&req.question_id)
    .bind(&req.collection_id)
    .bind(&chunk_ids_json)
    .bind(&topics_json)
    .bind(&req.notes)
    .bind(&tenant.tenant_id)
    .bind(&tenant.user_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("insert: {e}")))?;

    let row = sqlx::query_as::<_, RagBenchmarkItem>(
        "SELECT id, benchmark_id, question_id, collection_id,
                relevant_chunk_ids, required_topics, notes, tenant_id,
                curated_by, curated_at, updated_at
         FROM rag_benchmark_items
         WHERE benchmark_id = ? AND question_id = ? AND tenant_id = ?",
    )
    .bind(&req.benchmark_id)
    .bind(&req.question_id)
    .bind(&tenant.tenant_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read-back: {e}")))?;

    tracing::info!(
        event = "rag_gold_create",
        tenant = %tenant.tenant_id,
        benchmark = %req.benchmark_id,
        question = %req.question_id,
        n_chunks = req.relevant_chunk_ids.len(),
    );

    Ok(Json(row))
}

async fn update_item(
    State(pool): State<DbPool>,
    Extension(tenant): Extension<TenantContext>,
    Path(id): Path<String>,
    Json(req): Json<UpdateItemRequest>,
) -> Result<Json<RagBenchmarkItem>, (StatusCode, String)> {
    let chunk_ids_json = req.relevant_chunk_ids
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let topics_json = req.required_topics
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".into()));

    let updated = sqlx::query(
        "UPDATE rag_benchmark_items
            SET relevant_chunk_ids = COALESCE(?, relevant_chunk_ids),
                required_topics    = COALESCE(?, required_topics),
                notes              = COALESCE(?, notes),
                updated_at         = CURRENT_TIMESTAMP
          WHERE id = ? AND tenant_id = ?",
    )
    .bind(&chunk_ids_json)
    .bind(&topics_json)
    .bind(&req.notes)
    .bind(&id)
    .bind(&tenant.tenant_id)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("update: {e}")))?;

    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, format!("rag_benchmark_item not found: {id}")));
    }

    let row = sqlx::query_as::<_, RagBenchmarkItem>(
        "SELECT id, benchmark_id, question_id, collection_id,
                relevant_chunk_ids, required_topics, notes, tenant_id,
                curated_by, curated_at, updated_at
         FROM rag_benchmark_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read-back: {e}")))?;

    Ok(Json(row))
}
