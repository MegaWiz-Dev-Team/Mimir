use axum::{routing::get, Json, Router};
use mimir_core_ai::services::db::DbPool;
use serde_json::{json, Value};

pub fn features_routes() -> Router<DbPool> {
    Router::new().route("/feature-flags", get(get_feature_flags))
}

async fn get_feature_flags() -> Json<Value> {
    // Return sensible defaults for feature flags.
    // In the future, this could be fetched from the database or dynamic environment variables.
    Json(json!({
        "ocr_enabled": true,
        "dicom_enabled": false,
        "domain": "general"
    }))
}
