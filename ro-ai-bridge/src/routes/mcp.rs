//! MCP (Model Context Protocol) SSE Transport — Sprint 31
//!
//! Implements SSE-based MCP server transport per the MCP specification:
//! - GET  /api/v1/mcp/sse     → SSE stream (sends endpoint event + messages)
//! - POST /api/v1/mcp/message → Receive JSON-RPC messages from client
//!
//! The SSE transport allows MCP clients (like Claude Desktop, Cursor, etc.)
//! to connect and query the Mimir knowledge base.

use axum::{
    http::{HeaderMap, StatusCode},
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use tracing::info;

// ── JSON-RPC Types ────────────────────────────────────

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC Error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

// ── MCP Protocol Constants ────────────────────────────

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const MCP_SERVER_NAME: &str = "mimir-knowledge-base";
pub const MCP_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC error codes
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// ── MCP Message Handler ──────────────────────────────

/// Handle incoming JSON-RPC messages for the MCP protocol.
pub fn handle_mcp_message(request: &JsonRpcRequest, _tenant_id: &str) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "tools/list" => handle_tools_list(request),
        "resources/list" => handle_resources_list(request),
        "prompts/list" => handle_prompts_list(request),
        _ => JsonRpcResponse::error(
            request.id.clone(),
            METHOD_NOT_FOUND,
            format!("Method not found: {}", request.method),
        ),
    }
}

fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {},
                "resources": { "subscribe": false },
                "prompts": {},
            },
            "serverInfo": {
                "name": MCP_SERVER_NAME,
                "version": MCP_SERVER_VERSION,
            },
        }),
    )
}

fn handle_tools_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "tools": [
                {
                    "name": "query_knowledge",
                    "description": "Query the Mimir knowledge base using hybrid RAG (vector + tree + graph retrieval)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The question to search for in the knowledge base"
                            },
                            "mode": {
                                "type": "string",
                                "enum": ["vector", "tree", "graph", "hybrid"],
                                "description": "Search mode (default: hybrid)",
                                "default": "hybrid"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Maximum number of results",
                                "default": 5
                            }
                        },
                        "required": ["question"]
                    }
                },
                {
                    "name": "list_documents",
                    "description": "List available documents in the knowledge base for the current tenant",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "page": { "type": "integer", "default": 1 },
                            "limit": { "type": "integer", "default": 20 }
                        }
                    }
                },
                {
                    "name": "get_graph_entities",
                    "description": "Search for entities in the knowledge graph",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Entity name or type to search for"
                            },
                            "limit": { "type": "integer", "default": 10 }
                        },
                        "required": ["query"]
                    }
                }
            ]
        }),
    )
}

fn handle_resources_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "resources": [
                {
                    "uri": "mimir://tenant/documents",
                    "name": "Tenant Documents",
                    "description": "All documents indexed for the current tenant",
                    "mimeType": "application/json"
                },
                {
                    "uri": "mimir://tenant/graph",
                    "name": "Knowledge Graph",
                    "description": "Entity and relationship data from the knowledge graph",
                    "mimeType": "application/json"
                }
            ]
        }),
    )
}

fn handle_prompts_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        request.id.clone(),
        json!({
            "prompts": [
                {
                    "name": "rag_search",
                    "description": "Search the knowledge base and format results for RAG",
                    "arguments": [
                        {
                            "name": "question",
                            "description": "The question to search for",
                            "required": true
                        }
                    ]
                }
            ]
        }),
    )
}

// ── Route Handlers ────────────────────────────────────

/// POST /api/v1/mcp/message — Receive JSON-RPC message
async fn mcp_message(
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let tenant_id = crate::routes::tenant::extract_tenant_id(&headers);
    info!(
        event = "mcp_message",
        method = %request.method,
        tenant_id = tenant_id,
        "MCP JSON-RPC request"
    );

    let response = handle_mcp_message(&request, tenant_id);
    Json(response)
}

/// GET /api/v1/mcp/sse — SSE stream for MCP transport
async fn mcp_sse(headers: HeaderMap) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let tenant_id = crate::routes::tenant::extract_tenant_id(&headers).to_string();
    info!(event = "mcp_sse_connect", tenant_id = %tenant_id, "MCP SSE client connected");

    let stream = async_stream::stream! {
        // First event: send the message endpoint URL
        yield Ok(Event::default()
            .event("endpoint")
            .data("/api/v1/mcp/message"));

        // Keep-alive ping every 30 seconds
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            yield Ok(Event::default()
                .event("ping")
                .data(format!("{{\"timestamp\":\"{}\"}}", chrono::Utc::now().to_rfc3339())));
        }
    };

    Sse::new(stream)
}

// ── Routes ────────────────────────────────────────────

pub fn mcp_routes() -> Router<DbPool> {
    Router::new()
        .route("/sse", get(mcp_sse))
        .route("/message", post(mcp_message))
        .layer(axum::middleware::from_fn(
            crate::routes::tenant::require_tenant_id,
        ))
}

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str, params: Option<Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn test_jsonrpc_success_response() {
        let resp = JsonRpcResponse::success(Some(json!(1)), json!({"ok": true}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_error_response() {
        let resp = JsonRpcResponse::error(Some(json!(2)), -32601, "Not found".to_string());
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_jsonrpc_response_serialization() {
        let resp = JsonRpcResponse::success(Some(json!(1)), json!({"data": "test"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_initialize() {
        let req = make_request("initialize", None);
        let resp = handle_mcp_message(&req, "test-tenant");
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], MCP_SERVER_NAME);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list() {
        let req = make_request("tools/list", None);
        let resp = handle_mcp_message(&req, "test-tenant");
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.len() >= 3);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"query_knowledge"));
        assert!(names.contains(&"list_documents"));
        assert!(names.contains(&"get_graph_entities"));
    }

    #[test]
    fn test_tools_have_input_schema() {
        let req = make_request("tools/list", None);
        let tools = handle_mcp_message(&req, "t").result.unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();
        for tool in &tools {
            assert!(
                tool["inputSchema"].is_object(),
                "Tool {} must have inputSchema",
                tool["name"]
            );
        }
    }

    #[test]
    fn test_query_knowledge_tool_schema() {
        let req = make_request("tools/list", None);
        let tools = handle_mcp_message(&req, "t").result.unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();
        let qt = tools
            .iter()
            .find(|t| t["name"] == "query_knowledge")
            .unwrap();
        let schema = &qt["inputSchema"];
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["question"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("question")));
    }

    #[test]
    fn test_resources_list() {
        let req = make_request("resources/list", None);
        let resp = handle_mcp_message(&req, "test-tenant");
        let resources = resp.result.unwrap()["resources"]
            .as_array()
            .unwrap()
            .clone();
        assert!(resources.len() >= 2);
        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(uris.contains(&"mimir://tenant/documents"));
        assert!(uris.contains(&"mimir://tenant/graph"));
    }

    #[test]
    fn test_prompts_list() {
        let req = make_request("prompts/list", None);
        let resp = handle_mcp_message(&req, "test-tenant");
        let prompts = resp.result.unwrap()["prompts"].as_array().unwrap().clone();
        assert!(!prompts.is_empty());
        assert_eq!(prompts[0]["name"], "rag_search");
    }

    #[test]
    fn test_unknown_method_returns_error() {
        let req = make_request("nonexistent/method", None);
        let resp = handle_mcp_message(&req, "test-tenant");
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn test_mcp_routes_assembly_with_middleware() {
        // Verify routes + tenant auth middleware assemble without panic
        let _router = mcp_routes();
    }

    #[test]
    fn test_protocol_version() {
        assert_eq!(MCP_PROTOCOL_VERSION, "2024-11-05");
    }

    #[test]
    fn test_server_name() {
        assert_eq!(MCP_SERVER_NAME, "mimir-knowledge-base");
    }

    // --- Tenant auth middleware tests (unit) ---

    #[test]
    fn test_extract_tenant_id_with_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Tenant-Id", "my-tenant".parse().unwrap());
        let tid = crate::routes::tenant::extract_tenant_id(&headers);
        assert_eq!(tid, "my-tenant");
    }

    #[test]
    fn test_extract_tenant_id_missing_header_gets_default() {
        let headers = HeaderMap::new();
        let tid = crate::routes::tenant::extract_tenant_id(&headers);
        assert_eq!(tid, "default_tenant");
    }

    #[test]
    fn test_extract_tenant_id_empty_header_gets_default() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Tenant-Id", "".parse().unwrap());
        let tid = crate::routes::tenant::extract_tenant_id(&headers);
        assert_eq!(tid, "default_tenant");
    }
}
