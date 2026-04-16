use axum::{
    extract::{State, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::Deserialize;
use serde_json::json;
use tracing::error;

use crate::routes::tenant::extract_tenant_id;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::services::llm_router::LlmRouter;

#[derive(Deserialize, Debug)]
pub struct AssistantRequest {
    pub message: String,
    pub history: Option<Vec<AssistantMessage>>,
    pub current_page: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssistantMessage {
    pub role: String,
    pub content: String,
}

pub fn assistant_routes() -> Router<DbPool> {
    Router::new().route("/help", post(handle_assistant_chat))
}

/// POST /api/v1/assistant/help — Mimir Global Help Assistant
pub async fn handle_assistant_chat(
    headers: HeaderMap,
    State(pool): State<DbPool>,
    Json(payload): Json<AssistantRequest>,
) -> impl IntoResponse {
    let tenant_id = extract_tenant_id(&headers).to_string();

    let router: LlmRouter = match LlmRouter::new(pool.clone(), &tenant_id).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to init LLM Router for assistant: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to initialize LLM routing."})),
            );
        }
    };

    let (client, model) = match router.resolve_client("chat") {
        Ok(pair @ (mimir_core_ai::services::llm_router::UniversalClient::Rest { .. }, _)) => pair,
        Ok(_) | Err(_) => {
            tracing::info!("Configured chat provider lacks tool-call support. Falling back to Heimdall.");
            
            let (endpoint, api_key) = router.get_heimdall_credentials()
                .unwrap_or_else(|_| (
                    std::env::var("HEIMDALL_API_URL").unwrap_or_else(|_| "http://localhost:8081/v1".to_string()),
                    std::env::var("HEIMDALL_API_KEY").unwrap_or_default(),
                ));

            // Pull the default model from the same env var as config.heimdall_model
            let fallback_model = std::env::var("HEIMDALL_MODEL")
                .unwrap_or_else(|_| "mlx-community/Qwen3.5-35B-A3B-4bit".to_string());
                
            (
                mimir_core_ai::services::llm_router::UniversalClient::Rest {
                    provider: "heimdall".to_string(),
                    provider_key: None,
                    client: reqwest::Client::new(),
                    endpoint,
                    api_key,
                },
                fallback_model
            )
        }
    };

    let sys_prompt = format!(
        "You are 'Mimir', the core intelligence and agentic overseer of the Mimir AI Platform. \
        The Mimir platform is an advanced Agentic Medical RAG (Retrieval-Augmented Generation) system. \
        The user is currently on the page: {}. \
        Your knowledge encompasses the entire Mimir Ecosystem: \
        1. RAG (Retrieval-Augmented Generation): Users can upload documents or connect Web/SQL sources via the 'Data Sources' menu. The system processes these into Vector (Qdrant), Tree, and Graph (Neo4j) spaces. \
        2. Evaluate: Users can test RAG pipeline accuracy through 'Evaluation'. They can run Ground Truth benchmarks to measure Hit Rate, MRR, Context Relevance, and NDCG. \
        3. Agent Studio (Bifrost): Users can create specialized logical Agents in the Studio (Swarm Orchestrator), assign them skill sets, and deploy them to execute workflows or cognitive routing. \
        Your job is to answer questions to guide users on doing RAG, evaluating pipelines, or using Agent Studio. \
        Use the available tools to check pipeline statuses or Bifrost deploy health if asked. \
        Be concise, helpful, and polite. Reply in the same language the user speaks (mostly Thai). \
        If the user wants to report a bug or feature, confirm it using the submit_feedback tool.",
        payload.current_page
    );

    // Setup ReAct Messages
    let mut messages = vec![json!({"role": "system", "content": sys_prompt})];
    if let Some(history) = &payload.history {
        for msg in history {
            let role = if msg.role == "system" { "assistant" } else { &msg.role };
            messages.push(json!({"role": role, "content": msg.content}));
        }
    }
    messages.push(json!({"role": "user", "content": payload.message.clone()}));

    // Dynamically fetch tools from the internal MCP Server
    let tools_req = crate::routes::mcp::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };
    
    let mcp_tools_resp = crate::routes::mcp::handle_mcp_message(&tools_req, &tenant_id, &pool).await;
    let mcp_tools_array = mcp_tools_resp.result.unwrap_or_else(|| json!({"tools": []}))["tools"].clone();
    
    // Transform MCP tools schema to OpenAI Function schema for UniversalClient
    let mut oai_tools = Vec::new();
    if let Some(arr) = mcp_tools_array.as_array() {
        for t in arr {
            oai_tools.push(json!({
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": t["inputSchema"]
                }
            }));
        }
    }

    let mut iterations = 0;
    while iterations < 5 {
        iterations += 1;
        match client.prompt_with_tools(&model, json!(messages), Some(json!(oai_tools)), 2048, 0.3).await {
            Ok(mimir_core_ai::services::llm_router::AgentResponse::Text(reply)) => {
                return (StatusCode::OK, Json(json!({ "reply": reply })));
            }
            Ok(mimir_core_ai::services::llm_router::AgentResponse::ToolCalls(calls)) => {
                let mut tool_calls_json = Vec::new();
                for call in &calls {
                    tool_calls_json.push(json!({
                        "id": call.id,
                        "type": call.r#type,
                        "function": { "name": call.function.name.clone(), "arguments": call.function.arguments.clone() }
                    }));
                }
                messages.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": tool_calls_json
                }));

                for call in calls {
                    // Convert arguments string to JSON Object mapping
                    let args_val: serde_json::Value = serde_json::from_str(&call.function.arguments).unwrap_or(json!({}));
                    
                    let call_req = crate::routes::mcp::JsonRpcRequest {
                        jsonrpc: "2.0".to_string(),
                        id: Some(json!(2)),
                        method: "tools/call".to_string(),
                        params: Some(json!({
                            "name": call.function.name.clone(),
                            "arguments": args_val
                        })),
                    };

                    let call_resp = crate::routes::mcp::handle_mcp_message(&call_req, &tenant_id, &pool).await;
                    let result_str = if let Some(res) = call_resp.result {
                        if !res["is_error"].as_bool().unwrap_or(false) {
                            if let Some(content) = res["content"].as_array() {
                                if !content.is_empty() {
                                    content[0]["text"].as_str().unwrap_or("").to_string()
                                } else {
                                    "No content returned".to_string()
                                }
                            } else {
                                "Unknown result format".to_string()
                            }
                        } else {
                            format!("MCP Tool Error")
                        }
                    } else if let Some(err) = call_resp.error {
                        format!("MCP RPC Error: {}", err.message)
                    } else {
                        "Unknown error".to_string()
                    };

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "name": call.function.name,
                        "content": result_str
                    }));
                }
            }
            Err(e) => {
                tracing::error!("Agent loop failed: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to generate response: {}", e)})),
                );
            }
        }
    }

    (StatusCode::OK, Json(json!({"reply": "I'm sorry, I encountered too many steps processing your request."})))
}
