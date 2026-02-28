use axum::{
    routing::get,
    Router,
    Extension,
    Json,
    middleware,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{self, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::cors::{CorsLayer, Any};

use ro_ai_bridge::config::Config;
use mimir_core_ai::services::db;
use mimir_core_ai::middleware::request_id::request_id_middleware;
use mimir_core_ai::services::cron;
use ro_ai_bridge::routes::eval::eval_routes;
use ro_ai_bridge::routes::iam::iam_routes;
use ro_ai_bridge::routes::auth::auth_routes;
use ro_ai_bridge::routes::pipeline::pipeline_routes;
use ro_ai_bridge::routes::qc::qc_routes;
use ro_ai_bridge::routes::vector::vector_routes;
use ro_ai_bridge::routes::llm_usage::llm_usage_routes;
use ro_ai_bridge::routes::agents::agents_routes;
use ro_ai_bridge::routes::conversations::conversations_routes;
use ro_ai_bridge::routes::evaluations_ext::evaluations_ext_routes;
use ro_ai_bridge::routes::budget::{budget_settings_routes, budget_usage_routes};
use ro_ai_bridge::routes::cron::{cron_routes, cron_status_routes};
use ro_ai_bridge::routes::feedback::feedback_routes;
use ro_ai_bridge::routes::ocr::ocr_routes;

#[tokio::main]
async fn main() {
    // Initialize structured JSON logging with env-filter support
    // Usage: RUST_LOG=info (default), RUST_LOG=debug, RUST_LOG=ro_ai_bridge=debug,mimir_core_ai=info
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
                .with_current_span(true)
        )
        .init();
    
    // Load configuration
    let config = Config::from_env();
    let config = Arc::new(config);

    // Initialize database
    let pool = db::init_db().await.expect("Failed to initialize database");
    info!(event = "db_connected", "✅ Database connected and migrations applied");

    // Start cron worker for scheduled re-sync (Issue #150)
    let cron_tick_seconds: u64 = std::env::var("CRON_TICK_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let cron_state = cron::start_cron_worker(pool.clone(), cron_tick_seconds);

    // CORS layer for dashboard
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // build our application with routes
    let app = Router::new()
        .route("/health", get(health_check))
        .merge(eval_routes())
        .nest("/api/v1/iam", iam_routes())
        .nest("/api/v1/auth", auth_routes())
        .nest("/api/v1/pipeline", pipeline_routes())
        .nest("/api/v1/qc", qc_routes())
        .nest("/api/v1/vector", vector_routes())
        .nest("/api/v1/sources", ro_ai_bridge::routes::sources::sources_routes())
        .nest("/api/v1/llm-usage", llm_usage_routes())
        .nest("/api/v1/agents", agents_routes())
        .nest("/api/v1/conversations", conversations_routes())
        .nest("/api/v1/evaluations", evaluations_ext_routes())
        .nest("/api/v1/settings", budget_settings_routes())
        .merge(budget_usage_routes())
        // Sprint 14: Cron schedule, feedback & OCR routes
        .nest("/api/v1", cron_routes())
        .nest("/api/v1", cron_status_routes())
        .nest("/api/v1/feedback", feedback_routes())
        .nest("/api/v1", ocr_routes())
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(pool)
        .layer(Extension(config.clone()))
        .layer(Extension(cron_state))
        .layer(cors);

    // run it
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!(event = "server_starting", address = %addr, "🚀 listening on {}", addr);
    
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}


async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "ro-ai-bridge",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
