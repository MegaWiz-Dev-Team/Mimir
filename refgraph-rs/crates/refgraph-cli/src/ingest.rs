//! Direct-to-RAG ingest pipeline (bypasses buggy mimir-api embed-chunks).
//!
//! Flow:
//!   chunks JSONL → batch Heimdall /v1/embeddings (BGE-M3, 1024-dim)
//!                → Qdrant PUT /collections/source_chunks/points
//!
//! Why bypass mimir-api:
//!   `ro-ai-bridge/src/routes/vector.rs::embed_chunks` has two bugs in the
//!   currently deployed image:
//!     1. SQL filters by `chunks.tenant_id` — column does not exist; the
//!        `unwrap_or_default()` swallows the error and returns "No chunks
//!        found" even when chunks are present.
//!     2. `embed_texts()` declares `heimdall_url` but never calls Heimdall;
//!        it produces hash-based pseudo-vectors instead of real BGE-M3.
//!   Until those are fixed in a Mimir PR, the CLI does the embedding +
//!   upsert directly so /api/v1/search has *real* semantic vectors to query.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::heimdall::HeimdallClient;
use crate::qdrant::{payload, point_id, QdrantClient, QdrantPoint, VectorDense};
use crate::types::Chunk;

/// Configuration knobs for the ingest pipeline.
pub struct IngestConfig {
    pub heimdall_url: String,
    pub heimdall_api_key: String,
    pub heimdall_model: String,
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub tenant_id: String,
    /// Synthetic source_id base. Real Mimir-managed ids are auto-increment
    /// from 1, so we use 1_000_000_000+ to avoid collisions when /api/v1/search
    /// joins with data_sources (none of our points will match a Mimir source row).
    pub source_id_base: u64,
    /// Max characters fed to BGE-M3 per chunk. The model max is ~8192 tokens;
    /// 6000 chars ≈ 1500-2000 tokens conservative.
    pub max_chars_per_chunk: usize,
    /// Heimdall batch size — how many texts per /v1/embeddings call.
    pub embed_batch_size: usize,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            heimdall_url: "http://localhost:8080".into(),
            heimdall_api_key: String::new(),
            heimdall_model: "bge-m3".into(),
            qdrant_url: "http://localhost:6333".into(),
            qdrant_collection: "source_chunks".into(),
            tenant_id: "asgard_insurance".into(),
            source_id_base: 1_000_000_000,
            max_chars_per_chunk: 6000,
            embed_batch_size: 16,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestReport {
    pub chunks_read: usize,
    pub chunks_embedded: usize,
    pub points_upserted: usize,
    pub points_count_before: u64,
    pub points_count_after: u64,
}

pub async fn run(input: &Path, cfg: IngestConfig) -> Result<()> {
    let chunks = read_chunks(input)?;
    info!("Read {} chunks from {}", chunks.len(), input.display());

    let heimdall = HeimdallClient::new(
        cfg.heimdall_url.clone(),
        cfg.heimdall_api_key.clone(),
        cfg.heimdall_model.clone(),
    )?;
    let qdrant = QdrantClient::new(cfg.qdrant_url.clone())?;

    let before = qdrant.count(&cfg.qdrant_collection).await.unwrap_or(0);
    info!(
        "Qdrant collection '{}' point count before: {}",
        cfg.qdrant_collection, before
    );

    // Prepare texts (truncate to model limit).
    let texts: Vec<String> = chunks
        .iter()
        .map(|c| truncate_chars(&c.content, cfg.max_chars_per_chunk))
        .collect();

    // Batch embed.
    let mut all_vectors: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for (i, batch) in texts.chunks(cfg.embed_batch_size).enumerate() {
        info!(
            "Embedding batch {}/{} (size {})",
            i + 1,
            (texts.len() + cfg.embed_batch_size - 1) / cfg.embed_batch_size,
            batch.len()
        );
        let vecs = heimdall.embed(batch).await.context("Heimdall embed batch")?;
        all_vectors.extend(vecs);
    }
    assert_eq!(all_vectors.len(), chunks.len());

    // Build Qdrant points.
    let points: Vec<QdrantPoint> = chunks
        .iter()
        .enumerate()
        .zip(all_vectors.into_iter())
        .map(|((i, c), vec)| {
            let pid = point_id(cfg.source_id_base + i as u64, 0);
            QdrantPoint {
                id: pid,
                vector: VectorDense { dense: vec },
                payload: payload(
                    &c.content,
                    pid,
                    cfg.source_id_base + i as u64,
                    &cfg.tenant_id,
                ),
            }
        })
        .collect();

    let upserted = qdrant
        .upsert_points(&cfg.qdrant_collection, points)
        .await
        .context("Qdrant upsert")?;
    info!("Upserted {} points", upserted);

    let after = qdrant.count(&cfg.qdrant_collection).await.unwrap_or(0);
    let report = IngestReport {
        chunks_read: chunks.len(),
        chunks_embedded: all_vectors_len(&texts, cfg.embed_batch_size),
        points_upserted: upserted,
        points_count_before: before,
        points_count_after: after,
    };

    println!();
    println!("══════════════════════════════════════════════");
    println!("  Ingest Report (direct Heimdall + Qdrant)");
    println!("══════════════════════════════════════════════");
    println!("  Chunks read:        {}", report.chunks_read);
    println!("  Chunks embedded:    {}", report.chunks_embedded);
    println!("  Points upserted:    {}", report.points_upserted);
    println!(
        "  Qdrant points:      {} → {} (Δ {})",
        report.points_count_before,
        report.points_count_after,
        (report.points_count_after as i64) - (report.points_count_before as i64)
    );
    println!("  Tenant filter:      {}", cfg.tenant_id);
    println!("══════════════════════════════════════════════");

    if upserted == 0 {
        warn!("Zero points upserted — investigate Heimdall/Qdrant errors above");
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

fn all_vectors_len(texts: &[String], _batch_size: usize) -> usize {
    texts.len()
}

/// Truncate by *characters* (not bytes — Thai/UTF-8 must not be split mid-codepoint).
/// BGE-M3 tokens ≈ 0.25-0.4× chars for Thai, 0.25× for English.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
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
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate_chars("hello", 100), "hello");
    }

    #[test]
    fn truncate_caps_long_strings() {
        let s = "a".repeat(500);
        assert_eq!(truncate_chars(&s, 100).chars().count(), 100);
    }

    #[test]
    fn truncate_respects_utf8_boundaries() {
        // Thai chars are multi-byte — bytewise truncation would corrupt them.
        let s = "ประกันสุขภาพ".repeat(50); // 12 chars × 50 = 600 chars (~1800 bytes)
        let t = truncate_chars(&s, 10);
        assert_eq!(t.chars().count(), 10);
        // String must remain valid UTF-8 (panic if not, since we used chars()).
        let _ = t.as_bytes();
    }

    #[test]
    fn default_config_targets_localhost() {
        let cfg = IngestConfig::default();
        assert_eq!(cfg.heimdall_url, "http://localhost:8080");
        assert_eq!(cfg.qdrant_url, "http://localhost:6333");
        assert_eq!(cfg.tenant_id, "asgard_insurance");
        // source_id_base above the Mimir auto-increment range (currently in the
        // single digits) to avoid colliding with rows added by other paths.
        assert!(cfg.source_id_base >= 1_000_000_000);
    }
}
