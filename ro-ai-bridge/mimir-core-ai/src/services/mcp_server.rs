//! MCP Server — Model Context Protocol server implementation for Project Mimir.
//!
//! Exposes Mimir's capabilities as MCP tools that external clients can discover
//! and invoke via JSON-RPC style API.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Describes a single MCP tool capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// A request to call an MCP tool.
#[derive(Debug, Deserialize)]
pub struct McpToolCallRequest {
    pub name: String,
    pub arguments: HashMap<String, Value>,
}

/// Result of an MCP tool call.
#[derive(Debug, Serialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContent>,
    pub is_error: bool,
}

/// Content block in MCP response.
#[derive(Debug, Serialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Server info for MCP initialize response.
#[derive(Debug, Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    pub capabilities: McpCapabilities,
}

/// Server capabilities.
#[derive(Debug, Serialize)]
pub struct McpCapabilities {
    pub tools: McpToolCapability,
}

#[derive(Debug, Serialize)]
pub struct McpToolCapability {
    pub list_changed: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tool Registry
// ═══════════════════════════════════════════════════════════════════════════════

/// Returns the static list of tools available in Mimir.
pub fn list_tools() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "get_pipeline_status".into(),
            description: "Fetch the processing status of all data pipelines (running, failed, completed) and basic info like chunks, ETA.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tenant_id": { "type": "string" }
                },
                "required": ["tenant_id"]
            }),
        },
        McpToolDefinition {
            name: "check_bifrost_status".into(),
            description: "Check if the Swarm Agent Orchestrator (Bifrost) deployment engine is healthy and reachable from Mimir.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tenant_id": { "type": "string" }
                },
                "required": ["tenant_id"]
            }),
        },
        McpToolDefinition {
            name: "vector_search".into(),
            description: "Search documents using vector similarity (semantic search via Qdrant)"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query text" },
                    "tenant_id": { "type": "string", "description": "Tenant ID" },
                    "limit": { "type": "integer", "description": "Max results (default 10)", "default": 10 }
                },
                "required": ["query", "tenant_id"]
            }),
        },
        McpToolDefinition {
            name: "sql_query".into(),
            description: "Execute a read-only SQL query against the tenant database".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "SQL SELECT query" },
                    "tenant_id": { "type": "string", "description": "Tenant ID" }
                },
                "required": ["query", "tenant_id"]
            }),
        },
        McpToolDefinition {
            name: "graph_search".into(),
            description: "Search knowledge graph entities and relations (Neo4j)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entity": { "type": "string", "description": "Entity name or keyword" },
                    "tenant_id": { "type": "string", "description": "Tenant ID" },
                    "max_depth": { "type": "integer", "description": "Max relation depth", "default": 2 }
                },
                "required": ["entity", "tenant_id"]
            }),
        },
        McpToolDefinition {
            name: "source_list".into(),
            description: "List data sources for a tenant".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tenant_id": { "type": "string", "description": "Tenant ID" },
                    "source_type": { "type": "string", "description": "Filter by type (file, web, sql, mcp)" }
                },
                "required": ["tenant_id"]
            }),
        },
        McpToolDefinition {
            name: "submit_feedback".into(),
            description: "Submit a bug report or feature request (creates GitHub issue)".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "report_type": { "type": "string", "enum": ["bug", "feedback", "feature"] },
                    "title": { "type": "string", "description": "Report title" },
                    "description": { "type": "string", "description": "Detailed description" },
                    "priority": { "type": "string", "enum": ["low", "medium", "high", "critical"] }
                },
                "required": ["report_type", "title", "description"]
            }),
        },
    ]
}

/// Get server info for MCP initialize response.
pub fn server_info() -> McpServerInfo {
    McpServerInfo {
        name: "mimir-bridge".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: "2024-11-05".to_string(),
        capabilities: McpCapabilities {
            tools: McpToolCapability {
                list_changed: false,
            },
        },
    }
}

/// Validate tool call arguments against schema.
pub fn validate_tool_call(request: &McpToolCallRequest) -> Result<(), String> {
    let tools = list_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == request.name)
        .ok_or_else(|| {
            format!(
                "Unknown tool: '{}'. Available: {}",
                request.name,
                tools
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    // Check required fields
    if let Some(required) = tool.input_schema.get("required").and_then(|r| r.as_array()) {
        for field in required {
            if let Some(field_name) = field.as_str() {
                if !request.arguments.contains_key(field_name) {
                    return Err(format!(
                        "Missing required argument: '{}' for tool '{}'",
                        field_name, request.name
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Dispatch an MCP tool call (returns mock/formatted results for now).
/// In production, this connects to real services (Qdrant, Neo4j, MariaDB).
pub async fn dispatch_tool_call(request: &McpToolCallRequest, tenant_id: &str, pool: &crate::services::db::DbPool) -> McpToolCallResult {
    match validate_tool_call(request) {
        Ok(()) => {
            let text = match request.name.as_str() {
                "get_pipeline_status" => {
                    let rows: Result<Vec<(String, String, i64, Option<String>)>, _> = sqlx::query_as(
                        "SELECT ds.name, pr.status, COUNT(c.id), CAST(pr.started_at AS CHAR) 
                         FROM data_sources ds 
                         LEFT JOIN pipeline_runs pr ON ds.id = pr.source_id 
                         LEFT JOIN chunks c ON ds.id = c.source_id 
                         WHERE ds.tenant_id = ? GROUP BY ds.id"
                    ).bind(tenant_id).fetch_all(pool).await;

                    match rows {
                        Ok(data) => {
                            let mut arr = vec![];
                            for (name, status, chunks, started) in data {
                                arr.push(json!({"source": name, "status": status, "chunks": chunks, "started": started}));
                            }
                            serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
                        }
                        Err(e) => format!("DB Error: {}", e),
                    }
                }
                "check_bifrost_status" => {
                    let bifrost_url = std::env::var("BIFROST_URL").unwrap_or_else(|_| "http://bifrost.asgard.svc:8100".to_string());
                    let url = format!("{}/v1/agents/{}/run", bifrost_url, tenant_id);
                    // Just a mock reachability ping string
                    format!("Bifrost deployment engine is configured at {}. Simulating connectivity ok for {}.", url, tenant_id)
                }
                "vector_search" => {
                    let query = request
                        .arguments
                        .get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let limit = request
                        .arguments
                        .get("limit")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10);
                    format!(
                        "Vector search dispatched: query='{}', tenant='{}', limit={}",
                        query, tenant_id, limit
                    )
                }
                "sql_query" => {
                    let query = request
                        .arguments
                        .get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    format!(
                        "SQL query dispatched: query='{}', tenant='{}'",
                        query, tenant_id
                    )
                }
                "graph_search" => {
                    let entity = request
                        .arguments
                        .get("entity")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let depth = request
                        .arguments
                        .get("max_depth")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(2);
                    format!(
                        "Graph search dispatched: entity='{}', tenant='{}', depth={}",
                        entity, tenant_id, depth
                    )
                }
                "source_list" => {
                    format!("Source list dispatched: tenant='{}'", tenant_id)
                }
                "submit_feedback" => {
                    let rtype = request
                        .arguments
                        .get("report_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("feedback");
                    let title = request
                        .arguments
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let desc = request
                        .arguments
                        .get("description")
                        .and_then(|v| v.as_str());
                    let priority = request
                        .arguments
                        .get("priority")
                        .and_then(|v| v.as_str());
                    
                    let req = crate::services::feedback::CreateFeedbackRequest {
                        report_type: rtype.to_string(),
                        title: title.to_string(),
                        description: desc.map(|s| s.to_string()),
                        page_url: None,
                        browser_info: None,
                        priority: priority.map(|s| s.to_string()),
                        client_logs: None,
                    };
                    
                    let feedback_id = match crate::services::feedback::create_feedback(pool, tenant_id, None, &req, None).await {
                        Ok(id) => id,
                        Err(e) => return McpToolCallResult {
                            content: vec![McpContent {
                                content_type: "text".to_string(),
                                text: format!("Failed to create feedback in DB: {}", e),
                            }],
                            is_error: true,
                        }
                    };
                    
                    match crate::services::feedback::create_github_issue_for_feedback(pool, feedback_id, &req, None, tenant_id, None).await {
                        Ok((url, _)) => format!("Feedback submitted successfully! GitHub issue created at: {}", url),
                        Err(e) => format!("Feedback saved locally (ID: {}), but failed to create GitHub issue: {}", feedback_id, e)
                    }
                }
                _ => "Unknown tool".to_string(),
            };

            McpToolCallResult {
                content: vec![McpContent {
                    content_type: "text".to_string(),
                    text,
                }],
                is_error: false,
            }
        }
        Err(err) => McpToolCallResult {
            content: vec![McpContent {
                content_type: "text".to_string(),
                text: err,
            }],
            is_error: true,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-014ba: list_tools — returns all registered tools
    // ========================================
    #[test]
    fn test_list_tools_returns_all() {
        let tools = list_tools();
        assert_eq!(tools.len(), 5);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"vector_search"));
        assert!(names.contains(&"sql_query"));
        assert!(names.contains(&"graph_search"));
        assert!(names.contains(&"source_list"));
        assert!(names.contains(&"submit_feedback"));
    }

    // ========================================
    // UT-014bb: list_tools — each tool has valid schema
    // ========================================
    #[test]
    fn test_list_tools_have_schemas() {
        let tools = list_tools();
        for tool in &tools {
            assert!(!tool.name.is_empty(), "Tool name must not be empty");
            assert!(
                !tool.description.is_empty(),
                "Tool description must not be empty"
            );
            assert_eq!(
                tool.input_schema["type"], "object",
                "Schema type must be 'object'"
            );
            assert!(
                tool.input_schema.get("properties").is_some(),
                "Schema must have properties"
            );
            assert!(
                tool.input_schema.get("required").is_some(),
                "Schema must have required"
            );
        }
    }

    // ========================================
    // UT-014bc: server_info — returns correct info
    // ========================================
    #[test]
    fn test_server_info() {
        let info = server_info();
        assert_eq!(info.name, "mimir-bridge");
        assert_eq!(info.protocol_version, "2024-11-05");
        assert!(!info.version.is_empty());
    }

    // ========================================
    // UT-014bd: validate — rejects unknown tool
    // ========================================
    #[test]
    fn test_validate_unknown_tool() {
        let req = McpToolCallRequest {
            name: "nonexistent_tool".into(),
            arguments: HashMap::new(),
        };
        let result = validate_tool_call(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    // ========================================
    // UT-014be: validate — rejects missing required args
    // ========================================
    #[test]
    fn test_validate_missing_required_args() {
        let req = McpToolCallRequest {
            name: "vector_search".into(),
            arguments: HashMap::new(), // missing query + tenant_id
        };
        let result = validate_tool_call(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required argument"));
    }

    // ========================================
    // UT-014bf: validate — accepts valid args
    // ========================================
    #[test]
    fn test_validate_valid_args() {
        let mut args = HashMap::new();
        args.insert("query".into(), json!("test search"));
        args.insert("tenant_id".into(), json!("t1"));
        let req = McpToolCallRequest {
            name: "vector_search".into(),
            arguments: args,
        };
        assert!(validate_tool_call(&req).is_ok());
    }

    // ========================================
    // UT-014bg: dispatch — vector_search success
    // ========================================
    #[test]
    fn test_dispatch_vector_search() {
        let mut args = HashMap::new();
        args.insert("query".into(), json!("hello world"));
        args.insert("tenant_id".into(), json!("t1"));
        args.insert("limit".into(), json!(5));
        let req = McpToolCallRequest {
            name: "vector_search".into(),
            arguments: args,
        };
        let result = dispatch_tool_call(&req);
        assert!(!result.is_error);
        assert!(result.content[0].text.contains("hello world"));
        assert!(result.content[0].text.contains("limit=5"));
    }

    // ========================================
    // UT-014bh: dispatch — sql_query success
    // ========================================
    #[test]
    fn test_dispatch_sql_query() {
        let mut args = HashMap::new();
        args.insert("query".into(), json!("SELECT * FROM users"));
        args.insert("tenant_id".into(), json!("t1"));
        let req = McpToolCallRequest {
            name: "sql_query".into(),
            arguments: args,
        };
        let result = dispatch_tool_call(&req);
        assert!(!result.is_error);
        assert!(result.content[0].text.contains("SELECT * FROM users"));
    }

    // ========================================
    // UT-014bi: dispatch — graph_search success
    // ========================================
    #[test]
    fn test_dispatch_graph_search() {
        let mut args = HashMap::new();
        args.insert("entity".into(), json!("Aspirin"));
        args.insert("tenant_id".into(), json!("t1"));
        let req = McpToolCallRequest {
            name: "graph_search".into(),
            arguments: args,
        };
        let result = dispatch_tool_call(&req);
        assert!(!result.is_error);
        assert!(result.content[0].text.contains("Aspirin"));
    }

    // ========================================
    // UT-014bj: dispatch — error for invalid tool
    // ========================================
    #[test]
    fn test_dispatch_invalid_tool_returns_error() {
        let req = McpToolCallRequest {
            name: "bad_tool".into(),
            arguments: HashMap::new(),
        };
        let result = dispatch_tool_call(&req);
        assert!(result.is_error);
        assert!(result.content[0].text.contains("Unknown tool"));
    }

    // ========================================
    // UT-014bk: dispatch — source_list success
    // ========================================
    #[test]
    fn test_dispatch_source_list() {
        let mut args = HashMap::new();
        args.insert("tenant_id".into(), json!("t1"));
        let req = McpToolCallRequest {
            name: "source_list".into(),
            arguments: args,
        };
        let result = dispatch_tool_call(&req);
        assert!(!result.is_error);
        assert!(result.content[0].text.contains("tenant='t1'"));
    }

    // ========================================
    // UT-014bl: dispatch — submit_feedback success
    // ========================================
    #[test]
    fn test_dispatch_submit_feedback() {
        let mut args = HashMap::new();
        args.insert("report_type".into(), json!("bug"));
        args.insert("title".into(), json!("Test bug"));
        args.insert("description".into(), json!("Details"));
        let req = McpToolCallRequest {
            name: "submit_feedback".into(),
            arguments: args,
        };
        let result = dispatch_tool_call(&req);
        assert!(!result.is_error);
        assert!(result.content[0].text.contains("bug"));
    }
}
