//! Mimir HTTP ingest client.
//!
//! POSTs each chunk into `POST {mimir_url}/api/v1/tenants/{tenant_id}/ingest`
//! with body `{title, content, source}` — matches `ro-ai-bridge/src/routes/ingest.rs`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::types::Chunk;

#[derive(Debug, Serialize)]
struct IngestRequest<'a> {
    title: &'a str,
    content: &'a str,
    source: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct IngestResponse {
    #[serde(default)]
    document_id: i64,
    #[serde(default)]
    #[allow(dead_code)]
    status: String,
    #[serde(default)]
    tree_node_count: i64,
}

pub async fn run(input: &Path, mimir_url: &str, tenant_id: &str) -> Result<()> {
    let chunks = read_chunks(input)?;
    info!("Read {} chunks from {}", chunks.len(), input.display());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let endpoint = format!("{}/api/v1/tenants/{}/ingest", mimir_url.trim_end_matches('/'), tenant_id);
    info!("Ingesting to {}", endpoint);

    let mut ok = 0usize;
    let mut failed = 0usize;

    for (i, chunk) in chunks.iter().enumerate() {
        let title = chunk.title();
        let source = if chunk.source_url.is_empty() {
            None
        } else {
            Some(chunk.source_url.as_str())
        };
        let body = IngestRequest {
            title: &title,
            content: &chunk.content,
            source,
        };

        let res = client.post(&endpoint).json(&body).send().await;
        match res {
            Ok(r) if r.status().is_success() => {
                let body: IngestResponse = r.json().await.unwrap_or(IngestResponse {
                    document_id: 0,
                    status: "ok".into(),
                    tree_node_count: 0,
                });
                ok += 1;
                info!(
                    "  [{}/{}] ✅ {} → doc_id={} nodes={}",
                    i + 1,
                    chunks.len(),
                    title,
                    body.document_id,
                    body.tree_node_count
                );
            }
            Ok(r) => {
                failed += 1;
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                warn!("  [{}/{}] ❌ {} — HTTP {}: {}", i + 1, chunks.len(), title, status, truncate(&text, 200));
            }
            Err(e) => {
                failed += 1;
                warn!("  [{}/{}] ❌ {} — {}", i + 1, chunks.len(), title, e);
            }
        }
    }

    info!("Ingest complete: {} ok, {} failed", ok, failed);
    if failed > 0 && ok == 0 {
        anyhow::bail!("All {} ingest requests failed", failed);
    }
    Ok(())
}

pub fn read_chunks(input: &Path) -> Result<Vec<Chunk>> {
    let text = std::fs::read_to_string(input)
        .with_context(|| format!("read {}", input.display()))?;
    let mut chunks = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let c: Chunk = serde_json::from_str(line)
            .with_context(|| format!("parse line {} of {}", lineno + 1, input.display()))?;
        chunks.push(c);
    }
    Ok(chunks)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn read_chunks_handles_blank_lines() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "{}", r#"{"chunk_id":"c1","content":"hi","source_url":"https://x"}"#).unwrap();
        writeln!(f).unwrap();
        writeln!(f, "{}", r#"{"chunk_id":"c2","content":"bye","source_url":"https://y"}"#).unwrap();
        let chunks = read_chunks(f.path()).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chunk_id, "c1");
        assert_eq!(chunks[1].chunk_id, "c2");
    }

    #[test]
    fn truncate_caps_long_strings() {
        let s = "a".repeat(500);
        let t = truncate(&s, 100);
        assert_eq!(t.chars().count(), 101); // 100 'a' + 1 ellipsis
    }
}
