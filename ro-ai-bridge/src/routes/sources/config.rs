//! LLM configuration, credential resolution, and AI extraction.
//!
//! Public items (used by other route modules):
//! - `resolve_llm_credentials` — used by agents.rs, ocr.rs
//! - `infer_api_base` — used by agents.rs
//! - `call_llm_api` / `call_llm_api_with_logging` — used by ocr.rs

use axum::{
    Json, Extension, extract::{Path, State},
    http::{StatusCode, HeaderMap},
};
use std::sync::Arc;
use crate::config::Config;
use mimir_core_ai::services::db::DbPool;
use mimir_core_ai::models::sources::DataSource;
use mimir_core_ai::services::db;
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use crate::routes::tenant::extract_tenant_id;

// ─── LLM Fallback Extraction ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AiExtractRequest {
    model: String,
    output_format: String,  // "markdown" | "table"
}

#[derive(Debug, Serialize)]
pub(crate) struct AiExtractResponse {
    content: String,
    tokens_used: u32,
    model: String,
}

/// POST /api/v1/sources/:id/extract-ai
///
/// Use an LLM to extract content from a source file when native extraction fails.
/// Downloads the file from S3, sends its text content to the selected LLM,
/// and returns the extracted content.
pub(crate) async fn extract_with_ai(
    headers: HeaderMap,
    Extension(config): Extension<Arc<Config>>,
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<AiExtractRequest>,
) -> Result<Json<AiExtractResponse>, (StatusCode, Json<Value>)> {
    let tenant_id = extract_tenant_id(&headers);

    // 1. Fetch source from DB
    let source = sqlx::query_as::<_, DataSource>("SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?")
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let source = source.ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Source not found"})))
    })?;

    // 2. Download file from S3
    let s3_key = source.s3_key.as_deref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "Source has no S3 file — nothing to extract"})))
    })?;

    let file_data = super::upload::download_from_s3(&config, s3_key).await
        .map_err(|e| {
            error!("S3 download failed for AI extraction: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to download file: {}", e)})))
        })?;

    info!("Downloaded {} bytes from S3 for AI extraction (source_id={})", file_data.len(), id);

    // 3. Look up model configuration from DB
    let model_config = db::get_model_by_id(&pool, &payload.model).await
        .map_err(|e| {
            error!("Failed to look up model {}: {}", payload.model, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Model lookup failed: {}", e)})))
        })?;

    // 4. Determine API key and endpoint from model config or env
    let (api_key, api_base) = resolve_llm_credentials(&config, &model_config, &payload.model)?;

    // 5. Build the prompt
    let file_text = String::from_utf8_lossy(&file_data);
    let ext = s3_key.rsplit('.').next().unwrap_or("unknown");
    let format_instruction = match payload.output_format.as_str() {
        "table" => "Extract all tabular data from this content. Output as a Markdown table with headers and rows. If there are multiple tables, include all of them with section headers.",
        _ => "Extract the full content from this document. Output as clean, well-structured Markdown with headings, paragraphs, and lists as appropriate. Preserve the original structure and meaning.",
    };

    let prompt = format!(
        "{format_instruction}\n\n\
         The file is a .{ext} file.\n\n\
         --- FILE CONTENT ---\n{file_text}\n--- END ---"
    );

    // 6. Call the LLM API (with usage logging)
    let provider_str = model_config.as_ref().map(|m| m.provider.as_str()).unwrap_or("unknown");
    let (content, tokens_used) = call_llm_api_with_logging(
        &api_key, &api_base, &payload.model, &prompt,
        Some(&pool), Some(&tenant_id), Some(provider_str), Some("extract_with_ai"),
    ).await
        .map_err(|e| {
            error!("LLM API call failed: {}", e);
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("LLM extraction failed: {}", e)})))
        })?;

    info!("AI extraction completed for source {}: {} chars, {} tokens used", id, content.len(), tokens_used);

    Ok(Json(AiExtractResponse {
        content,
        tokens_used,
        model: payload.model,
    }))
}

// ─── LLM Credential Resolution ────────────────────────────────────────────────

/// Resolve API key and base URL for the given model.
pub fn resolve_llm_credentials(
    config: &Config,
    model_config: &Option<mimir_core_ai::models::model_config::ModelConfig>,
    model_id: &str,
) -> Result<(String, String), (StatusCode, Json<Value>)> {
    // Try to get API key from model metadata first
    if let Some(mc) = model_config {
        let api_key = mc.metadata.as_ref()
            .and_then(|m| m.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let api_base = mc.metadata.as_ref()
            .and_then(|m| m.get("api_base"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let Some(key) = api_key {
            let base = api_base.unwrap_or_else(|| infer_api_base(&mc.provider));
            return Ok((key, base));
        }
    }

    // Fall back to env-based credentials
    let provider = model_config.as_ref().map(|m| m.provider.as_str()).unwrap_or("");
    match provider {
        "gemini" | "google" => {
            let key = config.gemini_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for Gemini. Set GEMINI_API_KEY or add api_key to model metadata."
                })))
            })?;
            Ok((key, config.gemini_base_url.clone()))
        }
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for OpenAI. Set OPENAI_API_KEY or add api_key to model metadata."
                })))
            })?;
            Ok((key, "https://api.openai.com/v1/".to_string()))
        }
        "ollama" => {
            // Ollama doesn't need an API key
            Ok(("ollama".to_string(), format!("{}/v1/", config.ollama_url)))
        }
        "heimdall" => {
            let key = config.heimdall_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for Heimdall. Set HEIMDALL_API_KEY or add api_key to model metadata."
                })))
            })?;
            let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
            Ok((key, base))
        }
        _ => {
            // Try model_id to infer provider
            if model_id.starts_with("gpt-") || model_id.starts_with("o1-") || model_id.starts_with("o3-") {
                let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": "No API key for OpenAI model"})))
                })?;
                Ok((key, "https://api.openai.com/v1/".to_string()))
            } else if model_id.starts_with("gemini-") {
                let key = config.gemini_api_key.clone().ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": "No API key for Gemini model"})))
                })?;
                Ok((key, config.gemini_base_url.clone()))
            } else if model_id.starts_with("mlx-community/") || model_id.starts_with("lmstudio-community/") {
                // MLX/lmstudio models → Heimdall gateway
                let key = config.heimdall_api_key.clone().ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, Json(json!({"error": "No API key for Heimdall (inferred from mlx model)"})))
                })?;
                let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
                Ok((key, base))
            } else {
                // Default to Ollama for unknown models
                Ok(("ollama".to_string(), format!("{}/v1/", config.ollama_url)))
            }
        }
    }
}

/// Infer API base URL from provider name.
pub fn infer_api_base(provider: &str) -> String {
    match provider {
        "openai" => "https://api.openai.com/v1/".to_string(),
        "gemini" | "google" => "https://generativelanguage.googleapis.com/v1beta/openai/".to_string(),
        "ollama" => {
            let base = std::env::var("OLLAMA_URL")
                .or_else(|_| std::env::var("OLLAMA_API_URL"))
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            format!("{}/v1/", base.trim_end_matches('/'))
        }
        "heimdall" => {
            std::env::var("HEIMDALL_API_URL")
                .unwrap_or_else(|_| "http://192.168.1.133:3000/v1".to_string())
                + "/"
        }
        _ => {
            // Default: try OLLAMA_URL env, then localhost
            let base = std::env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            format!("{}/v1/", base.trim_end_matches('/'))
        }
    }
}

/// Call an OpenAI-compatible chat completions API.
/// If `pool` and `tenant_id` are provided, automatically logs usage to `llm_usage_logs`.
pub async fn call_llm_api(
    api_key: &str,
    api_base: &str,
    model: &str,
    prompt: &str,
) -> anyhow::Result<(String, u32)> {
    call_llm_api_with_logging(api_key, api_base, model, prompt, None, None, None, None).await
}

/// Internal LLM call with optional usage logging and daily token limit enforcement.
pub async fn call_llm_api_with_logging(
    api_key: &str,
    api_base: &str,
    model: &str,
    prompt: &str,
    pool: Option<&DbPool>,
    tenant_id: Option<&str>,
    provider: Option<&str>,
    caller: Option<&str>,
) -> anyhow::Result<(String, u32)> {
    // ─── Daily Token Limit Check ────────────────────────────────────────
    if let (Some(p), Some(tid)) = (pool, tenant_id) {
        let limit: Option<(i64,)> = sqlx::query_as(
            "SELECT max_daily_tokens FROM tenant_configs WHERE tenant_id = ?"
        )
        .bind(tid)
        .fetch_optional(p)
        .await
        .unwrap_or(None);

        if let Some((max_tokens,)) = limit {
            if max_tokens > 0 {
                let today_usage: (i64,) = sqlx::query_as(
                    "SELECT COALESCE(SUM(total_tokens), 0) FROM llm_usage_logs WHERE tenant_id = ? AND DATE(created_at) = CURDATE()"
                )
                .bind(tid)
                .fetch_one(p)
                .await
                .unwrap_or((0,));

                if today_usage.0 >= max_tokens {
                    return Err(anyhow::anyhow!(
                        "Daily token limit exceeded: used {}/{} tokens today",
                        today_usage.0, max_tokens
                    ));
                }
            }
        }
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let url = format!("{}chat/completions", api_base);

    let mut body = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a document extraction assistant. Extract content accurately and completely. Do not add commentary or explanation — only output the extracted content."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "max_tokens": 16000,
        "temperature": 0.1
    });

    // Only add chat_template_kwargs for local models (Heimdall/Ollama)
    // Gemini and OpenAI APIs reject unknown fields
    let prov = provider.unwrap_or("unknown");
    if prov != "gemini" && prov != "openai" {
        body["chat_template_kwargs"] = json!({ "enable_thinking": false });
    }

    info!("Calling LLM API: {} with model {}", url, model);

    let response = client.post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP request to LLM failed: {}", e));

    let latency_ms = start.elapsed().as_millis() as i32;

    // Handle HTTP error
    let response = match response {
        Ok(r) => r,
        Err(e) => {
            // Log error if pool is available
            if let (Some(p), Some(tid)) = (pool, tenant_id) {
                let _ = crate::routes::llm_usage::insert_llm_usage_log(
                    p, tid, model, provider.unwrap_or("unknown"), Some(&url), caller,
                    0, 0, 0, latency_ms, "error", Some(&e.to_string()),
                ).await;
            }
            return Err(e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        // Log error
        if let (Some(p), Some(tid)) = (pool, tenant_id) {
            let _ = crate::routes::llm_usage::insert_llm_usage_log(
                p, tid, model, provider.unwrap_or("unknown"), Some(&url), caller,
                0, 0, 0, latency_ms, "error", Some(&error_body),
            ).await;
        }
        return Err(anyhow::anyhow!("LLM API returned {}: {}", status, error_body));
    }

    let resp_json: Value = response.json().await
        .map_err(|e| anyhow::anyhow!("Failed to parse LLM response: {}", e))?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Qwen3.5 thinking mode: if content is empty, check the reasoning field
    let content = if content.is_empty() {
        resp_json["choices"][0]["message"]["reasoning"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else {
        content
    };

    let input_tokens = resp_json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as i32;
    let output_tokens = resp_json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as i32;
    let total_tokens = resp_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as i32;

    // Log success
    if let (Some(p), Some(tid)) = (pool, tenant_id) {
        let _ = crate::routes::llm_usage::insert_llm_usage_log(
            p, tid, model, provider.unwrap_or("unknown"), Some(&url), caller,
            input_tokens, output_tokens, total_tokens, latency_ms, "success", None,
        ).await;
    }

    Ok((content, total_tokens as u32))
}

// ─── TDD Tests for LLM Credential Resolution (#182) ───────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use mimir_core_ai::models::model_config::ModelConfig;

    /// Helper: create a minimal Config with Heimdall credentials set.
    fn test_config_with_heimdall() -> Config {
        Config {
            port: 3000,
            mariadb_url: String::new(),
            qdrant_url: String::new(),
            redis_url: String::new(),
            s3_endpoint: String::new(),
            s3_bucket: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            s3_region: String::new(),
            ollama_url: "http://localhost:11434".to_string(),
            local_model: String::new(),
            embed_model: String::new(),
            gemini_base_url: "https://generativelanguage.googleapis.com/v1beta/openai/".to_string(),
            gemini_api_key: Some("test-gemini-key".to_string()),
            gemini_model: String::new(),
            heimdall_api_url: "http://192.168.1.133:3000/v1".to_string(),
            heimdall_api_key: Some("test-heimdall-key".to_string()),
            heimdall_model: "mlx-community/Qwen3.5-35B-A3B-4bit".to_string(),
            neo4j_uri: "bolt://localhost:7687".to_string(),
            neo4j_user: "neo4j".to_string(),
            neo4j_password: "test_password".to_string(),
            jwt_secret: String::new(),
        }
    }

    /// Helper: create a minimal Config without Heimdall API key.
    fn test_config_no_heimdall_key() -> Config {
        let mut cfg = test_config_with_heimdall();
        cfg.heimdall_api_key = None;
        cfg
    }

    fn make_model_config(provider: &str) -> Option<ModelConfig> {
        Some(ModelConfig {
            model_id: "test-model".to_string(),
            provider: provider.to_string(),
            model_type: "chat".to_string(),
            is_active: true,
            capabilities: None,
            metadata: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    // ─── infer_api_base tests ──────────────────────────────────────────

    #[test]
    fn test_infer_api_base_heimdall() {
        // Set env for test
        unsafe { std::env::set_var("HEIMDALL_API_URL", "http://192.168.1.133:3000/v1"); }
        let base = infer_api_base("heimdall");
        assert!(base.contains("192.168.1.133"), "Heimdall base should contain gateway host, got: {}", base);
        assert!(base.ends_with('/'), "Base URL should end with /");
    }

    #[test]
    fn test_infer_api_base_openai() {
        let base = infer_api_base("openai");
        assert_eq!(base, "https://api.openai.com/v1/");
    }

    #[test]
    fn test_infer_api_base_gemini() {
        let base = infer_api_base("gemini");
        assert!(base.contains("generativelanguage.googleapis.com"));
    }

    #[test]
    fn test_infer_api_base_ollama() {
        let base = infer_api_base("ollama");
        assert_eq!(base, "http://localhost:11434/v1/");
    }

    #[test]
    fn test_infer_api_base_unknown_defaults_to_ollama() {
        let base = infer_api_base("some_unknown_provider");
        assert_eq!(base, "http://localhost:11434/v1/");
    }

    // ─── resolve_llm_credentials tests ─────────────────────────────────

    #[test]
    fn test_resolve_heimdall_provider_explicit() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("heimdall");
        let result = resolve_llm_credentials(&config, &mc, "mlx-community/Qwen3.5-9B-MLX-4bit");
        assert!(result.is_ok(), "Should resolve Heimdall credentials");
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
        assert!(base.contains("192.168.1.133"), "Base should be Heimdall URL, got: {}", base);
    }

    #[test]
    fn test_resolve_heimdall_via_mlx_prefix() {
        let config = test_config_with_heimdall();
        // No model_config (None) — should infer from model_id prefix
        let result = resolve_llm_credentials(&config, &None, "mlx-community/Qwen3.5-35B-A3B-4bit");
        assert!(result.is_ok(), "Should infer Heimdall from mlx-community/ prefix");
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
        assert!(base.contains("192.168.1.133:3000"));
    }

    #[test]
    fn test_resolve_heimdall_via_lmstudio_prefix() {
        let config = test_config_with_heimdall();
        let result = resolve_llm_credentials(&config, &None, "lmstudio-community/medgemma-4b-it-MLX-4bit");
        assert!(result.is_ok(), "Should infer Heimdall from lmstudio-community/ prefix");
        let (key, _) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
    }

    #[test]
    fn test_resolve_heimdall_missing_key_returns_error() {
        let config = test_config_no_heimdall_key();
        let mc = make_model_config("heimdall");
        let result = resolve_llm_credentials(&config, &mc, "some-model");
        assert!(result.is_err(), "Should return error when Heimdall API key is missing");
    }

    #[test]
    fn test_resolve_ollama_still_works() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("ollama");
        let result = resolve_llm_credentials(&config, &mc, "llama3.2");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "ollama");
        assert!(base.contains("11434"));
    }

    #[test]
    fn test_resolve_gemini_still_works() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("gemini");
        let result = resolve_llm_credentials(&config, &mc, "gemini-2.5-flash");
        assert!(result.is_ok());
        let (key, _) = result.unwrap();
        assert_eq!(key, "test-gemini-key");
    }

    #[test]
    fn test_resolve_unknown_model_defaults_to_ollama() {
        let config = test_config_with_heimdall();
        let result = resolve_llm_credentials(&config, &None, "some-random-model");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "ollama");
        assert!(base.contains("11434"));
    }
}
