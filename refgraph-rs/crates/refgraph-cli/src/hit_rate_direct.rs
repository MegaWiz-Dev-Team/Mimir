//! Direct Hit Rate@K validator — bypasses mimir-api entirely.
//!
//! The deployed mimir-api `/api/v1/search` route calls `embed_texts(query)`,
//! which (per `ro-ai-bridge/src/routes/vector.rs:19-66`) ignores its
//! `heimdall_url` variable and generates hash-based pseudo-vectors instead
//! of calling Heimdall. So even with real BGE-M3 vectors stored in Qdrant,
//! search via mimir-api compares fake-query vs real-doc → useless results.
//!
//! This module:
//!   1. Embeds each query via Heimdall (real BGE-M3)
//!   2. Searches Qdrant directly with that vector + tenant filter
//!   3. Reports Hit Rate@K against the same 10 standard queries
//!
//! This is the baseline that *would* be measured if the Mimir
//! `embed_texts` bug were fixed and the rest of the multi-source
//! retriever (Vector / Graph / Tree) operated against real embeddings.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

use crate::heimdall::HeimdallClient;
use crate::qdrant::{QdrantClient, SearchHit};
use crate::types::{
    standard_queries, HitRateReport, QueryResult, TestQuery, TierStats, TopResult,
};

pub struct DirectConfig {
    pub heimdall_url: String,
    pub heimdall_api_key: String,
    pub heimdall_model: String,
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub tenant_id: String,
}

pub async fn run(cfg: DirectConfig, top_k: usize) -> Result<()> {
    let queries = standard_queries();
    let heimdall = HeimdallClient::new(
        cfg.heimdall_url.clone(),
        cfg.heimdall_api_key.clone(),
        cfg.heimdall_model.clone(),
    )?;
    let qdrant = QdrantClient::new(cfg.qdrant_url.clone())?;

    info!(
        "Hit Rate@{} direct — {} queries → Heimdall({}) + Qdrant({} / {})",
        top_k,
        queries.len(),
        cfg.heimdall_model,
        cfg.qdrant_url,
        cfg.qdrant_collection
    );

    // Embed all queries in one batch
    let query_texts: Vec<String> = queries.iter().map(|q| q.query.clone()).collect();
    let query_vecs = heimdall
        .embed(&query_texts)
        .await
        .context("Heimdall embed queries")?;
    assert_eq!(query_vecs.len(), queries.len());

    let mut per_query = Vec::new();
    let mut by_tier: HashMap<String, TierStats> = HashMap::new();
    let mut hits = 0usize;

    for (i, (tq, qvec)) in queries.iter().zip(query_vecs.iter()).enumerate() {
        let start = Instant::now();
        let hits_vec = match qdrant
            .search(&cfg.qdrant_collection, qvec, &cfg.tenant_id, top_k)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                warn!("[{}/{}] ❌ {} — Qdrant search error: {}", i + 1, queries.len(), tq.query, e);
                Vec::new()
            }
        };
        let latency_ms = start.elapsed().as_millis();

        let matched_terms = match_terms(&hits_vec, &tq.expected_terms);
        let hit = !matched_terms.is_empty();
        if hit {
            hits += 1;
        }
        let ts = by_tier.entry(tq.tier.clone()).or_default();
        ts.total += 1;
        if hit {
            ts.hits += 1;
        }

        let top_results = hits_vec
            .iter()
            .map(|h| TopResult {
                source: format!("source_id={}", h.source_id),
                snippet: truncate(&h.content, 100),
                score: h.score,
            })
            .collect();

        let mark = if hit { "✅" } else { "❌" };
        info!(
            "[{}/{}] {} [{}] {} → {} matched in top-{} ({}ms, top score {:.3})",
            i + 1,
            queries.len(),
            mark,
            tq.tier,
            tq.query,
            matched_terms.len(),
            top_k,
            latency_ms,
            hits_vec.first().map(|h| h.score).unwrap_or(0.0)
        );

        per_query.push(QueryResult {
            query: tq.query.clone(),
            tier: tq.tier.clone(),
            hit,
            matched_terms,
            latency_ms,
            top_results,
        });
    }

    for ts in by_tier.values_mut() {
        ts.hit_rate = if ts.total > 0 {
            ts.hits as f64 / ts.total as f64
        } else {
            0.0
        };
    }

    let total = queries.len();
    let report = HitRateReport {
        total_queries: total,
        hits,
        misses: total - hits,
        hit_rate: hits as f64 / total as f64,
        by_tier,
        per_query,
    };

    print_report(&report, top_k);
    Ok(())
}

fn match_terms(hits: &[SearchHit], expected: &[String]) -> Vec<String> {
    if hits.is_empty() || expected.is_empty() {
        return Vec::new();
    }
    let haystack: String = hits
        .iter()
        .map(|h| h.content.clone())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    expected
        .iter()
        .filter(|t| haystack.contains(&t.to_lowercase()))
        .cloned()
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    let take = s.chars().take(max).collect::<String>();
    if s.chars().count() > max {
        format!("{}…", take)
    } else {
        take
    }
}

fn print_report(r: &HitRateReport, top_k: usize) {
    println!();
    println!("══════════════════════════════════════════════");
    println!("  Hit Rate@{} Report — DIRECT (bypass mimir-api)", top_k);
    println!("  Heimdall BGE-M3 → Qdrant /points/search");
    println!("══════════════════════════════════════════════");
    println!(
        "  Overall: {}/{} hits = {:.1}%",
        r.hits,
        r.total_queries,
        r.hit_rate * 100.0
    );
    println!();
    println!("  By tier:");
    let mut tiers: Vec<_> = r.by_tier.iter().collect();
    tiers.sort_by(|a, b| a.0.cmp(b.0));
    for (tier, stats) in tiers {
        println!(
            "    {:<14} {}/{} = {:.1}%",
            tier,
            stats.hits,
            stats.total,
            stats.hit_rate * 100.0
        );
    }
    println!();

    let gate = if r.hit_rate >= 0.75 {
        "✅ GO — meets S1 target (≥75%)"
    } else if r.hit_rate >= 0.50 {
        "⚠️  WARN — between 50% and 75%, tune before S2"
    } else {
        "❌ FAIL — <50%, activate Plan B (swap embedding model or improve chunking)"
    };
    println!("  Decision gate: {}", gate);
    println!("══════════════════════════════════════════════");
}
