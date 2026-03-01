use axum::{
    routing::{get, post},
    Router, Json,
};
use sqlx::MySqlPool;
use serde_json::{json, Value};
use mimir_core_ai::services::mcp_server::{
    self, McpToolCallRequest,
};

/// List available MCP tools.
async fn list_tools() -> Json<Value> {
    let tools = mcp_server::list_tools();
    Json(json!({
        "tools": tools
    }))
}

/// Get MCP server info (initialize response).
async fn server_info() -> Json<Value> {
    let info = mcp_server::server_info();
    Json(json!(info))
}

/// Call an MCP tool by name.
async fn call_tool(Json(request): Json<McpToolCallRequest>) -> Json<Value> {
    let result = mcp_server::dispatch_tool_call(&request);
    Json(json!(result))
}

pub fn mcp_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/tools", get(list_tools))
        .route("/info", get(server_info))
        .route("/tools/call", post(call_tool))
}
