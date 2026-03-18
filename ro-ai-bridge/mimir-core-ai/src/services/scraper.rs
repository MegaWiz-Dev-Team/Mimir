//! Web scraper service — calls Ratatoskr shared browser service.
//!
//! Replaces previous chromiumoxide implementation.
//! Ratatoskr provides headless Chromium via REST API.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Request body for Ratatoskr scrape API.
#[derive(Serialize)]
struct ScrapeRequest {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    wait_selector: Option<String>,
    scroll: bool,
    extract_text: bool,
}

/// Response from Ratatoskr scrape API.
#[derive(Deserialize)]
struct ScrapeResponse {
    #[allow(dead_code)]
    url: String,
    html: String,
    text: Option<String>,
    title: Option<String>,
}

pub struct ScraperService {
    client: reqwest::Client,
    ratatoskr_url: String,
}

impl ScraperService {
    /// Create a new scraper backed by Ratatoskr.
    pub async fn new() -> Result<Self> {
        let ratatoskr_url = std::env::var("RATATOSKR_URL")
            .unwrap_or_else(|_| "http://ratatoskr:9200".to_string());

        info!("🐿️ ScraperService → Ratatoskr at {}", ratatoskr_url);

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
            ratatoskr_url,
        })
    }

    /// Scrape a URL with optional wait selector and scroll behavior.
    ///
    /// API-compatible with the old chromiumoxide ScraperService.
    pub async fn scrape_url(
        &self,
        url: &str,
        wait_selector: Option<&str>,
        scroll: bool,
    ) -> Result<String> {
        info!("Scraping via Ratatoskr: {}", url);

        let resp = self
            .client
            .post(format!("{}/api/v1/scrape", self.ratatoskr_url))
            .json(&ScrapeRequest {
                url: url.to_string(),
                wait_selector: wait_selector.map(|s| s.to_string()),
                scroll,
                extract_text: false,
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("Ratatoskr scrape failed ({}): {}", status, body);
            anyhow::bail!("Scrape failed with status {}: {}", status, body);
        }

        let result: ScrapeResponse = resp.json().await?;
        Ok(result.html)
    }
}
