use anyhow::{Result, anyhow};
use rig::providers::{gemini, ollama};
use std::env;

use crate::models::iam::LlmConfig;

#[derive(Clone)]
pub enum UniversalClient {
    Ollama(ollama::Client),
    Gemini(gemini::Client),
    /// Heimdall self-hosted LLM gateway (OpenAI-compatible)
    Rest {
        client: reqwest::Client,
        endpoint: String,
        api_key: String,
    },
}

impl UniversalClient {
    /// Helper to get the provider name for DB storage
    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::Ollama(_) => "ollama",
            Self::Gemini(_) => "gemini",
            Self::Rest { .. } => "heimdall",
        }
    }

    /// Unified prompt execution across all supported LLM providers
    pub async fn prompt(&self, model: &str, preamble: &str, input: &str, max_tokens: u16, temperature: f32) -> Result<String> {
        use rig::completion::Prompt;
        
        match self {
            Self::Ollama(c) => {
                c.agent(model).preamble(preamble).build().prompt(input).await.map_err(|e| anyhow!("Ollama prompt error: {}", e))
            },
            Self::Gemini(c) => {
                c.agent(model).preamble(preamble).build().prompt(input).await.map_err(|e| anyhow!("Gemini prompt error: {}", e))
            },
            Self::Rest { client, endpoint, api_key } => {
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
                
                let resp = client.post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow!("Rest request failed: {}", e))?;
                    
                let json: serde_json::Value = resp.json().await
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
}

/// Centralized Router for resolving LLM interfaces based on tenant configuration.
#[derive(Clone)]
pub struct LlmRouter {
    pub tenant_id: String,
    pub config: LlmConfig,
}

impl LlmRouter {
    /// Loads the LLM router for a specific tenant.
    pub async fn new(pool: crate::services::db::DbPool, tenant_id: &str) -> Result<Self> {
        let iam = crate::services::iam::IamService::new_with_env(pool);
        let tenant_config = iam.get_tenant_config(tenant_id).await?;
        
        let config = tenant_config.llm_config.as_ref()
            .map(|c| c.0.clone())
            .unwrap_or_default();
            
        Ok(Self {
            tenant_id: tenant_id.to_string(),
            config,
        })
    }
    
    /// Get the Heimdall endpoint and API key, if configured.
    pub fn get_heimdall_credentials(&self) -> Result<(String, String)> {
        let endpoint = self.config.heimdall_url.clone()
            .or_else(|| env::var("HEIMDALL_API_URL").ok())
            .ok_or_else(|| anyhow!("Heimdall API URL not configured for tenant or globally"))?;
            
        let api_key = self.config.heimdall_api_key.clone()
            .or_else(|| env::var("HEIMDALL_API_KEY").ok())
            .unwrap_or_default();
            
        Ok((endpoint, api_key))
    }

    /// Resolves the LLM client configured for a specific purpose (slot).
    /// Purpose examples: "pipeline_generator", "pipeline_evaluator", "judge", "chat"
    pub fn resolve_client(&self, purpose: &str) -> Result<(UniversalClient, String)> {
        let slot = self.config.resolve_slot(purpose, None, None);
        
        match slot.provider.to_lowercase().as_str() {
            "gemini" => {
                let api_key = env::var("GEMINI_API_KEY")
                    .unwrap_or_else(|_| "".to_string());
                if api_key.is_empty() {
                    return Err(anyhow!("GEMINI_API_KEY not set"));
                }
                let client = gemini::Client::new(&api_key);
                Ok((UniversalClient::Gemini(client), slot.model))
            },
            "ollama" => {
                let url = env::var("OLLAMA_URL")
                    .or_else(|_| env::var("OLLAMA_HOST").map(|h| format!("http://{}", h)))
                    .unwrap_or_else(|_| "http://localhost:11434".to_string());
                let client = ollama::Client::from_url(&url);
                Ok((UniversalClient::Ollama(client), slot.model))
            },
            "heimdall" | "openai" | "rest" => {
                let (endpoint, api_key) = self.get_heimdall_credentials()?;
                Ok((UniversalClient::Rest {
                    client: reqwest::Client::new(),
                    endpoint,
                    api_key,
                }, slot.model))
            },
            other => Err(anyhow!("Unsupported LLM provider: {}", other))
        }
    }
    
    /// Embed texts via requested embedding provider matching slot, or default Heimdall API
    pub async fn embed_texts_strict(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let slot = self.config.resolve_slot("embedding", None, None);
        
        if slot.provider.to_lowercase() == "ollama" {
            // Strictly enforce fail-fast for Heimdall embedding, blocking Ollama fallback explicitly
            tracing::warn!("Embedding slot resolved to Ollama but Heimdall strict mode is enabled. Rerouting to Heimdall API.");
        }
        
        // P0 Fix: Enforce strict embedding logic using actual slot model if it exists, otherwise bge-m3.
        let model = if !slot.model.is_empty() && slot.model != "nomic-embed-text" {
            slot.model.clone()
        } else {
            "BAAI/bge-m3".to_string()
        };
        
        let (endpoint, api_key) = self.get_heimdall_credentials()?;
        let embed_url = format!("{}/embeddings", endpoint.trim_end_matches('/'));
        
        let client = reqwest::Client::new();
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

        let body: serde_json::Value = resp.json().await
            .map_err(|e| anyhow!("Failed to parse Heimdall embedding response: {}", e))?;

        let data = body["data"].as_array().ok_or_else(|| anyhow!("No 'data' array in Heimdall response"))?;
        let mut vectors = Vec::with_capacity(data.len());
        for item in data {
            let vec: Vec<f32> = item["embedding"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
                .unwrap_or_default();
            vectors.push(vec);
        }
        Ok(vectors)
    }
}
