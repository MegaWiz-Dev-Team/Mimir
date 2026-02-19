use anyhow::Result;
use axum::{
    routing::{get, post},
    Router, Json, extract::{State, Path},
    response::{IntoResponse, sse::Event, Sse},
    http::StatusCode,
};
use dotenvy::dotenv;
use ro_ai_bridge::services::db::{init_db, DbPool};
use ro_ai_bridge::agents::wiki_workshop::pipeline::{run_pipeline, resume_pipeline};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{info, error};
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;
use futures::stream::Stream;

use ro_ai_bridge::services::qdrant::QdrantService;
use ro_ai_bridge::agents::wiki_workshop::indexer::run_indexer;
use ro_ai_bridge::agents::simple_npc::SimpleNpcAgent;
use ro_ai_bridge::agents::oracle_rag::OracleRagAgent;
use ro_ai_bridge::models::persona::Persona;
use rig::providers::ollama;

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

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    limit: Option<usize>,
}

/// Chat request for agent interactions
#[derive(Deserialize)]
struct ChatRequest {
    /// Agent tier: 1 = Simple NPC (no RAG), 2 = Oracle RAG
    tier: i8,
    /// User message
    message: String,
    /// Persona name (e.g., "sage_ariel")
    persona: String,
    /// Optional session ID for conversation continuity
    session_id: Option<String>,
}

/// Chat response for non-streaming responses
#[derive(Serialize)]
struct ChatResponse {
    content: String,
    tier: i8,
    persona: String,
    latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools_used: Option<Vec<String>>,
}

/// SSE event types for streaming
#[derive(Serialize)]
struct StreamToken {
    token: String,
}

#[derive(Serialize)]
struct StreamDone {
    latency_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<serde_json::Value>>,
}

struct AppState {
    db: DbPool,
    qdrant: QdrantService,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let pool = init_db().await?;
    let qdrant = QdrantService::new();
    let state = Arc::new(AppState { 
        db: pool,
        qdrant,
    });

    let app = Router::new()
        .route("/api/pipeline/run", post(trigger_run))
        .route("/api/pipeline/runs", get(list_runs))
        .route("/api/pipeline/runs/{id}", get(get_run_details))
        .route("/api/pipeline/steps/{id}/qa", get(get_step_qa))
        .route("/api/pipeline/steps/{id}/report", get(get_step_report))
        .route("/api/pipeline/steps/{id}/retry", post(retry_step_handler))
        .route("/api/pipeline/runs/{id}/resume", post(resume_run_handler))
        .route("/api/vector/stats", get(get_vector_stats))
        .route("/api/vector/index", post(trigger_indexing))
        .route("/api/vector/search", post(search_vectors))
        // Agent chat endpoints
        .route("/api/agents/chat", post(chat_handler))
        .route("/api/agents/chat/stream", post(chat_stream_handler))
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state);

    let port = env::var("MONITOR_PORT").unwrap_or_else(|_| "8080".to_string());
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

async fn resume_run_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.clone();
    
    // Check if run exists
    let run = sqlx::query("SELECT id FROM pipeline_runs WHERE id = ?")
        .bind(&id)
        .fetch_optional(&db)
        .await
        .unwrap_or_default();

    if run.is_none() {
         return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Run not found"}))).into_response();
    }

    // Run resume in background
    let run_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = resume_pipeline(&db, run_id.clone()).await {
             error!("Background resume Run #{} failed: {}", run_id, e);
             let _ = sqlx::query("UPDATE pipeline_runs SET status = 'FAILED' WHERE id = ?")
                .bind(&run_id)
                .execute(&db).await;
        }
    });

    StatusCode::ACCEPTED.into_response()
}

async fn get_vector_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let collection_name = "wiki_qa";
    
    // 1. Get Qdrant stats
    let qdrant_info = state.qdrant.get_collection_info(collection_name).await.unwrap_or(serde_json::Value::Null);
    
    // 2. Get MariaDB stats
    let total_qa = sqlx::query("SELECT count(*) as count FROM qa_results")
        .fetch_one(&state.db)
        .await
        .map(|r| r.get::<i64, _>("count"))
        .unwrap_or(0);

    let indexed_qa = sqlx::query("SELECT count(*) as count FROM qa_results WHERE indexed_at IS NOT NULL")
        .fetch_one(&state.db)
        .await
        .map(|r| r.get::<i64, _>("count"))
        .unwrap_or(0);

    Json(serde_json::json!({
        "qdrant": qdrant_info,
        "database": {
            "total_qa": total_qa,
            "indexed_qa": indexed_qa,
            "pending_qa": total_qa - indexed_qa
        }
    }))
}

async fn trigger_indexing(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.clone();
    let qdrant = QdrantService::new();
    
    tokio::spawn(async move {
        let ollama_client = ollama::Client::new();
        if let Err(e) = run_indexer(&db, &qdrant, &ollama_client, "wiki_qa").await {
            error!("Background indexing failed: {}", e);
        }
    });

    StatusCode::ACCEPTED
}

async fn search_vectors(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SearchRequest>,
) -> impl IntoResponse {
    use rig::embeddings::EmbeddingModel;
    
    let ollama_client = ollama::Client::new();
    let embed_model = ollama_client.embedding_model("nomic-embed-text");
    
    match embed_model.embed_text(&payload.query).await {
        Ok(embedding) => {
            let vector_f32: Vec<f32> = embedding.vec.into_iter().map(|f| f as f32).collect();
            match state.qdrant.search("wiki_qa", vector_f32, payload.limit.unwrap_or(5)).await {
                Ok(results) => Json(results).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
            }
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    }
}

// ─── Agent Chat Handlers ────────────────────────────────────────────────────

/// Handle chat requests for both Tier 1 and Tier 2 agents
async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    
    // Load persona
    let persona = match Persona::load_by_name_cached(&payload.persona) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Persona not found: {}", e)}))
            ).into_response();
        }
    };
    
    // Route to appropriate agent based on tier
    match payload.tier {
        1 => {
            // Tier 1: Simple NPC (no RAG)
            let agent = SimpleNpcAgent::new(persona);
            
            match agent.chat(&payload.message).await {
                Ok(response) => {
                    let chat_response = ChatResponse {
                        content: response,
                        tier: 1,
                        persona: payload.persona,
                        latency_ms: start.elapsed().as_millis() as u64,
                        confidence_score: None,
                        confidence_level: None,
                        sources: None,
                        tools_used: None,
                    };
                    Json(chat_response).into_response()
                }
                Err(e) => {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Agent error: {}", e)}))
                    ).into_response()
                }
            }
        }
        2 => {
            // Tier 2: Oracle RAG
            let agent = OracleRagAgent::new(
                persona,
                state.qdrant.clone(),
                Some(state.db.clone()),
            );
            
            match agent.chat(&payload.message).await {
                Ok(response) => {
                    let chat_response = ChatResponse {
                        content: response.content,
                        tier: 2,
                        persona: payload.persona,
                        latency_ms: response.latency_ms,
                        confidence_score: Some(response.confidence_score),
                        confidence_level: Some(format!("{:?}", response.confidence_level)),
                        sources: Some(response.sources.iter().map(|s| {
                            serde_json::json!({
                                "source_type": s.source_type,
                                "source_id": s.source_id,
                                "relevance": s.relevance,
                                "snippet": s.snippet
                            })
                        }).collect()),
                        tools_used: Some(response.tools_used),
                    };
                    Json(chat_response).into_response()
                }
                Err(e) => {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("Agent error: {}", e)}))
                    ).into_response()
                }
            }
        }
        _ => {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid tier. Must be 1 or 2"}))
            ).into_response()
        }
    }
}

/// Handle streaming chat requests using SSE
async fn chat_stream_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let (tx, rx) = mpsc::channel(100);
    let start = std::time::Instant::now();
    
    // Clone needed data for the spawned task
    let persona_name = payload.persona.clone();
    let message = payload.message.clone();
    let tier = payload.tier;
    let db = state.db.clone();
    let qdrant = state.qdrant.clone();
    
    // Spawn task to generate response
    tokio::spawn(async move {
        // Load persona
        let persona = match Persona::load_by_name_cached(&persona_name) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(Event::default()
                    .event("error")
                    .json_data(serde_json::json!({"error": format!("Persona not found: {}", e)}))
                ).await;
                return;
            }
        };
        
        match tier {
            1 => {
                // Tier 1: Simple NPC (no RAG) - simulate streaming
                let agent = SimpleNpcAgent::new(persona);
                
                match agent.chat(&message).await {
                    Ok(response) => {
                        // Simulate token-by-token streaming
                        // Since rig-core doesn't support true streaming yet, we chunk the response
                        let words: Vec<&str> = response.split_whitespace().collect();
                        let chunk_size = 3; // Send 3 words at a time
                        
                        for chunk in words.chunks(chunk_size) {
                            let token = chunk.join(" ") + " ";
                            let _ = tx.send(Event::default()
                                .event("token")
                                .json_data(StreamToken { token })
                            ).await;
                            // Small delay to simulate streaming
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                        
                        // Send done event
                        let _ = tx.send(Event::default()
                            .event("done")
                            .json_data(StreamDone {
                                latency_ms: start.elapsed().as_millis() as u64,
                                confidence_score: None,
                                confidence_level: None,
                                sources: None,
                            })
                        ).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Event::default()
                            .event("error")
                            .json_data(serde_json::json!({"error": format!("Agent error: {}", e)}))
                        ).await;
                    }
                }
            }
            2 => {
                // Tier 2: Oracle RAG
                let agent = OracleRagAgent::new(persona, qdrant, Some(db));
                
                match agent.chat(&message).await {
                    Ok(response) => {
                        // Simulate token-by-token streaming
                        let words: Vec<&str> = response.content.split_whitespace().collect();
                        let chunk_size = 3;
                        
                        for chunk in words.chunks(chunk_size) {
                            let token = chunk.join(" ") + " ";
                            let _ = tx.send(Event::default()
                                .event("token")
                                .json_data(StreamToken { token })
                            ).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                        
                        // Send done event with metadata
                        let _ = tx.send(Event::default()
                            .event("done")
                            .json_data(StreamDone {
                                latency_ms: response.latency_ms,
                                confidence_score: Some(response.confidence_score),
                                confidence_level: Some(format!("{:?}", response.confidence_level)),
                                sources: Some(response.sources.iter().map(|s| {
                                    serde_json::json!({
                                        "source_type": s.source_type,
                                        "source_id": s.source_id,
                                        "relevance": s.relevance
                                    })
                                }).collect()),
                            })
                        ).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Event::default()
                            .event("error")
                            .json_data(serde_json::json!({"error": format!("Agent error: {}", e)}))
                        ).await;
                    }
                }
            }
            _ => {
                let _ = tx.send(Event::default()
                    .event("error")
                    .json_data(serde_json::json!({"error": "Invalid tier. Must be 1 or 2"}))
                ).await;
            }
        }
    });
    
    // Return SSE stream
    Sse::new(ReceiverStream::new(rx))
}
