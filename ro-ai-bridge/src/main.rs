use axum::{
    routing::get,
    Router,
    Extension,
    Json,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use tower_http::cors::{CorsLayer, Any};

use ro_ai_bridge::config::Config;
use mimir_core_ai::services::db;
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

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = Config::from_env();
    let config = Arc::new(config);

    // Initialize database
    let pool = db::init_db().await.expect("Failed to initialize database");
    info!("✅ Database connected and migrations applied");

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
        .with_state(pool)
        .layer(Extension(config.clone()))
        .layer(cors);

    // run it
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🚀 listening on {}", addr);
    
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
