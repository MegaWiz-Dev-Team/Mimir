//! Heimdall embeddings client.
//!
//! Calls `POST {heimdall_url}/v1/embeddings` (OpenAI-compatible) with
//! Bearer auth → returns 1024-dim BGE-M3 vectors.
//!
//! Heimdall expects API key in `Authorization: Bearer <key>`. The default
//! local key is the one in `~/Library/LaunchAgents/com.asgard.heimdall-gateway.plist`
//! (env `API_KEYS`). Pass it via `--heimdall-key` CLI flag or `HEIMDALL_API_KEY` env.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedItem>,
    #[serde(default)]
    #[allow(dead_code)]
    model: String,
}

#[derive(Debug, Deserialize)]
struct EmbedItem {
    embedding: Vec<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    index: usize,
}

pub struct HeimdallClient {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl HeimdallClient {
    pub fn new(base_url: String, api_key: String, model: String) -> Result<Self> {
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
        })
    }

    /// Embed a batch of texts. Returns one 1024-dim f32 vector per input.
    /// Heimdall responds in input order; we preserve that order.
    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let req = EmbedRequest {
            model: &self.model,
            input: texts,
        };
        let resp = self
            .client
            .post(format!("{}/v1/embeddings", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .context("POST /v1/embeddings")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Heimdall /v1/embeddings HTTP {}: {}",
                status,
                &body[..body.len().min(300)]
            );
        }
        let parsed: EmbedResponse = resp.json().await.context("parse embed response")?;
        if parsed.data.len() != texts.len() {
            anyhow::bail!(
                "Heimdall returned {} embeddings for {} inputs",
                parsed.data.len(),
                texts.len()
            );
        }
        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn embed_empty_input_returns_empty_without_request() {
        // Use bogus URL — should not be hit since empty input short-circuits.
        let client = HeimdallClient::new(
            "http://127.0.0.1:1".into(),
            "bogus".into(),
            "bge-m3".into(),
        )
        .unwrap();
        let result = client.embed(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn embed_propagates_http_errors_with_status() {
        let client = HeimdallClient::new(
            "http://127.0.0.1:1".into(), // unreachable — connection error
            "x".into(),
            "bge-m3".into(),
        )
        .unwrap();
        let err = client.embed(&["test".into()]).await.unwrap_err();
        // anyhow context message must mention the endpoint we tried
        assert!(format!("{:?}", err).contains("embeddings"));
    }
}
