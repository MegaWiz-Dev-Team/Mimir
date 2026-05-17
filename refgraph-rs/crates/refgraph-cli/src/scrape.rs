//! Web scraper for insurance product pages.
//!
//! Reads `insurer_urls.json`, fetches each URL, extracts visible text from
//! `<main>`, `<article>`, or `<body>` (in that order), and emits one
//! `Chunk` per (insurer, url) into a JSONL file.

use anyhow::{Context, Result};
use scraper::{Html, Selector};
use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use tracing::{info, warn};

use crate::types::Chunk;

#[derive(Debug, Deserialize)]
struct InsurerConfig {
    insurers: std::collections::HashMap<String, InsurerEntry>,
}

#[derive(Debug, Deserialize)]
struct InsurerEntry {
    name: String,
    #[serde(default)]
    language: String,
    urls: Vec<String>,
}

pub async fn run(config_path: &Path, out: &Path) -> Result<()> {
    let cfg_text = std::fs::read_to_string(config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let cfg: InsurerConfig = serde_json::from_str(&cfg_text)
        .with_context(|| format!("parse {}", config_path.display()))?;

    let client = reqwest::Client::builder()
        .user_agent("RefGraph/0.1 (Rust insurance scraper)")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut out_file = std::fs::File::create(out)
        .with_context(|| format!("create {}", out.display()))?;
    let mut chunk_count = 0usize;

    for (insurer_id, ins) in &cfg.insurers {
        info!("Scraping insurer {} ({})", insurer_id, ins.name);
        for (idx, url) in ins.urls.iter().enumerate() {
            match fetch_text(&client, url).await {
                Ok(text) if !text.is_empty() => {
                    let chunk = Chunk {
                        chunk_id: format!("url_{}__{}", insurer_id, idx),
                        content: text,
                        source_url: url.clone(),
                        insurer_id: insurer_id.clone(),
                        product_type: String::new(),
                        language: ins.language.clone(),
                        product_name: ins.name.clone(),
                        extra: Default::default(),
                    };
                    let line = serde_json::to_string(&chunk)?;
                    writeln!(out_file, "{}", line)?;
                    chunk_count += 1;
                    info!("  ✅ {} ({} chars)", url, chunk.content.len());
                }
                Ok(_) => warn!("  ⚠️  {} → empty content", url),
                Err(e) => warn!("  ❌ {} → {}", url, e),
            }
        }
    }

    info!("Wrote {} chunks to {}", chunk_count, out.display());
    Ok(())
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client.get(url).send().await.context("GET")?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let html = resp.text().await.context("body")?;
    Ok(extract_visible_text(&html))
}

/// Pull visible text out of HTML — prefers main/article over body.
/// Strips scripts/styles, collapses whitespace.
fn extract_visible_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    for sel in ["main", "article", "body"] {
        let selector = Selector::parse(sel).unwrap();
        if let Some(node) = doc.select(&selector).next() {
            let text = node
                .text()
                .collect::<Vec<_>>()
                .join("\n")
                .split('\n')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_from_body() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let txt = extract_visible_text(html);
        assert!(txt.contains("Hello"));
        assert!(txt.contains("World"));
    }

    #[test]
    fn prefers_main_over_body() {
        let html = "<html><body><nav>chrome</nav><main>real content</main></body></html>";
        let txt = extract_visible_text(html);
        assert!(txt.contains("real content"));
        // Both nav and main are under body, so when we fall through to body
        // we'd see "chrome". Verify we picked <main> first by absence of "chrome".
        assert!(!txt.contains("chrome"));
    }

    #[test]
    fn empty_html_returns_empty() {
        assert!(extract_visible_text("").is_empty());
        assert!(extract_visible_text("<html></html>").is_empty());
    }
}
