//! MLX + vLLM + Heimdall Providers (Issue #163, #180)
//!
//! Local LLM provider integration with OpenAI-compatible API format.
//! Supports MLX Server, vLLM, Heimdall Gateway, and benchmarking across providers.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Supported LLM providers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LlmProvider {
    Gemini,
    MLX,
    VLLM,
    Heimdall,
}

impl LlmProvider {
    pub fn as_str(&self) -> &str {
        match self {
            LlmProvider::Gemini => "gemini",
            LlmProvider::MLX => "mlx",
            LlmProvider::VLLM => "vllm",
            LlmProvider::Heimdall => "heimdall",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "gemini" => Some(LlmProvider::Gemini),
            "mlx" => Some(LlmProvider::MLX),
            "vllm" => Some(LlmProvider::VLLM),
            "heimdall" => Some(LlmProvider::Heimdall),
            _ => None,
        }
    }
}

/// Provider-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: LlmProvider,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl ProviderConfig {
    pub fn mlx_default() -> Self {
        Self {
            provider: LlmProvider::MLX,
            endpoint: "http://localhost:8080".to_string(),
            model: "mlx-community/Mistral-7B-Instruct-v0.3-4bit".to_string(),
            api_key: None,
            max_tokens: 2048,
            temperature: 0.7,
        }
    }

    pub fn vllm_default() -> Self {
        Self {
            provider: LlmProvider::VLLM,
            endpoint: "http://localhost:8000".to_string(),
            model: "mistralai/Mistral-7B-Instruct-v0.3".to_string(),
            api_key: None,
            max_tokens: 2048,
            temperature: 0.7,
        }
    }

    pub fn gemini_default(api_key: &str) -> Self {
        Self {
            provider: LlmProvider::Gemini,
            endpoint: "https://generativelanguage.googleapis.com".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: Some(api_key.to_string()),
            max_tokens: 8192,
            temperature: 0.7,
        }
    }

    /// Heimdall self-hosted LLM gateway (OpenAI-compatible, requires API key)
    pub fn heimdall_default(api_key: &str) -> Self {
        Self {
            provider: LlmProvider::Heimdall,
            endpoint: std::env::var("HEIMDALL_API_URL")
                .unwrap_or_else(|_| "http://192.168.1.133:3000/v1/".to_string()),
            model: std::env::var("HEIMDALL_MODEL")
                .unwrap_or_else(|_| "mlx-community/Qwen3.5-35B-A3B-4bit".to_string()),
            api_key: Some(api_key.to_string()),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// Heimdall available models registry
pub const HEIMDALL_MODELS: &[(&str, &str, &str)] = &[
    ("mlx-community/Qwen3.5-35B-A3B-4bit", "35B (MoE 3B Active)", "Primary — RAG, Chat, QA Generation"),
    ("mlx-community/Qwen3.5-27B-4bit", "27B", "Complex reasoning tasks"),
    ("mlx-community/Qwen3.5-9B-MLX-4bit", "9B", "Light tasks, low latency"),
    ("mlx-community/Qwen3-0.6B-4bit", "0.6B", "Smoke test, health check"),
    ("lmstudio-community/medgemma-4b-it-MLX-4bit", "4B", "Medical domain, OCR"),
];

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Chat completion request (OpenAI-compatible)
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Chat completion response (OpenAI-compatible)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Embedding request
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
}

/// Model info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub owned_by: String,
}

/// Benchmark result
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    pub provider: String,
    pub model: String,
    pub latency_ms: f64,
    pub tokens_per_second: f64,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub success: bool,
    pub error: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pure Functions — TDD-testable (no I/O)
// ═══════════════════════════════════════════════════════════════════════════════

/// Build an OpenAI-compatible chat completion request for MLX Server.
pub fn build_mlx_request(config: &ProviderConfig, messages: &[ChatMessage]) -> Value {
    json!({
        "model": config.model,
        "messages": messages,
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": false
    })
}

/// Build an OpenAI-compatible chat completion request for vLLM.
pub fn build_vllm_request(config: &ProviderConfig, messages: &[ChatMessage]) -> Value {
    json!({
        "model": config.model,
        "messages": messages,
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": false,
        "n": 1
    })
}

/// Build an OpenAI-compatible chat completion request for Heimdall Gateway.
///
/// Heimdall uses the same OpenAI-compatible format as MLX,
/// but always includes the model ID from the registry.
pub fn build_heimdall_request(config: &ProviderConfig, messages: &[ChatMessage]) -> Value {
    json!({
        "model": config.model,
        "messages": messages,
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": false
    })
}

/// Build the full endpoint URL for chat completions.
pub fn build_chat_url(config: &ProviderConfig) -> String {
    let base = config.endpoint.trim_end_matches('/');
    match config.provider {
        LlmProvider::MLX | LlmProvider::VLLM | LlmProvider::Heimdall => {
            format!("{}/v1/chat/completions", base)
        }
        LlmProvider::Gemini => {
            format!("{}/v1beta/models/{}:generateContent", base, config.model)
        }
    }
}

/// Build the models list URL.
pub fn build_models_url(config: &ProviderConfig) -> String {
    let base = config.endpoint.trim_end_matches('/');
    match config.provider {
        LlmProvider::MLX | LlmProvider::VLLM | LlmProvider::Heimdall => {
            format!("{}/v1/models", base)
        }
        LlmProvider::Gemini => {
            format!("{}/v1beta/models", base)
        }
    }
}

/// Build the embeddings URL.
pub fn build_embeddings_url(config: &ProviderConfig) -> String {
    let base = config.endpoint.trim_end_matches('/');
    match config.provider {
        LlmProvider::MLX | LlmProvider::VLLM | LlmProvider::Heimdall => {
            format!("{}/v1/embeddings", base)
        }
        LlmProvider::Gemini => {
            format!("{}/v1beta/models/{}:embedContent", base, config.model)
        }
    }
}

/// Parse an OpenAI-compatible chat completion response.
pub fn parse_chat_response(response: &Value) -> Result<ChatCompletionResponse, String> {
    // Parse choices
    let choices = response.get("choices")
        .and_then(|c| c.as_array())
        .ok_or("Missing 'choices' in response")?;

    let parsed_choices: Vec<ChatChoice> = choices.iter().enumerate().map(|(i, choice)| {
        let default_msg = json!({});
        let message = choice.get("message").unwrap_or(&default_msg);
        ChatChoice {
            index: choice.get("index").and_then(|v| v.as_u64()).unwrap_or(i as u64) as usize,
            message: ChatMessage {
                role: message.get("role").and_then(|v| v.as_str()).unwrap_or("assistant").to_string(),
                content: message.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            },
            finish_reason: choice.get("finish_reason").and_then(|v| v.as_str()).map(String::from),
        }
    }).collect();

    let usage = response.get("usage").map(|u| {
        TokenUsage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        }
    });

    Ok(ChatCompletionResponse {
        id: response.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        object: response.get("object").and_then(|v| v.as_str()).unwrap_or("chat.completion").to_string(),
        model: response.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        choices: parsed_choices,
        usage,
    })
}

/// Parse a models list response.
pub fn parse_models_response(response: &Value) -> Vec<ModelInfo> {
    response.get("data")
        .and_then(|d| d.as_array())
        .map(|models| {
            models.iter().map(|m| {
                ModelInfo {
                    id: m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    object: m.get("object").and_then(|v| v.as_str()).unwrap_or("model").to_string(),
                    owned_by: m.get("owned_by").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                }
            }).collect()
        })
        .unwrap_or_default()
}

/// Calculate benchmark metrics from timing data.
pub fn calculate_benchmark(
    provider: &ProviderConfig,
    latency_ms: f64,
    prompt_tokens: u32,
    completion_tokens: u32,
    success: bool,
    error: Option<String>,
) -> BenchmarkResult {
    let tokens_per_second = if latency_ms > 0.0 && success {
        (completion_tokens as f64 / latency_ms) * 1000.0
    } else {
        0.0
    };

    BenchmarkResult {
        provider: provider.provider.as_str().to_string(),
        model: provider.model.clone(),
        latency_ms,
        tokens_per_second,
        prompt_tokens,
        completion_tokens,
        success,
        error,
    }
}

/// Detect GPU availability (checks environment / system info).
pub fn detect_gpu_info() -> Value {
    // Check for Apple Silicon
    let is_apple_silicon = cfg!(target_arch = "aarch64") && cfg!(target_os = "macos");

    // Check CUDA availability via env var
    let cuda_visible = std::env::var("CUDA_VISIBLE_DEVICES").ok();
    let has_cuda = cuda_visible.is_some();

    // Check Heimdall availability
    let heimdall_url = std::env::var("HEIMDALL_API_URL").ok();
    let has_heimdall = heimdall_url.is_some();

    let recommended = if has_heimdall {
        "heimdall"
    } else if is_apple_silicon {
        "mlx"
    } else if has_cuda {
        "vllm"
    } else {
        "gemini"
    };

    json!({
        "apple_silicon": is_apple_silicon,
        "cuda_available": has_cuda,
        "cuda_devices": cuda_visible.unwrap_or_default(),
        "heimdall_available": has_heimdall,
        "heimdall_url": heimdall_url.unwrap_or_default(),
        "recommended_provider": recommended
    })
}

/// Validate provider configuration.
pub fn validate_provider_config(config: &ProviderConfig) -> Result<(), String> {
    if config.endpoint.is_empty() {
        return Err("Endpoint URL is required".to_string());
    }

    if config.model.is_empty() {
        return Err("Model name is required".to_string());
    }

    if config.provider == LlmProvider::Gemini && config.api_key.is_none() {
        return Err("Gemini requires an API key".to_string());
    }

    if config.provider == LlmProvider::Heimdall && config.api_key.is_none() {
        return Err("Heimdall requires an API key".to_string());
    }

    if config.max_tokens == 0 {
        return Err("max_tokens must be > 0".to_string());
    }

    if config.temperature < 0.0 || config.temperature > 2.0 {
        return Err("temperature must be between 0.0 and 2.0".to_string());
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage { role: "system".to_string(), content: "You are a helpful assistant.".to_string() },
            ChatMessage { role: "user".to_string(), content: "Hello!".to_string() },
        ]
    }

    // ========================================
    // UT-014b_q: build_mlx_request
    // ========================================
    #[test]
    fn test_build_mlx_request() {
        let config = ProviderConfig::mlx_default();
        let req = build_mlx_request(&config, &test_messages());

        assert_eq!(req["model"], "mlx-community/Mistral-7B-Instruct-v0.3-4bit");
        assert_eq!(req["max_tokens"], 2048);
        let temp = req["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01, "temperature should be ~0.7, got {}", temp);
        assert_eq!(req["stream"], false);
        assert!(req["messages"].is_array());
        assert_eq!(req["messages"].as_array().unwrap().len(), 2);
    }

    // ========================================
    // UT-014b_r: build_vllm_request
    // ========================================
    #[test]
    fn test_build_vllm_request() {
        let config = ProviderConfig::vllm_default();
        let req = build_vllm_request(&config, &test_messages());

        assert_eq!(req["model"], "mistralai/Mistral-7B-Instruct-v0.3");
        assert_eq!(req["n"], 1);
        assert!(req["messages"].is_array());
    }

    #[test]
    fn test_build_vllm_request_custom_model() {
        let mut config = ProviderConfig::vllm_default();
        config.model = "meta-llama/Llama-3-8B".to_string();
        config.max_tokens = 4096;

        let req = build_vllm_request(&config, &test_messages());
        assert_eq!(req["model"], "meta-llama/Llama-3-8B");
        assert_eq!(req["max_tokens"], 4096);
    }

    // ========================================
    // UT-014b_s: parse_provider_response
    // ========================================
    #[test]
    fn test_parse_chat_response_success() {
        let response = json!({
            "id": "chatcmpl-abc123",
            "object": "chat.completion",
            "model": "mlx-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        });

        let parsed = parse_chat_response(&response).unwrap();
        assert_eq!(parsed.id, "chatcmpl-abc123");
        assert_eq!(parsed.choices.len(), 1);
        assert_eq!(parsed.choices[0].message.content, "Hello! How can I help?");
        assert_eq!(parsed.choices[0].finish_reason.as_deref(), Some("stop"));
        assert_eq!(parsed.usage.as_ref().unwrap().total_tokens, 18);
    }

    #[test]
    fn test_parse_chat_response_missing_choices() {
        let response = json!({ "error": "model not found" });
        assert!(parse_chat_response(&response).is_err());
    }

    #[test]
    fn test_parse_models_response() {
        let response = json!({
            "data": [
                { "id": "model-1", "object": "model", "owned_by": "mlx" },
                { "id": "model-2", "object": "model", "owned_by": "vllm" }
            ]
        });

        let models = parse_models_response(&response);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "model-1");
        assert_eq!(models[1].owned_by, "vllm");
    }

    #[test]
    fn test_parse_models_response_empty() {
        let response = json!({});
        assert!(parse_models_response(&response).is_empty());
    }

    // ========================================
    // UT-014b_t: provider_config validation
    // ========================================
    #[test]
    fn test_validate_provider_config_mlx_ok() {
        let config = ProviderConfig::mlx_default();
        assert!(validate_provider_config(&config).is_ok());
    }

    #[test]
    fn test_validate_provider_config_vllm_ok() {
        let config = ProviderConfig::vllm_default();
        assert!(validate_provider_config(&config).is_ok());
    }

    #[test]
    fn test_validate_provider_config_gemini_no_key() {
        let mut config = ProviderConfig::gemini_default("key");
        config.api_key = None;
        assert!(validate_provider_config(&config).is_err());
    }

    #[test]
    fn test_validate_provider_config_empty_endpoint() {
        let mut config = ProviderConfig::mlx_default();
        config.endpoint = String::new();
        assert!(validate_provider_config(&config).is_err());
    }

    #[test]
    fn test_validate_provider_config_bad_temperature() {
        let mut config = ProviderConfig::mlx_default();
        config.temperature = 3.0;
        assert!(validate_provider_config(&config).is_err());
    }

    #[test]
    fn test_validate_provider_config_zero_tokens() {
        let mut config = ProviderConfig::mlx_default();
        config.max_tokens = 0;
        assert!(validate_provider_config(&config).is_err());
    }

    // ========================================
    // UT-014b_u: benchmark calculations
    // ========================================
    #[test]
    fn test_calculate_benchmark_success() {
        let config = ProviderConfig::mlx_default();
        let result = calculate_benchmark(&config, 500.0, 10, 50, true, None);

        assert_eq!(result.provider, "mlx");
        assert!(result.tokens_per_second > 0.0);
        assert_eq!(result.tokens_per_second, 100.0); // 50 tokens / 0.5s
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_calculate_benchmark_failure() {
        let config = ProviderConfig::vllm_default();
        let result = calculate_benchmark(
            &config, 1000.0, 0, 0, false,
            Some("Connection refused".to_string()),
        );

        assert_eq!(result.provider, "vllm");
        assert_eq!(result.tokens_per_second, 0.0);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    // ========================================
    // URL builders
    // ========================================
    #[test]
    fn test_build_chat_url_mlx() {
        let config = ProviderConfig::mlx_default();
        assert_eq!(build_chat_url(&config), "http://localhost:8080/v1/chat/completions");
    }

    #[test]
    fn test_build_chat_url_vllm() {
        let config = ProviderConfig::vllm_default();
        assert_eq!(build_chat_url(&config), "http://localhost:8000/v1/chat/completions");
    }

    #[test]
    fn test_build_chat_url_gemini() {
        let config = ProviderConfig::gemini_default("key");
        assert!(build_chat_url(&config).contains("generateContent"));
    }

    #[test]
    fn test_build_models_url() {
        let config = ProviderConfig::mlx_default();
        assert_eq!(build_models_url(&config), "http://localhost:8080/v1/models");
    }

    #[test]
    fn test_build_embeddings_url() {
        let config = ProviderConfig::vllm_default();
        assert_eq!(build_embeddings_url(&config), "http://localhost:8000/v1/embeddings");
    }

    // ========================================
    // Provider enum
    // ========================================
    #[test]
    fn test_provider_as_str() {
        assert_eq!(LlmProvider::Gemini.as_str(), "gemini");
        assert_eq!(LlmProvider::MLX.as_str(), "mlx");
        assert_eq!(LlmProvider::VLLM.as_str(), "vllm");
        assert_eq!(LlmProvider::Heimdall.as_str(), "heimdall");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!(LlmProvider::from_str("mlx"), Some(LlmProvider::MLX));
        assert_eq!(LlmProvider::from_str("VLLM"), Some(LlmProvider::VLLM));
        assert_eq!(LlmProvider::from_str("heimdall"), Some(LlmProvider::Heimdall));
        assert_eq!(LlmProvider::from_str("HEIMDALL"), Some(LlmProvider::Heimdall));
        assert_eq!(LlmProvider::from_str("unknown"), None);
    }

    #[test]
    fn test_detect_gpu_info() {
        let info = detect_gpu_info();
        assert!(info.get("apple_silicon").is_some());
        assert!(info.get("recommended_provider").is_some());
        assert!(info.get("heimdall_available").is_some());
    }

    // ========================================
    // UT-014b_v: Heimdall provider
    // ========================================
    #[test]
    fn test_heimdall_default_config() {
        let config = ProviderConfig::heimdall_default("hd-test-key");
        assert_eq!(config.provider, LlmProvider::Heimdall);
        assert!(config.endpoint.contains("192.168.1.133") || !config.endpoint.is_empty());
        assert!(!config.model.is_empty());
        assert_eq!(config.api_key, Some("hd-test-key".to_string()));
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn test_build_heimdall_request() {
        let config = ProviderConfig::heimdall_default("key");
        let req = build_heimdall_request(&config, &test_messages());

        assert_eq!(req["model"], config.model);
        assert_eq!(req["max_tokens"], 4096);
        assert_eq!(req["stream"], false);
        assert!(req["messages"].is_array());
        assert_eq!(req["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_build_chat_url_heimdall() {
        let config = ProviderConfig::heimdall_default("key");
        let url = build_chat_url(&config);
        assert!(url.ends_with("/v1/chat/completions"), "URL should end with /v1/chat/completions, got: {}", url);
    }

    #[test]
    fn test_build_models_url_heimdall() {
        let config = ProviderConfig::heimdall_default("key");
        let url = build_models_url(&config);
        assert!(url.ends_with("/v1/models"), "URL should end with /v1/models, got: {}", url);
    }

    #[test]
    fn test_build_embeddings_url_heimdall() {
        let config = ProviderConfig::heimdall_default("key");
        let url = build_embeddings_url(&config);
        assert!(url.ends_with("/v1/embeddings"), "URL should end with /v1/embeddings, got: {}", url);
    }

    #[test]
    fn test_validate_heimdall_ok() {
        let config = ProviderConfig::heimdall_default("key");
        assert!(validate_provider_config(&config).is_ok());
    }

    #[test]
    fn test_validate_heimdall_no_key() {
        let mut config = ProviderConfig::heimdall_default("key");
        config.api_key = None;
        let err = validate_provider_config(&config).unwrap_err();
        assert!(err.contains("Heimdall requires an API key"));
    }

    #[test]
    fn test_heimdall_models_registry() {
        assert_eq!(HEIMDALL_MODELS.len(), 5);
        // First model should be the primary MoE
        assert!(HEIMDALL_MODELS[0].0.contains("Qwen3.5-35B"));
        // Last model should be medical domain
        assert!(HEIMDALL_MODELS[4].0.contains("medgemma"));
    }

    #[test]
    fn test_calculate_benchmark_heimdall() {
        let config = ProviderConfig::heimdall_default("key");
        let result = calculate_benchmark(&config, 250.0, 20, 100, true, None);
        assert_eq!(result.provider, "heimdall");
        assert!(result.tokens_per_second > 0.0);
        assert_eq!(result.tokens_per_second, 400.0); // 100 tokens / 0.25s
        assert!(result.success);
    }

    // UT-014b_w: Heimdall request is OpenAI-compatible (same structure as MLX)
    #[test]
    fn test_heimdall_request_openai_compatible() {
        let hd_config = ProviderConfig::heimdall_default("key");
        let mlx_config = ProviderConfig::mlx_default();
        let msgs = test_messages();

        let hd_req = build_heimdall_request(&hd_config, &msgs);
        let mlx_req = build_mlx_request(&mlx_config, &msgs);

        // Same structure keys
        assert!(hd_req.get("model").is_some());
        assert!(hd_req.get("messages").is_some());
        assert!(hd_req.get("max_tokens").is_some());
        assert!(hd_req.get("temperature").is_some());
        assert!(hd_req.get("stream").is_some());
        // MLX has the same keys
        assert_eq!(hd_req.as_object().unwrap().keys().collect::<Vec<_>>().len(),
                   mlx_req.as_object().unwrap().keys().collect::<Vec<_>>().len());
    }
}
