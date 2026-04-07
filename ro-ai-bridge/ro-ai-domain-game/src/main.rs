use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tracing::info;

use mimir_core_ai::config::Config;
use mimir_core_ai::services::db;

use ro_ai_domain_game::api::rathena_gateway::rathena_routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Config::from_env();

    let pool = db::init_db().await.expect("Failed to initialize database");
    info!("✅ Database connected and migrations applied");

    // build our application with routes
    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/rathena", rathena_routes())
        .with_state(pool);

    // run it
    let addr = format!("0.0.0.0:{}", config.port);
    info!("🚀 listening on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "ro-ai-domain-game",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
