use anyhow::Result;
use axum::{
    routing::{get, post},
    Router, Json, extract::{State, Path, Extension},
    response::{IntoResponse, sse::Event, Sse},
    http::StatusCode,
};
use dotenvy::dotenv;
use mimir_core_ai::middleware::tenant::{tenant_auth_middleware, TenantContext};
use mimir_core_ai::services::db::{init_db, DbPool};
use mimir_core_ai::qa_qc::pipeline::{run_pipeline_with_config, resume_pipeline_with_config};
use mimir_core_ai::config::QAConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{info, error};
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;
use futures::stream::Stream;

use mimir_core_ai::services::qdrant::QdrantService;
use mimir_core_ai::qa_qc::indexer::run_indexer;
use mimir_core_ai::rag_engine::{OracleRagAgent, LlmProvider};
use mimir_core_ai::models::persona::Persona;
use mimir_core_ai::services::iam::IamService;
use ro_ai_domain_game::simple_npc::SimpleNpcAgent;
use rig::providers::ollama;
use mimir_core_ai::qa_qc::clustering::{ClusteringService, ResolveClusterRequest};
use axum::extract::Query;

#[derive(Deserialize)]
struct RunRequest {
    provider: Option<String>,
    model: Option<String>,
    test_run: Option<bool>,
    tenant_id: Option<String>,
}

#[derive(Serialize)]
struct RunResponse {
    run_id: String,
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    limit: Option<usize>,
    tenant_id: Option<String>,
    show_expired: Option<bool>,
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
    /// Provider: "ollama" (local) or "gemini" (cloud)
    provider: Option<String>,
    /// Model name (e.g., "llama3.2", "gemini-2.5-flash")
    model: Option<String>,
    /// Tenant ID (e.g., "default_tenant")
    tenant_id: Option<String>,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    access_token: String,
    tenant_id: String,
}

/// Chat response for non-streaming responses
#[derive(Serialize)]
struct ChatResponse {
    content: String,
    tier: i8,
    persona: String,
    latency_ms: u64,
    /// Provider used for this response
    provider: String,
    /// Model used for this response
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools_used: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<serde_json::Value>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<serde_json::Value>,
}

struct AppState {
    db: DbPool,
    qdrant: QdrantService,
    iam: IamService,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let pool = init_db().await?;
    let qdrant = QdrantService::new();
    let iam = IamService::new(pool.clone());
    let state = Arc::new(AppState { 
        db: pool.clone(), // Clone pool for AppState
        qdrant,
        iam,
    });

    let auth_routes = Router::new()
        
        // Quality Control
        .route("/api/v1/qc/clusters", get(get_qc_clusters))
        .route("/api/v1/qc/resolve/{id}", post(resolve_qc_cluster))
        .route("/api/v1/qc/generate", post(generate_qc_clusters))
        
        // Pipeline endpoints
        .route("/api/v1/pipeline/run", post(trigger_run))
        .route("/api/v1/pipeline/runs", get(list_runs))
        .route("/api/v1/pipeline/runs/{id}", get(get_run_details))
        .route("/api/v1/pipeline/steps/{id}/qa", get(get_step_qa))
        .route("/api/v1/pipeline/steps/{id}/report", get(get_step_report))
        .route("/api/v1/pipeline/steps/{id}/retry", post(retry_step_handler))
        .route("/api/v1/pipeline/steps/{id}/generate_missing", post(generate_missing_qa_handler))
        .route("/api/v1/pipeline/runs/{id}/resume", post(resume_run_handler))
        .route("/api/v1/vector/stats", get(get_vector_stats))
        .route("/api/v1/vector/index", post(trigger_indexing))
        .route("/api/v1/vector/search", post(search_vectors))
        .route("/api/v1/vector/{id}", axum::routing::delete(delete_vector_handler))
        // Agent chat endpoints
        .route("/api/v1/agents/chat", post(chat_handler))
        .route("/api/v1/agents/chat/stream", post(chat_stream_handler))
        // Model config endpoints
        .route("/api/v1/models", get(models_handler))
        .route("/api/v1/personas/{name}/config", post(update_persona_config_handler))
        // Wiki content endpoint
        .route("/api/v1/wiki/{filename}", get(get_wiki_content))
        .layer(axum::middleware::from_fn(tenant_auth_middleware));

    let iam_router = ro_ai_bridge::routes::iam::iam_routes().with_state(pool);

    let app = Router::new()
        // Auth (Public)
        .route("/api/v1/auth/login", post(auth_login))
        .nest("/api/v1/iam", iam_router)
        .merge(auth_routes)
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

async fn auth_login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    match state.iam.login(&payload.username, &payload.password).await {
        Ok((access_token, tenant_id)) => {
            (StatusCode::OK, Json(LoginResponse { access_token, tenant_id })).into_response()
        },
        Err(e) => {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn trigger_run(
    State(state): State<Arc<AppState>>,
    tenant_ctx: Option<Extension<TenantContext>>,
    Json(payload): Json<RunRequest>,
) -> Json<RunResponse> {
    let tenant_id = tenant_ctx.map(|ctx| ctx.tenant_id.clone())
        .unwrap_or_else(|| payload.tenant_id.unwrap_or_else(|| "default_tenant".to_string()));
        
    let tenant_config = state.iam.get_tenant_config(&tenant_id).await.ok();
    
    let default_provider_str = tenant_config.as_ref()
        .map(|c| c.default_provider.clone())
        .unwrap_or_else(|| "ollama".to_string());
    let default_model_str = tenant_config.as_ref()
        .map(|c| c.default_model.clone());

    let provider = payload.provider.unwrap_or(default_provider_str);
    let model = payload.model.or(default_model_str).unwrap_or_else(|| {
        if provider == "gemini" { "gemini-2.5-flash".to_string() } else { "llama3.2".to_string() }
    });
    
    let is_test = payload.test_run.unwrap_or(false);
    
    // Load QA config from file (uses defaults if file not found)
    let config_path = std::env::var("QA_CONFIG_PATH").unwrap_or_else(|_| "data/qa_config.json".to_string());
    let qa_config = QAConfig::from_file_or_default(&config_path);
    info!("📋 QA Config: default_count={}, {} size rules, {} file patterns", 
        qa_config.default_count, qa_config.rules.len(), qa_config.file_patterns.patterns.len());

    let db = state.db.clone();
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_id_inner = run_id.clone();
    
    tokio::spawn(async move {
        // Pass qa_config to pipeline - it will calculate count per chunk
        if let Err(e) = run_pipeline_with_config(&db, run_id_inner.clone(), &provider, &model, "data/wiki", is_test, qa_config, tenant_id).await {
            tracing::error!("Background pipeline failed: {}", e);
            let _ = sqlx::query("UPDATE pipeline_runs SET status = 'FAILED' WHERE id = ?")
                .bind(run_id_inner)
                .execute(&db).await;
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
        let steps = sqlx::query(
            "SELECT ps.*, \
             (SELECT COUNT(*) FROM qa_results WHERE step_id = ps.id) as qa_count, \
             (SELECT coverage_score FROM evaluation_reports WHERE step_id = ps.id) as coverage_score \
             FROM pipeline_steps ps \
             WHERE ps.run_id = ?"
        )
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
                "qa_count": s.get::<Option<i64>, _>("qa_count").unwrap_or(0),
                "coverage_score": s.get::<Option<f32>, _>("coverage_score"),
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

    // Load QA config
    let config_path = std::env::var("QA_CONFIG_PATH").unwrap_or_else(|_| "data/qa_config.json".to_string());
    let qa_config = QAConfig::from_file_or_default(&config_path);
    
    tokio::spawn(async move {
        if let Err(e) = mimir_core_ai::qa_qc::pipeline::retry_step_with_config(&db, id, qa_config).await {
            tracing::error!("Background retry Step #{} failed: {}", id, e);
            let _ = sqlx::query("UPDATE pipeline_steps SET status = 'FAILED', error_message = ? WHERE id = ?")
                .bind(e.to_string())
                .bind(id)
                .execute(&db).await;
        }
    });

    StatusCode::ACCEPTED
}

async fn generate_missing_qa_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let db = state.db.clone();
    
    // Check if step exists
    let step = sqlx::query("SELECT id FROM pipeline_steps WHERE id = ?")
        .bind(id as i64)
        .fetch_optional(&db)
        .await
        .unwrap_or_default();

    if step.is_none() {
         return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Step not found"}))).into_response();
    }

    // Load QA config
    let config_path = std::env::var("QA_CONFIG_PATH").unwrap_or_else(|_| "data/qa_config.json".to_string());
    let qa_config = QAConfig::from_file_or_default(&config_path);
    
    // Spawn background task
    let step_id = id;
    tokio::spawn(async move {
        if let Err(e) = mimir_core_ai::qa_qc::pipeline::generate_missing_qa_for_step(&db, step_id, qa_config).await {
            tracing::error!("Missing QA generation for Step #{} failed: {}", step_id, e);
        }
    });

    StatusCode::ACCEPTED.into_response()
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

    // Load QA config
    let config_path = std::env::var("QA_CONFIG_PATH").unwrap_or_else(|_| "data/qa_config.json".to_string());
    let qa_config = QAConfig::from_file_or_default(&config_path);
    
    // Run resume in background
    let run_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = resume_pipeline_with_config(&db, run_id.clone(), qa_config).await {
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
    Extension(tenant): Extension<TenantContext>,
    Json(payload): Json<SearchRequest>,
) -> impl IntoResponse {
    use rig::embeddings::EmbeddingModel;
    
    let ollama_client = ollama::Client::new();
    let embed_model = ollama_client.embedding_model("nomic-embed-text");
    
    let target_tenant = if tenant.role == "SuperAdmin" {
        payload.tenant_id.unwrap_or(tenant.tenant_id)
    } else {
        tenant.tenant_id
    };

    let show_expired = payload.show_expired.unwrap_or(false);

    match embed_model.embed_text(&payload.query).await {
        Ok(embedding) => {
            let vector_f32: Vec<f32> = embedding.vec.into_iter().map(|f| f as f32).collect();
            match state.qdrant.search("wiki_qa", vector_f32, payload.limit.unwrap_or(5), &target_tenant, show_expired).await {
                Ok(results) => Json(results).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
            }
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    }
}

async fn delete_vector_handler(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<TenantContext>,
    axum::extract::Path(id): axum::extract::Path<u64>,
) -> impl IntoResponse {
    if tenant.role != "SuperAdmin" && tenant.role != "admin" {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Unauthorized to delete vectors"}))).into_response();
    }
    
    match state.qdrant.delete_point("wiki_qa", id).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"status": "success"}))).into_response(),
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
    
    let tenant_id = payload.tenant_id.clone().unwrap_or_else(|| "default_tenant".to_string());
    let tenant_config = state.iam.get_tenant_config(&tenant_id).await.ok();
    
    let default_provider_str = tenant_config.as_ref()
        .map(|c| c.default_provider.clone())
        .unwrap_or_else(|| "ollama".to_string());
    let default_model_str = tenant_config.as_ref()
        .map(|c| c.default_model.clone());

    let provider = payload.provider.as_deref()
        .unwrap_or(&default_provider_str)
        .parse::<LlmProvider>()
        .unwrap_or(LlmProvider::Ollama);
    
    let model = payload.model.clone()
        .or(default_model_str)
        .unwrap_or_else(|| {
            match provider {
                LlmProvider::Ollama => "llama3.2".to_string(),
                LlmProvider::Gemini => "gemini-2.5-flash".to_string(),
            }
        });

    info!("💬 Received non-streaming chat request: tier={}, persona={}, provider={}, model={}, message={}, tenant_id={:?}", 
          payload.tier, payload.persona, provider, model, payload.message, payload.tenant_id);
    
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
            let agent = SimpleNpcAgent::with_model(persona, &model);
            
            match agent.chat(&payload.message).await {
                Ok(response) => {
                    let action = agent.action_capture.lock().await.clone(); // Capture action
                    
                    let chat_response = ChatResponse {
                        content: response,
                        tier: 1,
                        persona: payload.persona,
                        latency_ms: start.elapsed().as_millis() as u64,
                        provider: provider.to_string(),
                        model: model.clone(),
                        confidence_score: None,
                        confidence_level: None,
                        sources: None,
                        tools_used: None,
                        action,
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
            // Tier 2: Oracle RAG with provider support
            let mut plugins: Vec<Box<dyn mimir_core_ai::rag_engine::DynamicContextPlugin>> = vec![];
            plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryMobDbTool::new(state.db.clone())));
            plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryItemDbTool::new(state.db.clone())));

            let agent = OracleRagAgent::with_provider(
                persona,
                state.qdrant.clone(),
                plugins,
                provider.clone(),
                Some(&model),
                None,
                tenant_id.clone(),
            );
            
            match agent.chat(&payload.message).await {
                Ok(response) => {
                    let chat_response = ChatResponse {
                        content: response.content,
                        tier: 2,
                        persona: payload.persona,
                        latency_ms: response.latency_ms,
                        provider: provider.to_string(),
                        model: model.clone(),
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
                        action: None,
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
    let tenant_id = payload.tenant_id.clone().unwrap_or_else(|| "default_tenant".to_string());
    let tenant_config = state.iam.get_tenant_config(&tenant_id).await.ok();
    
    let default_provider_str = tenant_config.as_ref()
        .map(|c| c.default_provider.clone())
        .unwrap_or_else(|| "ollama".to_string());
    let default_model_str = tenant_config.as_ref()
        .map(|c| c.default_model.clone());

    let provider = payload.provider.as_deref()
        .unwrap_or(&default_provider_str)
        .parse::<LlmProvider>()
        .unwrap_or(LlmProvider::Ollama);
    
    let model = payload.model.clone()
        .or(default_model_str)
        .unwrap_or_else(|| {
            match provider {
                LlmProvider::Ollama => "llama3.2".to_string(),
                LlmProvider::Gemini => "gemini-2.5-flash".to_string(),
            }
        });
    
    info!("💬 Received streaming chat request: tier={}, persona={}, provider={}, model={}, message={}, tenant_id={:?}", 
          payload.tier, payload.persona, provider, model, payload.message, payload.tenant_id);
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
                let agent = SimpleNpcAgent::with_model(persona, &model);
                
                match agent.chat(&message).await {
                    Ok(response) => {
                        let action = agent.action_capture.lock().await.clone(); // Capture action
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
                                action,
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
                // Tier 2: Oracle RAG with provider support
                let mut plugins: Vec<Box<dyn mimir_core_ai::rag_engine::DynamicContextPlugin>> = vec![];
                plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryMobDbTool::new(db.clone())));
                plugins.push(Box::new(ro_ai_domain_game::tools::rag_tools::QueryItemDbTool::new(db.clone())));

                let agent = OracleRagAgent::with_provider(
                    persona, 
                    qdrant, 
                    plugins,
                    provider.clone(),
                    Some(&model),
                    None,
                    tenant_id.clone(),
                );
                
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
                                action: None,
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

// ─── Model Config API ────────────────────────────────────────────────────────

/// Get available LLM models from the database
async fn models_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match mimir_core_ai::services::db::get_active_llm_models(&state.db).await {
        Ok(models) => Json(models).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to fetch models: {}", e)}))
        ).into_response()
    }
}

// ─── Wiki Content API ────────────────────────────────────────────────────────

/// Get wiki content by filename
async fn get_wiki_content(
    Path(filename): Path<String>,
) -> impl IntoResponse {
    // Safety check: only allow alphanum, dots, underscores and hyphens
    if !filename.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-') {
        return (StatusCode::BAD_REQUEST, "Invalid filename").into_response();
    }

    // Safety check: only .md files
    if !filename.ends_with(".md") {
        return (StatusCode::BAD_REQUEST, "Only .md files allowed").into_response();
    }

    let path = std::path::Path::new("data/wiki").join(filename);
    
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content.into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Wiki file not found").into_response(),
    }
}

// ─── Phase 7: Data Quality Control APIs ────────────────────────────────────────

#[derive(Deserialize)]
struct GetClustersQuery {
    status: Option<String>,
}

async fn get_qc_clusters(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<TenantContext>,
    Query(params): Query<GetClustersQuery>,
) -> impl IntoResponse {
    match ClusteringService::get_clusters(&state.db, &tenant.tenant_id, params.status.as_deref()).await {
        Ok(clusters) => (StatusCode::OK, Json(serde_json::json!({ "clusters": clusters }))).into_response(),
        Err(e) => {
            error!("Failed to get QC clusters: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn resolve_qc_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(_tenant): Extension<TenantContext>, // Could add tenant check
    Json(payload): Json<ResolveClusterRequest>,
) -> impl IntoResponse {
    match ClusteringService::resolve_cluster(&state.db, &id, payload).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            error!("Failed to resolve QC cluster {}: {}", id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn generate_qc_clusters(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<TenantContext>,
) -> impl IntoResponse {
    // Return early to not block the request
    let db = state.db.clone();
    let tenant_id = tenant.tenant_id.clone();
    tokio::spawn(async move {
        if let Err(e) = ClusteringService::trigger_clustering(&db, &tenant_id).await {
            error!("Background QC Clustering failed: {}", e);
        }
    });
    
    (StatusCode::ACCEPTED, Json(serde_json::json!({"status": "Generation started in background"}))).into_response()
}

#[derive(Deserialize)]
pub struct UpdatePersonaConfigRequest {
    model_id: String,
}

async fn update_persona_config_handler(
    Path(name): Path<String>,
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<UpdatePersonaConfigRequest>,
) -> impl IntoResponse {
    let mut persona = match Persona::load_by_name_cached(&name) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Persona '{}' not found: {}", name, e)})),
            ).into_response();
        }
    };

    // Update the model_id
    persona.model_id = Some(payload.model_id.clone());

    // Save back to yaml
    if let Err(e) = persona.save() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save persona: {}", e)})),
        ).into_response();
    }

    info!("Updated persona '{}' config to use model_id: {}", name, payload.model_id);

    (StatusCode::OK, Json(serde_json::json!({"success": true, "persona": persona}))).into_response()
}
