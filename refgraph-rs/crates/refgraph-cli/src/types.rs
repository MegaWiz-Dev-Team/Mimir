//! Pipeline-wide types shared across CLI subcommands.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A chunk produced by Phase-1 scraping. Compatible with both refgraph-core's
/// `RawChunk` and the insurance_ingestion_s2 JSONL output (extra fields are
/// captured in `extra` for forward compat).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Stable chunk identifier. May come from upstream as `source_id` or
    /// `chunk_id`; either is accepted via `#[serde(alias = ...)]`.
    #[serde(alias = "source_id")]
    pub chunk_id: String,
    pub content: String,
    /// URL the chunk was extracted from. Aliased from upstream `source_url`.
    #[serde(default, alias = "source_url")]
    pub source_url: String,
    #[serde(default)]
    pub insurer_id: String,
    #[serde(default)]
    pub product_type: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub product_name: String,
    /// Everything else from upstream we don't model explicitly.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Chunk {
    pub fn title(&self) -> String {
        if !self.product_name.is_empty() {
            self.product_name.clone()
        } else if !self.insurer_id.is_empty() {
            format!("{}: {}", self.insurer_id, self.chunk_id)
        } else {
            self.chunk_id.clone()
        }
    }
}

/// 10 standard insurance queries used for Hit Rate@3 validation.
/// Mirrors `s1_test_query_baseline` memory + insurance_ingestion_s2/phase5.
pub fn standard_queries() -> Vec<TestQuery> {
    vec![
        // Tier 1: Lookup
        q("What products cover hospitalization?", "lookup",
            &["PRU Mao Mao", "PRUBetter Care", "hospital"]),
        q("Critical illness coverage options", "lookup",
            &["Critical Illness", "PRULady Cancer", "PRURokrai"]),
        // Tier 2: Reasoning
        q("Which plans are suitable for young adults aged 20-35?", "reasoning",
            &["PRULife Care", "PRUEasy PA", "young"]),
        q("Difference between term and whole life insurance?", "reasoning",
            &["term", "whole life", "PRUWhole Life"]),
        // Tier 3: Exclusion
        q("Are dental procedures covered?", "exclusion",
            &["dental", "exclusion", "not covered"]),
        q("What conditions are excluded from critical illness plans?", "exclusion",
            &["exclusion", "limitation", "excluded"]),
        // Tier 4: Robustness
        q("PRU product premium cost", "robustness",
            &["premium", "PRU", "cost"]),
        q("insurance coverage limit", "robustness",
            &["coverage", "limit", "million"]),
        q("ประกันสุขภาพ", "robustness_th",
            &["health", "ประกัน", "สุขภาพ"]),
        q("ประกันชีวิต", "robustness_th",
            &["life", "ประกัน", "ชีวิต"]),
    ]
}

fn q(query: &str, tier: &str, expected: &[&str]) -> TestQuery {
    TestQuery {
        query: query.to_string(),
        tier: tier.to_string(),
        expected_terms: expected.iter().map(|s| s.to_string()).collect(),
    }
}

#[derive(Debug, Clone)]
pub struct TestQuery {
    pub query: String,
    pub tier: String,
    /// At least one of these terms (case-insensitive substring) must appear
    /// in the top-K results' content for the query to count as a hit.
    pub expected_terms: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HitRateReport {
    pub total_queries: usize,
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f64,
    pub by_tier: HashMap<String, TierStats>,
    pub per_query: Vec<QueryResult>,
}

#[derive(Debug, Serialize, Default)]
pub struct TierStats {
    pub total: usize,
    pub hits: usize,
    pub hit_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub query: String,
    pub tier: String,
    pub hit: bool,
    pub matched_terms: Vec<String>,
    pub latency_ms: u128,
    pub top_results: Vec<TopResult>,
}

#[derive(Debug, Serialize)]
pub struct TopResult {
    pub source: String,
    pub snippet: String,
    pub score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_phase1_jsonl_shape() {
        // Real shape produced by insurance_ingestion_s2/phase1 (source_id, source_url aliased)
        let line = r#"{"source_id":"url_insurer_001__0","content":"Health insurance plans","insurer_id":"insurer_001","product_type":"health","language":"en","product_name":"Health","metadata":{"source_url":"https://prudential.co.th/en/products/health/"}}"#;
        let c: Chunk = serde_json::from_str(line).expect("phase1 chunk parse");
        assert_eq!(c.chunk_id, "url_insurer_001__0");
        assert_eq!(c.content, "Health insurance plans");
        assert_eq!(c.insurer_id, "insurer_001");
    }

    #[test]
    fn parses_native_chunk_jsonl() {
        let line = r#"{"chunk_id":"c1","content":"X","source_url":"https://x.com"}"#;
        let c: Chunk = serde_json::from_str(line).expect("native chunk parse");
        assert_eq!(c.chunk_id, "c1");
        assert_eq!(c.source_url, "https://x.com");
    }

    #[test]
    fn standard_queries_has_10_balanced_tiers() {
        let qs = standard_queries();
        assert_eq!(qs.len(), 10, "must have 10 standard queries");
        let lookup = qs.iter().filter(|q| q.tier == "lookup").count();
        let reasoning = qs.iter().filter(|q| q.tier == "reasoning").count();
        let exclusion = qs.iter().filter(|q| q.tier == "exclusion").count();
        let robust = qs.iter().filter(|q| q.tier.starts_with("robustness")).count();
        assert_eq!(lookup, 2);
        assert_eq!(reasoning, 2);
        assert_eq!(exclusion, 2);
        assert_eq!(robust, 4);
    }

    #[test]
    fn title_prefers_product_name() {
        let mut c = Chunk {
            chunk_id: "c1".into(),
            content: "x".into(),
            source_url: "".into(),
            insurer_id: "insurer_001".into(),
            product_type: "".into(),
            language: "".into(),
            product_name: "PRU Mao Mao".into(),
            extra: HashMap::new(),
        };
        assert_eq!(c.title(), "PRU Mao Mao");
        c.product_name.clear();
        assert_eq!(c.title(), "insurer_001: c1");
    }
}
