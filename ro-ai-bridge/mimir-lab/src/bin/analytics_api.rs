//! analytics-api — HTTP backend behind Hermodr's analytics MCP tools.
//!
//! Env:
//!   BIND               (default 0.0.0.0:8091)
//!   ANALYTICS_DATA_DIR (default /data/analytics) — dir of <table>.parquet
//!   MARIADB_URL        (optional) — enables dataset list/profile via registry
//!   TYR_AUDIT_URL/TOKEN(optional) — forward audit to Tyr (else tracing)

use mimir_lab::audit;
use mimir_lab::registry::Registry;
use mimir_lab::server::{router, AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8091".into());
    let data_dir = std::env::var("ANALYTICS_DATA_DIR").unwrap_or_else(|_| "/data/analytics".into());
    let audit = audit::sink_from_env();

    let registry = match std::env::var("MARIADB_URL") {
        Ok(url) if !url.is_empty() => match Registry::connect(&url).await {
            Ok(r) => {
                tracing::info!("registry connected");
                Some(Arc::new(r))
            }
            Err(e) => {
                tracing::warn!(error = %e, "registry connect failed — dataset list/profile disabled");
                None
            }
        },
        _ => {
            tracing::info!("MARIADB_URL unset — dataset list/profile disabled");
            None
        }
    };

    let state = AppState {
        data_dir: Arc::new(data_dir.clone()),
        audit,
        registry,
    };

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("bind analytics-api");
    tracing::info!(%bind, %data_dir, "analytics-api listening");
    axum::serve(listener, router(state))
        .await
        .expect("serve analytics-api");
}
