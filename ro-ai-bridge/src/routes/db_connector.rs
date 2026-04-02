//! External DB Connector Routes (Issue #152)
//!
//! - POST /api/v1/db-connector/test-connection   — test external DB connection
//! - POST /api/v1/db-connector/discover-schema   — list tables + columns
//! - POST /api/v1/db-connector/import             — import data via query → markdown chunks

use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use mimir_core_ai::services::db_connector::{
    self, DiscoverSchemaRequest, ImportRequest, TestConnectionRequest,
};
use serde_json::json;
use sqlx::MySqlPool;
use tracing::{error, info};

pub fn db_connector_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/test-connection", post(test_connection))
        .route("/discover-schema", post(discover_schema))
        .route("/import", post(import_data))
}

/// POST /db-connector/test-connection — test an external DB connection
async fn test_connection(
    headers: HeaderMap,
    State(pool): State<MySqlPool>,
    Json(req): Json<TestConnectionRequest>,
) -> impl IntoResponse {
    // Validate config
    if let Err(e) = db_connector::validate_connection_config(&req) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Invalid config: {}", e)
            })),
        )
            .into_response();
    }

    // Test connection
    match db_connector::test_connection(&req.connection_string, &req.db_type).await {
        Ok(version) => {
            let tenant_id = extract_tenant_id(&headers);
            // Save connection with success status
            let conn_id = db_connector::save_connection(&pool, tenant_id, &req, "connected").await;

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "version": version,
                    "db_type": req.db_type.to_string(),
                    "connection_id": conn_id.unwrap_or(0)
                })),
            )
                .into_response()
        }
        Err(e) => {
            let tenant_id = extract_tenant_id(&headers);
            let _ = db_connector::save_connection(&pool, tenant_id, &req, "failed").await;

            error!(error = %e, "DB connection test failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "success": false,
                    "error": format!("Connection failed: {}", e)
                })),
            )
                .into_response()
        }
    }
}

/// POST /db-connector/discover-schema — list tables and columns
async fn discover_schema(Json(req): Json<DiscoverSchemaRequest>) -> impl IntoResponse {
    match db_connector::discover_schema(&req.connection_string, &req.db_type).await {
        Ok(schemas) => {
            info!(tables = schemas.len(), "Schema discovery complete");
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "tables": schemas,
                    "count": schemas.len()
                })),
            )
                .into_response()
        }
        Err(e) => {
            error!(error = %e, "Schema discovery failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": format!("Schema discovery failed: {}", e)
                })),
            )
                .into_response()
        }
    }
}

/// POST /db-connector/import — execute query and return data as markdown
async fn import_data(Json(req): Json<ImportRequest>) -> impl IntoResponse {
    // Validate query first (sandboxing)
    if let Err(e) = db_connector::validate_query(&req.query) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Query rejected: {}", e)
            })),
        )
            .into_response();
    }

    match db_connector::execute_import_query(&req.connection_string, &req.db_type, &req.query).await
    {
        Ok(result) => {
            info!(
                rows = result.rows_imported,
                columns = result.columns.len(),
                chars = result.total_chars,
                source = %req.source_name,
                "Data import complete"
            );
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "rows_imported": result.rows_imported,
                    "columns": result.columns,
                    "markdown_preview": result.markdown_preview,
                    "total_chars": result.total_chars
                })),
            )
                .into_response()
        }
        Err(e) => {
            error!(error = %e, "Data import failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": format!("Import failed: {}", e)
                })),
            )
                .into_response()
        }
    }
}
