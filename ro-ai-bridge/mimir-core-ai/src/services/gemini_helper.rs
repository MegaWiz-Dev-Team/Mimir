//! Shared Gemini API helper.
//!
//! Single source of truth for:
//!   - Base URL  (env: `GEMINI_BASE_URL`, default `https://generativelanguage.googleapis.com/v1beta`)
//!   - API key   (env: `GEMINI_API_KEY`)
//!   - Default model fallbacks (centralized constants)
//!
//! Used by: evaluation/runner.rs (judge), routes/insights.rs (analysis),
//! routes/auto_tune.rs (parameter tuning).

use anyhow::{Context, Result};
use serde_json::Value;
use std::time::Duration;

/// Default model for LLM-as-judge scoring (cheap, fast).
pub const DEFAULT_JUDGE_MODEL: &str = "gemini-2.5-flash";

/// Default model for analysis/insights (cheap, structured output).
pub const DEFAULT_INSIGHT_MODEL: &str = "gemini-2.5-flash";

/// Default model for hypothesis generation + auto-tuning (more capable, slower).
pub const DEFAULT_AUTO_TUNE_MODEL: &str = "gemini-3.1-pro-preview";

/// Read the **native** Gemini API base URL.
///
/// This helper uses the native `/v1beta/models/...:generateContent` schema, NOT
/// the OpenAI-compat `/v1beta/openai/v1/chat/completions` schema. If
/// `GEMINI_BASE_URL` points to the OpenAI-compat path (used by Bifrost), we
/// strip the trailing `/openai` so we land on the native root.
pub fn base_url() -> String {
    let raw = std::env::var("GEMINI_BASE_URL")
        .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".to_string());
    let trimmed = raw.trim_end_matches('/');
    // If env var pointed at the OpenAI-compat sub-path, strip it.
    let native = trimmed.strip_suffix("/openai").unwrap_or(trimmed);
    native.to_string()
}

/// Read the Gemini API key from env (caller is expected to error gracefully if missing).
pub fn api_key() -> Result<String> {
    std::env::var("GEMINI_API_KEY").context("GEMINI_API_KEY not set")
}

/// Build the full generateContent URL for a given model.
pub fn generate_content_url(model: &str) -> Result<String> {
    let key = api_key()?;
    Ok(format!("{}/models/{}:generateContent?key={}", base_url(), model, key))
}

/// Configuration for a Gemini call.
#[derive(Debug, Clone)]
pub struct GeminiCallConfig {
    pub temperature: f32,
    pub max_output_tokens: u32,
    /// If true, request `application/json` mime type (use when prompt asks for JSON output)
    pub force_json: bool,
    pub timeout_secs: u64,
}

impl Default for GeminiCallConfig {
    fn default() -> Self {
        Self { temperature: 0.2, max_output_tokens: 2048, force_json: false, timeout_secs: 60 }
    }
}

/// Result of a Gemini call.
#[derive(Debug)]
pub struct GeminiCallResult {
    pub text: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// Call Gemini's generateContent with a single text prompt.
/// Returns (text, input_tokens, output_tokens).
pub async fn call_text(model: &str, prompt: &str, cfg: &GeminiCallConfig) -> Result<GeminiCallResult> {
    let mut gen_config = serde_json::json!({
        "temperature": cfg.temperature,
        "maxOutputTokens": cfg.max_output_tokens,
    });
    if cfg.force_json {
        gen_config["response_mime_type"] = serde_json::json!("application/json");
    }

    let body = serde_json::json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": gen_config,
    });

    let url = generate_content_url(model)?;
    let resp = reqwest::Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_secs))
        .build()?
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Gemini API request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Gemini {}: {}", status, &text[..text.len().min(400)]));
    }

    let json: Value = resp.json().await.context("Gemini response not JSON")?;
    let text = json["candidates"].get(0)
        .and_then(|c| c["content"]["parts"].get(0))
        .and_then(|p| p["text"].as_str())
        .unwrap_or("")
        .to_string();
    let input_tokens = json["usageMetadata"]["promptTokenCount"].as_i64().unwrap_or(0);
    let output_tokens = json["usageMetadata"]["candidatesTokenCount"].as_i64().unwrap_or(0);

    if text.is_empty() {
        let finish = json["candidates"].get(0)
            .and_then(|c| c["finishReason"].as_str())
            .unwrap_or("UNKNOWN");
        return Err(anyhow::anyhow!("Gemini returned empty text (finishReason={})", finish));
    }
    Ok(GeminiCallResult { text, input_tokens, output_tokens })
}

/// Extract a JSON object from Gemini's text output, tolerating
/// markdown code fences and partial-content noise.
pub fn extract_json_object(text: &str) -> Option<Value> {
    let cleaned = text.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let start = cleaned.find('{')?;
    let mut depth = 0i32;
    let mut end = cleaned.len();
    for (i, ch) in cleaned[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    serde_json::from_str(&cleaned[start..end]).ok()
}
