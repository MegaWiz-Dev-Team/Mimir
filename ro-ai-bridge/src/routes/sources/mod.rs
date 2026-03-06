//! Data Sources — CRUD, sync, upload, connectors, and LLM configuration
//!
//! This module was refactored from a single 1500+ line file into sub-modules
//! for maintainability. All public APIs remain unchanged.

mod crud;
mod sync;
mod upload;
mod connectors;
mod config;

// Re-export public items to preserve existing import paths
pub use config::{resolve_llm_credentials, infer_api_base, call_llm_api, call_llm_api_with_logging};
pub use upload::download_from_s3_public;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use mimir_core_ai::services::db::DbPool;

pub fn sources_routes() -> Router<DbPool> {
    Router::new()
        .route("/", get(crud::list_sources).post(crud::create_source))
        .route("/upload", post(upload::upload_file))
        .route("/preview", get(connectors::preview_url))
        .route("/{id}", put(crud::update_source).delete(crud::delete_source))
        .route("/{id}/sync", post(sync::sync_source))
        .route("/{id}/extract-ai", post(config::extract_with_ai))
        .route("/{id}/logs", get(sync::stream_logs))
        .route("/{id}/discover-hierarchy", post(connectors::discover_hierarchy))
        .route("/{id}/import-pages", post(connectors::import_pages))
}
