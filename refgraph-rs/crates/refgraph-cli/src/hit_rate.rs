//! Hit Rate@K validator.
//!
//! Runs 10 standard insurance queries against
//! `POST {mimir_url}/api/v1/search` (the multi-source RAG endpoint —
//! Vector + Graph + Tree fusion with rerank) and computes Hit Rate@K —
//! fraction of queries where any top-K result contains at least one
//! expected term.
//!
//! Available since mimir-api:s1 (built 2026-05-17 from
//! feat/curator-review-ui). The endpoint takes JSON
//! `{query, tenant_id, limit}` and returns
//! `{results: [{content, title, score, source_type, metadata}], ...}`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

use crate::types::{
    standard_queries, HitRateReport, QueryResult, TestQuery, TierStats, TopResult,
};

#[derive(Debug, Serialize)]
struct SearchRequest<'a> {
    query: &'a str,
    tenant_id: &'a str,
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchHit>,
    /// Some Mimir variants return results under different keys; we try
    /// `results` first then fall back to `chunks` / `hits` in the handler.
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Debug, Deserialize, Default)]
struct SearchHit {
    #[serde(default)]
    content: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    title: String,
    /// `score` in newer Mimir / `relevance_score` in insurance RAG.
    #[serde(default, alias = "relevance_score")]
    score: Option<f64>,
}

impl SearchHit {
    fn body(&self) -> String {
        if !self.content.is_empty() {
            self.content.clone()
        } else if !self.text.is_empty() {
            self.text.clone()
        } else {
            self.snippet.clone()
        }
    }
    fn source_label(&self) -> String {
        if !self.source.is_empty() {
            self.source.clone()
        } else {
            self.title.clone()
        }
    }
}

pub async fn run(mimir_url: &str, tenant_id: &str, top_k: usize) -> Result<()> {
    let queries = standard_queries();
    // /api/v1/search SOURCE_TIMEOUT_SECS=45 — client must wait longer than
    // the slowest source timeout to see structured "0 results" rather than
    // a client-side abort.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let endpoint = format!("{}/api/v1/search", mimir_url.trim_end_matches('/'));
    info!("Hit Rate@{} — {} queries → {}", top_k, queries.len(), endpoint);

    let mut per_query = Vec::new();
    let mut by_tier: HashMap<String, TierStats> = HashMap::new();
    let mut hits = 0usize;

    for (i, tq) in queries.iter().enumerate() {
        let result = run_one_query(&client, &endpoint, tenant_id, tq, top_k).await;
        let q_res = match result {
            Ok(r) => r,
            Err(e) => {
                warn!("[{}/{}] ❌ {} — request error: {}", i + 1, queries.len(), tq.query, e);
                QueryResult {
                    query: tq.query.clone(),
                    tier: tq.tier.clone(),
                    hit: false,
                    matched_terms: Vec::new(),
                    latency_ms: 0,
                    top_results: Vec::new(),
                }
            }
        };

        let mark = if q_res.hit { "✅" } else { "❌" };
        info!(
            "[{}/{}] {} [{}] {} → {} matched in top-{} ({}ms)",
            i + 1,
            queries.len(),
            mark,
            tq.tier,
            tq.query,
            q_res.matched_terms.len(),
            top_k,
            q_res.latency_ms
        );

        if q_res.hit {
            hits += 1;
        }
        let ts = by_tier.entry(tq.tier.clone()).or_default();
        ts.total += 1;
        if q_res.hit {
            ts.hits += 1;
        }
        per_query.push(q_res);
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

async fn run_one_query(
    client: &reqwest::Client,
    endpoint: &str,
    tenant_id: &str,
    tq: &TestQuery,
    top_k: usize,
) -> Result<QueryResult> {
    let body = SearchRequest {
        query: &tq.query,
        tenant_id,
        limit: top_k,
    };

    let start = Instant::now();
    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .context("POST /api/search")?;
    let status = resp.status();
    let raw_text = resp.text().await.unwrap_or_default();
    let latency_ms = start.elapsed().as_millis();

    if !status.is_success() {
        anyhow::bail!("HTTP {}: {}", status, &raw_text[..raw_text.len().min(200)]);
    }

    let parsed: SearchResponse = serde_json::from_str(&raw_text).unwrap_or(SearchResponse {
        results: Vec::new(),
        extra: HashMap::new(),
    });

    // Mimir may return results under `results`, `chunks`, or `hits`. Try fallbacks.
    let mut hits: Vec<SearchHit> = parsed.results;
    if hits.is_empty() {
        for key in ["chunks", "hits", "items", "documents"] {
            if let Some(Value::Array(arr)) = parsed.extra.get(key) {
                hits = arr
                    .iter()
                    .filter_map(|v| serde_json::from_value::<SearchHit>(v.clone()).ok())
                    .collect();
                if !hits.is_empty() {
                    break;
                }
            }
        }
    }
    hits.truncate(top_k);

    let matched_terms = match_terms(&hits, &tq.expected_terms);
    let hit = !matched_terms.is_empty();

    let top_results = hits
        .iter()
        .map(|h| TopResult {
            source: h.source_label(),
            snippet: truncate(&h.body(), 120),
            score: h.score.unwrap_or(0.0),
        })
        .collect();

    Ok(QueryResult {
        query: tq.query.clone(),
        tier: tq.tier.clone(),
        hit,
        matched_terms,
        latency_ms,
        top_results,
    })
}

/// Case-insensitive substring match between expected terms and concatenated hit bodies.
fn match_terms(hits: &[SearchHit], expected: &[String]) -> Vec<String> {
    if hits.is_empty() || expected.is_empty() {
        return Vec::new();
    }
    let haystack: String = hits
        .iter()
        .map(|h| h.body())
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
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn print_report(r: &HitRateReport, top_k: usize) {
    println!();
    println!("══════════════════════════════════════════════");
    println!("  Hit Rate@{} Report", top_k);
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
        "❌ FAIL — <50%, activate Plan B (swap embedding model)"
    };
    println!("  Decision gate: {}", gate);
    println!("══════════════════════════════════════════════");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(body: &str) -> SearchHit {
        SearchHit {
            content: body.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn match_terms_case_insensitive() {
        let hits = vec![hit("PRU Mao Mao covers Hospitalization expenses.")];
        let matched = match_terms(&hits, &vec!["pru mao mao".into(), "hospital".into()]);
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn match_terms_no_match_returns_empty() {
        let hits = vec![hit("life insurance details")];
        let matched = match_terms(&hits, &vec!["dental".into(), "auto".into()]);
        assert!(matched.is_empty());
    }

    #[test]
    fn match_terms_empty_inputs() {
        assert!(match_terms(&[], &vec!["x".into()]).is_empty());
        assert!(match_terms(&[hit("anything")], &[]).is_empty());
    }
}
