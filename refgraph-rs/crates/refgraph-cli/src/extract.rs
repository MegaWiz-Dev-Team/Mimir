//! Entity extraction wrapper around refgraph-core.
//!
//! Reads chunks JSONL, converts to `refgraph_core::RawChunk`, runs the
//! extractor + deduplicator, emits entities JSONL.

use anyhow::{Context, Result};
use refgraph_core::types::RawChunk;
use refgraph_core::{Deduplicator, EntityExtractor};
use std::io::Write;
use std::path::Path;
use tracing::info;

use crate::ingest::read_chunks;

pub fn run(input: &Path, out: &Path) -> Result<()> {
    let chunks = read_chunks(input)?;
    info!("Read {} chunks for extraction", chunks.len());

    let extractor = EntityExtractor::new();
    let dedup = Deduplicator::new(0.95);

    let mut entities = Vec::new();
    for c in &chunks {
        let raw = RawChunk {
            chunk_id: c.chunk_id.clone(),
            content: c.content.clone(),
            source_url: c.source_url.clone(),
            page_index: None,
            token_count: c.content.split_whitespace().count(),
        };
        let extracted = extractor.extract(&raw.content)?;
        entities.extend(extracted);
    }

    let before = entities.len();
    let deduped = dedup.deduplicate(entities)?;
    info!("Extracted {} entities, {} after dedup", before, deduped.len());

    let mut f = std::fs::File::create(out)
        .with_context(|| format!("create {}", out.display()))?;
    for e in &deduped {
        let line = serde_json::to_string(e)?;
        writeln!(f, "{}", line)?;
    }
    info!("Wrote entities → {}", out.display());
    Ok(())
}
