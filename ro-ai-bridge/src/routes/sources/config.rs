//! LLM configuration, credential resolution, and AI extraction.
//!
//! Public items (used by other route modules):
//! - `resolve_llm_credentials` — used by agents.rs, ocr.rs
//! - `infer_api_base` — used by agents.rs
//! - `call_llm_api` / `call_llm_api_with_logging` — used by ocr.rs

use crate::config::Config;
use crate::routes::tenant::extract_tenant_id;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use mimir_core_ai::models::sources::DataSource;
use mimir_core_ai::services::db;
use mimir_core_ai::services::db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

// ─── LLM Fallback Extraction ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AiExtractRequest {
    model: String,
    output_format: String, // "markdown" | "table"
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
    let source = sqlx::query_as::<_, DataSource>(
        "SELECT * FROM data_sources WHERE id = ? AND tenant_id = ?",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let source = source.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Source not found"})),
        )
    })?;

    // 2. Download file from S3
    let s3_key = source.s3_key.as_deref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Source has no S3 file — nothing to extract"})),
        )
    })?;

    let file_data = super::upload::download_from_s3(&config, s3_key)
        .await
        .map_err(|e| {
            error!("S3 download failed for AI extraction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to download file: {}", e)})),
            )
        })?;

    info!(
        "Downloaded {} bytes from S3 for AI extraction (source_id={})",
        file_data.len(),
        id
    );

    // 3. Look up model configuration from DB
    let model_config = db::get_model_by_id(&pool, &payload.model)
        .await
        .map_err(|e| {
            error!("Failed to look up model {}: {}", payload.model, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Model lookup failed: {}", e)})),
            )
        })?;

    // 4. Determine API key and endpoint from model config or env
    let (api_key, api_base) = resolve_llm_credentials(&config, &model_config, &payload.model)?;

    // 5. Build the prompt or use Multimodal OCR
    let ext = s3_key.rsplit('.').next().unwrap_or("unknown").to_lowercase();
    let format_instruction = match payload.output_format.as_str() {
        "table" => "Extract all tabular data from this content. Output as a Markdown table with headers and rows. If there are multiple tables, include all of them with section headers.",
        _ => "Extract the full content from this document. Output as clean, well-structured Markdown with headings, paragraphs, and lists as appropriate. Preserve the original structure and meaning.",
    };

    let provider_str = model_config
        .as_ref()
        .map(|m| m.provider.as_str())
        .unwrap_or("unknown");

    let (content, tokens_used) = if mimir_core_ai::services::ocr::is_ocr_capable(&ext)
        && (provider_str == "gemini" || provider_str == "google")
    {
        info!("Using Gemini Vision OCR for {} with model {}", s3_key, payload.model);
        mimir_core_ai::services::ocr::extract_text_from_image(
            &file_data,
            s3_key,
            &api_key,
            &api_base,
            &payload.model,
        )
        .await
        .map_err(|e| {
            error!("OCR Vision extraction failed: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("OCR extraction failed: {}", e)})),
            )
        })?
    } else {
        let file_text = String::from_utf8_lossy(&file_data);
        let prompt = format!(
            "{format_instruction}\n\n\
             The file is a .{ext} file.\n\n\
             --- FILE CONTENT ---\n{file_text}\n--- END ---"
        );

        call_llm_api_with_logging(
            &api_key,
            &api_base,
            &payload.model,
            &prompt,
            Some(&pool),
            Some(&tenant_id),
            Some(provider_str),
            Some("extract_with_ai"),
        )
        .await
        .map_err(|e| {
            error!("LLM API call failed: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("LLM extraction failed: {}", e)})),
            )
        })?
    };

    info!(
        "AI extraction completed for source {}: {} chars, {} tokens used",
        id,
        content.len(),
        tokens_used
    );

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
    _model_id: &str,
) -> Result<(String, String), (StatusCode, Json<Value>)> {
    // Try to get API key from model metadata first
    if let Some(mc) = model_config {
        let api_key = mc
            .metadata
            .as_ref()
            .and_then(|m| m.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let api_base = mc
            .metadata
            .as_ref()
            .and_then(|m| m.get("api_base"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let Some(key) = api_key {
            let base = api_base.unwrap_or_else(|| infer_api_base(&mc.provider));
            return Ok((key, base));
        }
    }

    // Fall back to env-based credentials
    let provider = model_config
        .as_ref()
        .map(|m| m.provider.as_str())
        .unwrap_or("");
    match provider {
        "gemini" | "google" => {
            let key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
            if key.is_empty() {
                return Err((StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for Gemini. Set GEMINI_API_KEY in environment constraints."
                }))));
            }
            let base = std::env::var("GEMINI_BASE_URL")
                .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta/openai/".to_string());
            Ok((key, format!("{}/", base.trim_end_matches('/'))))
        }
        "openai" => {
            // Route through Heimdall with prefix-based routing
            let key = config.heimdall_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured. Set HEIMDALL_API_KEY. OpenAI models are routed through Heimdall Gateway."
                })))
            })?;
            let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
            Ok((key, base))
        }
        "heimdall" | "ollama" | "local" => {
            let key = config.heimdall_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "No API key configured for Heimdall. Set HEIMDALL_API_KEY or add api_key to model metadata."
                })))
            })?;
            let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
            Ok((key, base))
        }
        _ => {
            // All unknown providers route through Heimdall Gateway
            let key = config.heimdall_api_key.clone().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(json!({
                    "error": format!("No API key configured. Set HEIMDALL_API_KEY. Provider '{}' routes through Heimdall Gateway.", provider)
                })))
            })?;
            let base = format!("{}/", config.heimdall_api_url.trim_end_matches('/'));
            Ok((key, base))
        }
    }
}

/// Infer API base URL from provider name.
/// All providers now route through Heimdall Gateway.
pub fn infer_api_base(provider: &str) -> String {
    match provider.to_lowercase().as_str() {
        // All providers route through Heimdall Gateway prefix-based routing
        _ => {
            let base = std::env::var("HEIMDALL_API_URL")
                .unwrap_or_else(|_| "http://localhost:3000/v1".to_string());
            format!("{}/", base.trim_end_matches('/'))
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
        let limit: Option<(i64,)> =
            sqlx::query_as("SELECT max_daily_tokens FROM tenant_configs WHERE tenant_id = ?")
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
                        today_usage.0,
                        max_tokens
                    ));
                }
            }
        }
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_default();
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

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
    // Gemini/Google and OpenAI APIs reject unknown fields
    let prov = provider.unwrap_or("unknown");
    if prov != "gemini" && prov != "google" && prov != "openai" {
        body["chat_template_kwargs"] = json!({ "enable_thinking": false });
    }

    info!("Calling LLM API: {} with model {}", url, model);

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key.trim()))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP request to LLM failed (url={:?}, key_len={}): {:?}", url, api_key.len(), e));

    let latency_ms = start.elapsed().as_millis() as i32;

    // Handle HTTP error
    let response = match response {
        Ok(r) => r,
        Err(e) => {
            // Log error if pool is available
            if let (Some(p), Some(tid)) = (pool, tenant_id) {
                let _ = crate::routes::llm_usage::insert_llm_usage_log(
                    p,
                    tid,
                    model,
                    provider.unwrap_or("unknown"),
                    Some(&url),
                    caller,
                    0,
                    0,
                    0,
                    latency_ms,
                    "error",
                    Some(&e.to_string()),
                )
                .await;
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
                p,
                tid,
                model,
                provider.unwrap_or("unknown"),
                Some(&url),
                caller,
                0,
                0,
                0,
                latency_ms,
                "error",
                Some(&error_body),
            )
            .await;
        }
        return Err(anyhow::anyhow!(
            "LLM API returned {}: {}",
            status,
            error_body
        ));
    }

    let resp_json: Value = response
        .json()
        .await
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
    let output_tokens = resp_json["usage"]["completion_tokens"]
        .as_u64()
        .unwrap_or(0) as i32;
    let total_tokens = resp_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as i32;

    // Log success
    if let (Some(p), Some(tid)) = (pool, tenant_id) {
        let _ = crate::routes::llm_usage::insert_llm_usage_log(
            p,
            tid,
            model,
            provider.unwrap_or("unknown"),
            Some(&url),
            caller,
            input_tokens,
            output_tokens,
            total_tokens,
            latency_ms,
            "success",
            None,
        )
        .await;
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
            heimdall_api_url: "http://localhost:3000/v1".to_string(),
            heimdall_api_key: Some("test-heimdall-key".to_string()),
            heimdall_model: "mlx-community/Qwen3.5-35B-A3B-4bit".to_string(),
            jwt_secret: String::new(),
            ollama_url: String::new(),
            local_model: String::new(),
            embed_model: String::new(),
            gemini_base_url: String::new(),
            gemini_api_key: None,
            gemini_model: String::new(),
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
    fn test_infer_api_base_all_providers_route_through_heimdall() {
        // Set env for test
        unsafe {
            std::env::set_var("HEIMDALL_API_URL", "http://heimdall.example.com:3000/v1");
        }
        for provider in &["heimdall", "openai", "gemini", "google", "ollama", "unknown"] {
            let base = infer_api_base(provider);
            assert!(
                base.contains("heimdall.example.com"),
                "Provider '{}' should route through Heimdall, got: {}",
                provider, base
            );
            assert!(base.ends_with('/'), "Base URL should end with /");
        }
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
        assert!(
            base.contains("localhost:3000"),
            "Base should be Heimdall URL, got: {}",
            base
        );
    }

    #[test]
    fn test_resolve_gemini_routes_direct() {
        unsafe {
            std::env::set_var("GEMINI_API_KEY", "test-gemini-key");
            std::env::set_var("GEMINI_BASE_URL", "https://test.google.com/openai/");
        }
        let config = test_config_with_heimdall();
        let mc = make_model_config("gemini");
        let result = resolve_llm_credentials(&config, &mc, "gemini-2.5-flash");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-gemini-key", "Gemini should use Gemini key");
        assert!(base.contains("test.google.com"), "Should route directly to Google");
    }

    #[test]
    fn test_resolve_ollama_routes_through_heimdall() {
        let config = test_config_with_heimdall();
        let mc = make_model_config("ollama");
        let result = resolve_llm_credentials(&config, &mc, "llama3.2");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-heimdall-key", "Ollama should now use Heimdall key");
        assert!(base.contains("localhost:3000"), "Should route through Heimdall");
    }

    #[test]
    fn test_resolve_unknown_provider_routes_through_heimdall() {
        let config = test_config_with_heimdall();
        let result = resolve_llm_credentials(&config, &None, "some-random-model");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "test-heimdall-key");
        assert!(base.contains("localhost:3000"));
    }

    #[test]
    fn test_resolve_heimdall_missing_key_returns_error() {
        let config = test_config_no_heimdall_key();
        let mc = make_model_config("heimdall");
        let result = resolve_llm_credentials(&config, &mc, "some-model");
        assert!(
            result.is_err(),
            "Should return error when Heimdall API key is missing"
        );
    }

    #[test]
    fn test_resolve_model_metadata_api_key_takes_precedence() {
        let config = test_config_with_heimdall();
        let mc = Some(ModelConfig {
            model_id: "custom-model".to_string(),
            provider: "heimdall".to_string(),
            model_type: "chat".to_string(),
            is_active: true,
            capabilities: None,
            metadata: Some(serde_json::json!({
                "api_key": "custom-key-from-metadata",
                "api_base": "http://custom.endpoint/v1"
            })),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        });
        let result = resolve_llm_credentials(&config, &mc, "custom-model");
        assert!(result.is_ok());
        let (key, base) = result.unwrap();
        assert_eq!(key, "custom-key-from-metadata");
        assert_eq!(base, "http://custom.endpoint/v1");
    }
}
