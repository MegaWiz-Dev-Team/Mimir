use axum::{
    routing::get,
    Router,
    Json,
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

use ro_ai_bridge::config::Config;
use ro_ai_bridge::routes;
// Services are likely used within routes or initialized here if needed
// use ro_ai_bridge::services;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = Config::from_env();

    // build our application with a route
    let app = Router::new()
        .route("/health", get(health_check));

    // run it
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("listening on {}", addr);
    
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
