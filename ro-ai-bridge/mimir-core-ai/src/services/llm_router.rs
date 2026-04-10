use anyhow::{Result, anyhow};
use rig::providers::{gemini, ollama};
use std::env;

use crate::models::iam::LlmConfig;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug)]
pub enum AgentResponse {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}
#[derive(Clone)]
pub enum UniversalClient {
    Ollama(ollama::Client),
    Gemini(gemini::Client),
    /// OpenAI-compatible REST API endpoints (e.g. Heimdall, OpenAI, Azure, Flash-MoE)
    Rest {
        provider: String,
        client: reqwest::Client,
        endpoint: String,
        api_key: String,
    },
}

impl UniversalClient {
    /// Helper to get the provider name for DB storage
    pub fn provider_name(&self) -> &str {
        match self {
            Self::Ollama(_) => "ollama",
            Self::Gemini(_) => "gemini",
            Self::Rest { provider, .. } => provider.as_str(),
        }
    }

    /// Unified prompt execution across all supported LLM providers
    pub async fn prompt(
        &self,
        model: &str,
        preamble: &str,
        input: &str,
        max_tokens: u16,
        temperature: f32,
    ) -> Result<String> {
        use rig::completion::Prompt;

        match self {
            Self::Ollama(c) => c
                .agent(model)
                .preamble(preamble)
                .build()
                .prompt(input)
                .await
                .map_err(|e| anyhow!("Ollama prompt error: {}", e)),
            Self::Gemini(c) => c
                .agent(model)
                .preamble(preamble)
                .build()
                .prompt(input)
                .await
                .map_err(|e| anyhow!("Gemini prompt error: {}", e)),
            Self::Rest {
                client,
                endpoint,
                api_key,
                ..
            } => {
                let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        { "role": "system", "content": preamble },
                        { "role": "user", "content": input }
                    ],
                    "max_tokens": max_tokens,
                    "temperature": temperature,
                    "stream": false
                });

                let resp = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key.trim()))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow!("Rest request failed: {}", e))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow!("Rest request failed with status {}: {}", status, text));
                }

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| anyhow!("Rest parse error: {}", e))?;

                json.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow!("Rest: no content in response"))
            }
        }
    }

    /// Agentic loop Execution with full history and JSON Tool definitions
    pub async fn prompt_with_tools(
        &self,
        model: &str,
        messages: serde_json::Value,
        tools: Option<serde_json::Value>,
        max_tokens: u16,
        temperature: f32,
    ) -> Result<AgentResponse> {
        match self {
            Self::Rest {
                client,
                endpoint,
                api_key,
                ..
            } => {
                let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
                let mut body = serde_json::json!({
                    "model": model,
                    "messages": messages,
                    "max_tokens": max_tokens,
                    "temperature": temperature,
                    "stream": false
                });

                if let Some(t) = tools {
                    body.as_object_mut().unwrap().insert("tools".to_string(), t);
                }

                let resp = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key.trim()))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("Rest request failed: {}", e))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("Rest request failed with status {}: {}", status, text));
                }

                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| anyhow::anyhow!("Rest parse error: {}", e))?;

                let message_obj = json.get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("message"))
                    .ok_or_else(|| anyhow::anyhow!("Rest: no message in response: {:?}", json))?;

                if let Some(tool_calls_arr) = message_obj.get("tool_calls") {
                    let calls: Vec<ToolCall> = serde_json::from_value(tool_calls_arr.clone())
                        .map_err(|e| anyhow::anyhow!("Failed to parse tool_calls: {}", e))?;
                    if !calls.is_empty() {
                        return Ok(AgentResponse::ToolCalls(calls));
                    }
                }

                if let Some(content) = message_obj.get("content").and_then(|c| c.as_str()) {
                    return Ok(AgentResponse::Text(content.to_string()));
                }

                Err(anyhow::anyhow!("Rest: neither content nor tool_calls found"))
            }
            _ => Err(anyhow::anyhow!("prompt_with_tools is only supported for Rest clients")),
        }
    }

    /// Execute Cross-Encoder reranking using TEI interface
    pub async fn rerank(
        &self,
        model: &str,
        query: &str,
        texts: &[String],
    ) -> Result<Vec<(usize, f32)>> {
        match self {
            Self::Ollama(_) | Self::Gemini(_) => {
                Err(anyhow!("Reranker API not natively supported for Ollama/Gemini via text endpoints"))
            }
            Self::Rest {
                client,
                endpoint,
                api_key,
                ..
            } => {
                let url = format!("{}/rerank", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "query": query,
                    "texts": texts,
                    "return_text": false
                });

                let mut req = client.post(&url);
                if !api_key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", api_key));
                }

                let resp = req
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow!("Rerank request failed: {}", e))?;

                if !resp.status().is_success() {
                    let err_txt = resp.text().await.unwrap_or_default();
                    return Err(anyhow!("Rerank API error: {}", err_txt));
                }

                #[derive(serde::Deserialize)]
                struct RerankResult {
                    index: usize,
                    score: f32,
                }

                // TEI might return an array directly, or an object containing "results"
                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| anyhow!("Rerank parse error: {}", e))?;

                let results: Vec<RerankResult> = if json.is_array() {
                    serde_json::from_value(json).unwrap_or_default()
                } else if let Some(res) = json.get("results") {
                    serde_json::from_value(res.clone()).unwrap_or_default()
                } else {
                    return Err(anyhow!("Unexpected rerank response format"));
                };

                Ok(results.into_iter().map(|r| (r.index, r.score)).collect())
            }
        }
    }
}

/// Centralized Router for resolving LLM interfaces based on tenant configuration.
#[derive(Clone)]
pub struct LlmRouter {
    pub tenant_id: String,
    pub config: LlmConfig,
    pub default_provider: String,
    pub default_model: String,
}

impl LlmRouter {
    /// Loads the LLM router for a specific tenant.
    /// Falls back to environment-based defaults if the tenant has no config row.
    pub async fn new(pool: crate::services::db::DbPool, tenant_id: &str) -> Result<Self> {
        let iam = crate::services::iam::IamService::new_with_env(pool);

        // Graceful fallback: if tenant has no config row, use defaults
        let tenant_config = match iam.get_tenant_config(tenant_id).await {
            Ok(tc) => tc,
            Err(e) => {
                tracing::warn!(
                    "⚠️ No tenant config found for '{}': {}. Using default LLM configuration.",
                    tenant_id,
                    e
                );
                // Return a router with default env-based configuration
                return Ok(Self {
                    tenant_id: tenant_id.to_string(),
                    config: LlmConfig::default(),
                    default_provider: "heimdall".to_string(),
                    default_model: "mlx-community/gemma-4-26b-a4b-it-4bit".to_string(),
                });
            }
        };

        let config = tenant_config
            .llm_config
            .as_ref()
            .map(|c| c.0.clone())
            .unwrap_or_default();

        let default_provider = tenant_config.default_provider.clone();
        let default_model = tenant_config.default_model.clone();

        Ok(Self {
            tenant_id: tenant_id.to_string(),
            config,
            default_provider,
            default_model,
        })
    }

    /// Get the Heimdall endpoint and API key, if configured.
    pub fn get_heimdall_credentials(&self) -> Result<(String, String)> {
        let endpoint = self
            .config
            .heimdall_url
            .clone()
            .or_else(|| env::var("HEIMDALL_API_URL").ok())
            .ok_or_else(|| anyhow!("Heimdall API URL not configured for tenant or globally"))?;

        let api_key = self
            .config
            .heimdall_api_key
            .clone()
            .or_else(|| env::var("HEIMDALL_API_KEY").ok())
            .unwrap_or_default();

        Ok((endpoint, api_key))
    }

    /// Resolves the LLM client configured for a specific purpose (slot).
    /// Purpose examples: "pipeline_generator", "pipeline_evaluator", "judge", "chat"
    pub fn resolve_client(&self, purpose: &str) -> Result<(UniversalClient, String)> {
        self.resolve_client_with_overrides(purpose, None, None)
    }

    /// Resolves the LLM client configured for a specific purpose (slot) with optional overrides.
    pub fn resolve_client_with_overrides(
        &self,
        purpose: &str,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> Result<(UniversalClient, String)> {
        let slot = if let (Some(p), Some(m)) = (provider_override, model_override) {
            crate::models::iam::LlmSlot {
                provider: p.to_string(),
                model: m.to_string()
            }
        } else {
            self.config.resolve_slot(purpose, Some(&self.default_provider), Some(&self.default_model))
        };

        match slot.provider.to_lowercase().as_str() {
            "gemini" => {
                // Read from llm_config.google_api_key (migrated from provider_api_keys column)
                let api_key = self
                    .config
                    .google_api_key
                    .clone()
                    .or_else(|| env::var("GEMINI_API_KEY").ok())
                    .unwrap_or_default();
                if api_key.is_empty() {
                    return Err(anyhow!(
                        "GEMINI_API_KEY not set in tenant config or globally"
                    ));
                }
                let client = gemini::Client::new(&api_key);
                Ok((UniversalClient::Gemini(client), slot.model))
            }
            "ollama" => {
                let url = env::var("OLLAMA_URL")
                    .or_else(|_| env::var("OLLAMA_HOST").map(|h| format!("http://{}", h)))
                    .unwrap_or_else(|_| "http://localhost:11434".to_string());
                let client = ollama::Client::from_url(&url);
                Ok((UniversalClient::Ollama(client), slot.model))
            }
            "heimdall" | "openai" | "rest" | "azure" | "flashmoe" => {
                let provider = slot.provider.to_lowercase();

                if provider == "flashmoe" {
                    // Flash-MoE Standalone Engine running via Sidecar
                    let endpoint = env::var("FLASHMOE_API_URL")
                        .unwrap_or_else(|_| "http://localhost:8081/v1".to_string());
                    Ok((
                        UniversalClient::Rest {
                            provider: "flashmoe".to_string(),
                            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default(),
                            endpoint,
                            api_key: "flashmoe-local".to_string(),
                        },
                        slot.model,
                    ))
                } else if provider == "openai" {
                    // Read from llm_config.openai_api_key (migrated from provider_api_keys)
                    let api_key = self
                        .config
                        .openai_api_key
                        .clone()
                        .or_else(|| env::var("OPENAI_API_KEY").ok())
                        .unwrap_or_default();
                    if api_key.is_empty() {
                        return Err(anyhow!(
                            "OpenAI API Key not set in tenant config or globally"
                        ));
                    }
                    Ok((
                        UniversalClient::Rest {
                            provider: "openai".to_string(),
                            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default(),
                            endpoint: "https://api.openai.com/v1".to_string(),
                            api_key,
                        },
                        slot.model,
                    ))
                } else if provider == "azure" {
                    // Read from llm_config.azure_api_key (migrated from provider_api_keys)
                    let api_key = self.config.azure_api_key.clone().unwrap_or_default();
                    // Azure endpoint could be stored as a separate field; fall back to env
                    let endpoint = env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default();
                    if api_key.is_empty() || endpoint.is_empty() {
                        return Err(anyhow!(
                            "Azure API Key or Endpoint not set in tenant config or globally"
                        ));
                    }
                    Ok((
                        UniversalClient::Rest {
                            provider: "azure".to_string(),
                            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default(),
                            endpoint,
                            api_key,
                        },
                        slot.model,
                    ))
                } else {
                    // Heimdall / Rest
                    let (endpoint, api_key) = self.get_heimdall_credentials()?;
                    Ok((
                        UniversalClient::Rest {
                            provider: "heimdall".to_string(),
                            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default(),
                            endpoint,
                            api_key,
                        },
                        slot.model,
                    ))
                }
            }
            other => Err(anyhow!("Unsupported LLM provider: {}", other)),
        }
    }

    /// Resolves the reranker. Defaults to Heimdall API TEI endpoints.
    pub fn resolve_reranker(&self, requested_model: Option<&str>) -> Result<(UniversalClient, String)> {
        // Fallback to Heimdall TEI model if none specified
        let model = requested_model.unwrap_or("BAAI/bge-reranker-v2-m3").to_string();
        let (endpoint, api_key) = self.get_heimdall_credentials()?;
        
        Ok((
            UniversalClient::Rest {
                provider: "heimdall".to_string(),
                client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default(),
                endpoint,
                api_key,
            },
            model,
        ))
    }

    /// Embed texts via requested embedding provider matching slot, or default Heimdall API
    pub async fn embed_texts_strict(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let slot = self.config.resolve_slot("embedding", None, None);

        // Use tenant's configured embedding model (respects admin Settings → Search tab config)
        let model = if !slot.model.is_empty() {
            slot.model.clone()
        } else {
            "BAAI/bge-m3".to_string()
        };

        let (endpoint, api_key) = self.get_heimdall_credentials()?;
        let embed_url = format!("{}/embeddings", endpoint.trim_end_matches('/'));

        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default();
        let resp = client
            .post(&embed_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": model, // Dynamic assignment derived from tenant settings or Heimdall standard override
                "input": texts,
            }))
            .send()
            .await
            .map_err(|e| anyhow!("Embedding HTTP error (Heimdall API): {}", e))?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Heimdall Embedding API error: {}", err));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse Heimdall embedding response: {}", e))?;

        let data = body["data"]
            .as_array()
            .ok_or_else(|| anyhow!("No 'data' array in Heimdall response"))?;
        let mut vectors = Vec::with_capacity(data.len());
        for item in data {
            let vec: Vec<f32> = item["embedding"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect()
                })
                .unwrap_or_default();
            vectors.push(vec);
        }
        Ok(vectors)
    }
}
