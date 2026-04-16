use anyhow::{Result, anyhow};
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

/// Unified LLM client — all requests go through Heimdall Gateway.
///
/// Heimdall handles provider routing via model prefix:
/// - No prefix → Local MLX/llama.cpp backend
/// - `openrouter/...` → OpenRouter API
/// - `gemini/...` → Google Gemini API
/// - `openai/...` → OpenAI API
///
/// `provider_key` is forwarded as `X-Provider-Key` header for per-tenant
/// API key override at the Heimdall layer.
#[derive(Clone)]
pub enum UniversalClient {
    /// OpenAI-compatible REST API via Heimdall Gateway
    Rest {
        provider: String,
        client: reqwest::Client,
        endpoint: String,
        api_key: String,
        /// Per-tenant provider key forwarded to Heimdall as `X-Provider-Key` header.
        /// Heimdall uses this to authenticate with external providers (OpenRouter, Gemini, OpenAI)
        /// instead of its own centralized key.
        provider_key: Option<String>,
    },
}

impl UniversalClient {
    /// Helper to get the provider name for DB storage
    pub fn provider_name(&self) -> &str {
        match self {
            Self::Rest { provider, .. } => provider.as_str(),
        }
    }

    /// Build the request headers, including X-Provider-Key if set.
    fn build_headers(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Rest {
                api_key,
                provider_key,
                ..
            } => {
                let mut headers = vec![
                    ("Authorization", format!("Bearer {}", api_key.trim())),
                    ("Content-Type", "application/json".to_string()),
                ];
                if let Some(pk) = provider_key {
                    let cleaned_pk = pk.trim();
                    if !cleaned_pk.is_empty() {
                        headers.push(("X-Provider-Key", cleaned_pk.to_string()));
                    }
                }
                headers
            }
        }
    }

    /// Unified prompt execution — all traffic goes through Heimdall.
    pub async fn prompt(
        &self,
        model: &str,
        preamble: &str,
        input: &str,
        max_tokens: u16,
        temperature: f32,
    ) -> Result<String> {
        match self {
            Self::Rest {
                client,
                endpoint,
                ..
            } => {
                let url = format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'));
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

                let headers = self.build_headers();
                tracing::info!("LLM Router Prompt: Sending to {} with {} headers", url, headers.len());
                let mut req = client.post(&url);
                for (k, v) in &headers {
                    req = req.header(*k, v);
                }
                
                let req_builder = req.json(&body);
                tracing::info!("LLM Router Request Body serialization successful");
                
                let resp = req_builder
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("Rest request failed: Builder Error. URL: '{}', e: {}", url, e))?;

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
                ..
            } => {
                let url = format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'));
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

                let headers = self.build_headers();
                let mut req = client.post(&url);
                for (k, v) in &headers {
                    req = req.header(*k, v);
                }

                let mut hdrs_str = String::new();
                for (k, v) in &headers {
                    hdrs_str.push_str(&format!("{}: {} | ", k, v));
                }
                
                let resp = req
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("Rest request failed: {}, URL: '{}', HEADERS: {}", e, url, hdrs_str))?;

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
        }
    }

    /// Execute Cross-Encoder reranking via Heimdall's `/rerank` endpoint
    pub async fn rerank(
        &self,
        model: &str,
        query: &str,
        texts: &[String],
    ) -> Result<Vec<(usize, f32)>> {
        match self {
            Self::Rest {
                client,
                endpoint,
                ..
            } => {
                let url = format!("{}/rerank", endpoint.trim().trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "query": query,
                    "texts": texts,
                    "return_text": false
                });

                let headers = self.build_headers();
                let mut req = client.post(&url);
                for (k, v) in &headers {
                    req = req.header(*k, v);
                }

                let resp = req
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
///
/// All providers now route through Heimdall Gateway using model-prefix convention:
/// - `"gemini/gemini-2.5-flash"` → Heimdall routes to Google Gemini
/// - `"openrouter/anthropic/claude-3.5-sonnet"` → Heimdall routes to OpenRouter
/// - `"openai/gpt-4o"` → Heimdall routes to OpenAI
/// - `"mlx-community/Qwen3.5-35B-A3B-4bit"` → Heimdall routes locally
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
            .filter(|s| !s.trim().is_empty())
            .or_else(|| env::var("HEIMDALL_API_URL").ok())
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| anyhow!("Heimdall API URL not configured for tenant or globally"))?;

        let api_key = self
            .config
            .heimdall_api_key
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| env::var("HEIMDALL_API_KEY").ok())
            .unwrap_or_default();

        Ok((endpoint, api_key))
    }

    /// Resolve the tenant's provider-specific API key, if any, to forward via X-Provider-Key.
    ///
    /// This allows per-tenant billing: Mimir sends the tenant's own API key to Heimdall,
    /// which forwards it to the external provider instead of using its centralized key.
    fn resolve_provider_key(&self, provider: &str) -> Option<String> {
        match provider {
            "gemini" | "google" => self.config.google_api_key.clone(),
            "openai" => self.config.openai_api_key.clone(),
            // OpenRouter, Azure etc. — could be added to LlmConfig in the future
            _ => None,
        }
    }

    /// Construct the model string with provider prefix for Heimdall routing.
    ///
    /// Maps tenant-level provider names to Heimdall prefix conventions:
    /// - `("gemini", "gemini-2.5-flash")` → `"gemini/gemini-2.5-flash"`
    /// - `("openai", "gpt-4o")` → `"openai/gpt-4o"`
    /// - `("heimdall", "mlx-community/Qwen3.5-35B-A3B-4bit")` → `"mlx-community/Qwen3.5-35B-A3B-4bit"` (no prefix)
    fn prefixed_model(provider: &str, model: &str) -> String {
        match provider {
            "gemini" | "google" => format!("gemini/{}", model),
            "openai" => format!("openai/{}", model),
            "openrouter" => format!("openrouter/{}", model),
            // Local providers — no prefix needed
            "heimdall" | "rest" | "flashmoe" | _ => model.to_string(),
        }
    }

    /// Resolves the LLM client configured for a specific purpose (slot).
    /// Purpose examples: "pipeline_generator", "pipeline_evaluator", "judge", "chat"
    pub fn resolve_client(&self, purpose: &str) -> Result<(UniversalClient, String)> {
        self.resolve_client_with_overrides(purpose, None, None)
    }

    /// Resolves the LLM client configured for a specific purpose (slot) with optional overrides.
    ///
    /// All providers now go through Heimdall Gateway. The provider name determines
    /// the model prefix sent to Heimdall for routing.
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

        let provider = slot.provider.to_lowercase();

        // Get Heimdall credentials — ALL traffic goes through Heimdall
        let (endpoint, api_key) = self.get_heimdall_credentials()?;

        // Build the prefixed model name for Heimdall's router
        let prefixed_model = Self::prefixed_model(&provider, &slot.model);

        // Resolve per-tenant provider key (if tenant has their own API key for this provider)
        let provider_key = self.resolve_provider_key(&provider);

        tracing::debug!(
            "🔀 LlmRouter: purpose={}, provider={}, model={} → prefixed={}",
            purpose,
            provider,
            slot.model,
            prefixed_model
        );

        Ok((
            UniversalClient::Rest {
                provider: provider.clone(),
                client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(300))
                    .build()
                    .unwrap_or_default(),
                endpoint,
                api_key,
                provider_key,
            },
            prefixed_model,
        ))
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
                provider_key: None,
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
        let embed_url = format!("{}/embeddings", endpoint.trim().trim_end_matches('/'));

        // Resolve per-tenant provider key for embedding provider
        let provider_key = self.resolve_provider_key(&slot.provider);

        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)).build().unwrap_or_default();
        let mut req = client
            .post(&embed_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key));

        // Forward tenant-specific provider key if available
        if let Some(pk) = &provider_key {
            if !pk.is_empty() {
                req = req.header("X-Provider-Key", pk);
            }
        }

        let resp = req
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

#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub thinking_tokens: u32,
}

pub fn extract_token_usage(json: &serde_json::Value) -> TokenUsage {
    let mut usage = TokenUsage::default();
    if let Some(u) = json.get("usage") {
        usage.prompt_tokens = u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        usage.completion_tokens = u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if let Some(details) = u.get("completion_tokens_details") {
            usage.thinking_tokens = details.get("reasoning_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        } else {
            // Some providers might just put it at the root of usage
            usage.thinking_tokens = u.get("reasoning_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        }
    }
    usage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_token_usage_openai_format() {
        // TDD test for extracting token usage
        let mock_response = serde_json::json!({
            "choices": [{
                "message": { "content": "hello world" }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "completion_tokens_details": {
                    "reasoning_tokens": 5
                }
            }
        });

        let usage = extract_token_usage(&mock_response);
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.thinking_tokens, 5);
    }

    #[test]
    fn test_extract_token_usage_no_usage_field() {
        let resp = serde_json::json!({"choices": [{"message": {"content": "hi"}}]});
        let usage = extract_token_usage(&resp);
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.thinking_tokens, 0);
    }

    #[test]
    fn test_extract_token_usage_gemini_flat_reasoning() {
        let resp = serde_json::json!({
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 100,
                "reasoning_tokens": 30
            }
        });
        let usage = extract_token_usage(&resp);
        assert_eq!(usage.prompt_tokens, 50);
        assert_eq!(usage.completion_tokens, 100);
        assert_eq!(usage.thinking_tokens, 30);
    }

    // ── New routing tests ──────────────────────────────────────────────────

    #[test]
    fn test_prefixed_model_gemini() {
        assert_eq!(
            LlmRouter::prefixed_model("gemini", "gemini-2.5-flash"),
            "gemini/gemini-2.5-flash"
        );
        assert_eq!(
            LlmRouter::prefixed_model("google", "gemini-2.5-pro"),
            "gemini/gemini-2.5-pro"
        );
    }

    #[test]
    fn test_prefixed_model_openai() {
        assert_eq!(
            LlmRouter::prefixed_model("openai", "gpt-4o"),
            "openai/gpt-4o"
        );
    }

    #[test]
    fn test_prefixed_model_openrouter() {
        assert_eq!(
            LlmRouter::prefixed_model("openrouter", "anthropic/claude-3.5-sonnet"),
            "openrouter/anthropic/claude-3.5-sonnet"
        );
    }

    #[test]
    fn test_prefixed_model_heimdall_no_prefix() {
        assert_eq!(
            LlmRouter::prefixed_model("heimdall", "mlx-community/Qwen3.5-35B-A3B-4bit"),
            "mlx-community/Qwen3.5-35B-A3B-4bit"
        );
    }

    #[test]
    fn test_universal_client_provider_name() {
        let client = UniversalClient::Rest {
            provider: "gemini".to_string(),
            client: reqwest::Client::new(),
            endpoint: "http://localhost:8080/v1".to_string(),
            api_key: "test-key".to_string(),
            provider_key: Some("tenant-gemini-key".to_string()),
        };
        assert_eq!(client.provider_name(), "gemini");
    }

    #[test]
    fn test_build_headers_with_provider_key() {
        let client = UniversalClient::Rest {
            provider: "heimdall".to_string(),
            client: reqwest::Client::new(),
            endpoint: "http://localhost:8080/v1".to_string(),
            api_key: "gateway-key".to_string(),
            provider_key: Some("tenant-openai-key".to_string()),
        };
        let headers = client.build_headers();
        assert_eq!(headers.len(), 3); // Auth + Content-Type + X-Provider-Key
        assert_eq!(headers[2].0, "X-Provider-Key");
        assert_eq!(headers[2].1, "tenant-openai-key");
    }

    #[test]
    fn test_build_headers_without_provider_key() {
        let client = UniversalClient::Rest {
            provider: "heimdall".to_string(),
            client: reqwest::Client::new(),
            endpoint: "http://localhost:8080/v1".to_string(),
            api_key: "gateway-key".to_string(),
            provider_key: None,
        };
        let headers = client.build_headers();
        assert_eq!(headers.len(), 2); // Auth + Content-Type only
    }
}
