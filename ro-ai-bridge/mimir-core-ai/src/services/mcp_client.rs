use anyhow::{Result, Context};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use futures::StreamExt;
use tracing::{info, warn, error};

#[derive(Clone)]
pub struct McpClient {
    client: Client,
    base_url: String,
    session_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SseEndpointEvent {
    uri: String,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<Value>,
    id: u64,
}

impl McpClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            session_endpoint: None,
        }
    }

    /// Connect to the MCP server via SSE to initialize the session.
    /// Returns the active session endpoint (POST URL).
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to MCP Server at {}/sse", self.base_url);
        
        let sse_url = format!("{}/sse", self.base_url.trim_end_matches('/'));
        let mut source = EventSource::get(&sse_url);

        // 1. Wait for 'endpoint' event to get the POST URI
        while let Some(event) = source.next().await {
            match event {
                Ok(Event::Open) => info!("SSE Connection Open"),
                Ok(Event::Message(message)) => {
                    if message.event == "endpoint" {
                        let endpoint_url = message.data;
                        info!("Received Session Endpoint: {}", endpoint_url);
                        
                        // Construct absolute URL if relative
                        self.session_endpoint = Some(if endpoint_url.starts_with("http") {
                            endpoint_url
                        } else {
                            // Logic to combine base + relative uri (GitBook MCP usually returns relative)
                            // Assuming base is the knowledge base URL
                            // Warning: This joining needs to be robust. 
                            let base = self.base_url.trim_end_matches("/");
                            // e.g. base_url/messages?sessionId=...
                             format!("{}{}", base, endpoint_url)
                        });
                        
                        // Stop listening after receiving endpoint (or keep it open for notifications?)
                        // For basic fetch, we got what we need. 
                        // Note: SSE connection must usually stay open for the session to be valid in MCP.
                        // So we should spawn a task to keep reading events (and handle notifications).
                        tokio::spawn(async move {
                            while let Some(e) = source.next().await {
                                if let Err(err) = e {
                                    warn!("SSE Error in background: {:?}", err);
                                    break;
                                }
                            }
                        });
                        
                        break;
                    }
                }
                Err(e) => {
                    error!("SSE Connection failed: {:?}", e);
                    return Err(anyhow::anyhow!("SSE Connection failed: {:?}", e));
                }
            }
        }
        
        // 2. Send 'initialize' request
        self.initialize().await?;

        Ok(())
    }

    async fn initialize(&self) -> Result<()> {
        info!("Sending Initialize Request...");
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": true },
                "sampling": {}
            },
            "clientInfo": {
                "name": "ro-ai-bridge",
                "version": "0.1.0"
            }
        });
        
        let response = self.send_request("initialize", params).await?;
        info!("Initialized: {:?}", response);
        
        // Send 'notifications/initialized'
        self.send_notification("notifications/initialized", json!({})).await?;
        
        Ok(())
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let endpoint = self.session_endpoint.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Session endpoint not found. Did you call connect()?"))?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as u64,
        };

        let res = self.client.post(endpoint)
            .json(&request)
            .send()
            .await?;

        let json_resp: JsonRpcResponse = res.json().await?;
        
        if let Some(error) = json_resp.error {
            return Err(anyhow::anyhow!("JSON-RPC Error: {:?}", error));
        }

        Ok(json_resp.result.unwrap_or(Value::Null))
    }
    
    async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
         let endpoint = self.session_endpoint.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Session endpoint not found"))?;
            
        // Notification is just a request without id (strictly speaking, but implementation varies)
        // Or JSON-RPC 2.0 notification omits "id". 
        // We often just fire and forget.
        
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.client.post(endpoint)
            .json(&body)
            .send()
            .await?;
            
        Ok(())
    }

    /// Fetch resources (like Whitepaper pages) from the GitBook MCP server.
    pub async fn fetch_resources(&self) -> Result<Vec<Value>> {
        info!("Fetching resources list...");
        let response = self.send_request("resources/list", json!({})).await?;
        
        // Response format: { "resources": [ ... ] }
        let resources = response.get("resources")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
            
        Ok(resources)
    }
    
    /// Read a specific resource content
    pub async fn read_resource(&self, uri: &str) -> Result<String> {
        info!("Reading resource: {}", uri);
        let response = self.send_request("resources/read", json!({ "uri": uri })).await?;
        
        // Response format: { "contents": [ { "uri": "...", "mimeType": "...", "text": "..." } ] }
        let text = response.get("contents")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
            
        Ok(text)
    }
}
