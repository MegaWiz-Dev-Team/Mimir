use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use mimir_core_ai::services::llm_router::UniversalClient;
use crate::retrieval::qdrant::RetrievalResult;

// ── Configuration ─────────────────────────────────────

/// Weights for each retrieval source in the ensemble.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleWeights {
    pub vector: f32,
    pub tree: f32,
    pub graph: f32,
}

impl Default for EnsembleWeights {
    fn default() -> Self {
        Self {
            vector: 0.5,
            tree: 0.3,
            graph: 0.2,
        }
    }
}

impl EnsembleWeights {
    /// Validate that weights sum to ~1.0 and are non-negative.
    pub fn validate(&self) -> Result<(), String> {
        if self.vector < 0.0 || self.tree < 0.0 || self.graph < 0.0 {
            return Err("Weights must be non-negative".to_string());
        }
        let sum = self.vector + self.tree + self.graph;
        if (sum - 1.0).abs() > 0.01 {
            return Err(format!("Weights must sum to 1.0, got {:.2}", sum));
        }
        Ok(())
    }

    /// Normalize weights so they sum to exactly 1.0.
    pub fn normalize(&mut self) {
        let sum = self.vector + self.tree + self.graph;
        if sum > 0.0 {
            self.vector /= sum;
            self.tree /= sum;
            self.graph /= sum;
        }
    }
}

// ── Reranker ──────────────────────────────────────────

/// Rerank and merge results from multiple retrieval sources.
///
/// Algorithm:
/// 1. Apply source_type weight to each result's score
/// 2. Sort by weighted score descending
/// 3. Deduplicate by title (keep highest scored)
/// 4. Truncate to limit
pub fn rerank_results(
    results: &[RetrievalResult],
    weights: &EnsembleWeights,
    limit: usize,
) -> Vec<RetrievalResult> {
    // Apply weights
    let mut weighted: Vec<RetrievalResult> = results
        .iter()
        .map(|r| {
            let weight = match r.source_type.as_str() {
                "vector" => weights.vector,
                "tree" => weights.tree,
                "graph" => weights.graph,
                _ => 0.1,
            };
            RetrievalResult {
                content: r.content.clone(),
                title: r.title.clone(),
                score: r.score * weight,
                source_type: r.source_type.clone(),
                metadata: r.metadata.clone(),
            }
        })
        .collect();

    // Sort by weighted score descending
    weighted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deduplicate by title (keep highest scored)
    let mut seen = std::collections::HashSet::new();
    weighted.retain(|r| seen.insert(r.title.clone()));

    // Truncate
    weighted.truncate(limit);

    weighted
}

/// Reciprocal Rank Fusion (RRF) reranking — source-type-agnostic.
///
/// Algorithm:
/// 1. Rank results within each source type by their original score
/// 2. Compute RRF score: weight / (k + rank) for each result per source
/// 3. Sum RRF scores for results appearing in multiple sources
/// 4. Sort by total RRF score descending, dedup, truncate
///
/// This is superior to weighted-score when source score scales differ
/// (e.g., vector 0-1 vs tree hardcoded 0.8).
pub fn rerank_results_rrf(
    results: &[RetrievalResult],
    weights: &EnsembleWeights,
    limit: usize,
) -> Vec<RetrievalResult> {
    use std::collections::HashMap;

    if results.is_empty() {
        return vec![];
    }

    const RRF_K: f32 = 60.0; // Standard RRF tuning constant

    // Group results by source type and rank within each
    let source_types = ["vector", "tree", "graph"];
    let mut rrf_scores: HashMap<String, (f32, RetrievalResult)> = HashMap::new();

    for src in &source_types {
        let weight = match *src {
            "vector" => weights.vector,
            "tree" => weights.tree,
            "graph" => weights.graph,
            _ => 0.0,
        };

        if weight <= 0.0 {
            continue;
        }

        // Get results from this source, sorted by score descending
        let mut src_results: Vec<&RetrievalResult> = results.iter()
            .filter(|r| r.source_type == *src)
            .collect();
        src_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Assign RRF scores based on rank
        for (rank_0, result) in src_results.iter().enumerate() {
            let rank = (rank_0 + 1) as f32; // 1-indexed
            let rrf_score = weight / (RRF_K + rank);

            rrf_scores.entry(result.title.clone())
                .and_modify(|(score, _)| *score += rrf_score)
                .or_insert((rrf_score, (*result).clone()));
        }
    }

    // Sort by total RRF score descending
    let mut final_results: Vec<RetrievalResult> = rrf_scores.into_values()
        .map(|(score, mut r)| {
            r.score = score;
            r
        })
        .collect();
    final_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    final_results.truncate(limit);
    final_results
}

/// Reranks the given pre-filtered results using an actual Cross-Encoder machine learning model.
pub async fn cross_encoder_rerank(
    client: &UniversalClient,
    model: &str,
    query: &str,
    mut results: Vec<RetrievalResult>,
    final_top_k: usize,
) -> anyhow::Result<Vec<RetrievalResult>> {
    if results.is_empty() {
        return Ok(Vec::new());
    }

    let texts: Vec<String> = results.iter().map(|r| r.content.clone()).collect();
    let scores = client.rerank(model, query, &texts).await?;

    let mut score_map: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    for (i, s) in scores {
        score_map.insert(i, s);
    }

    for (i, res) in results.iter_mut().enumerate() {
        if let Some(s) = score_map.get(&i) {
            res.score = *s;
        } else {
            // Assign low score to unranked items
            res.score = f32::MIN;
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut seen = std::collections::HashSet::new();
    results.retain(|r| seen.insert(r.title.clone()));
    
    results.truncate(final_top_k);

    Ok(results)
}

/// Compute a summary of which sources contributed to the final results.
pub fn source_distribution(results: &[RetrievalResult]) -> Value {
    let mut vector_count = 0;
    let mut tree_count = 0;
    let mut graph_count = 0;

    for r in results {
        match r.source_type.as_str() {
            "vector" => vector_count += 1,
            "tree" => tree_count += 1,
            "graph" => graph_count += 1,
            _ => {}
        }
    }

    json!({
        "vector": vector_count,
        "tree": tree_count,
        "graph": graph_count,
        "total": results.len(),
    })
}

/// Determine the overall mode_used based on which sources contributed.
pub fn determine_mode_used(results: &[RetrievalResult]) -> &'static str {
    let has_vector = results.iter().any(|r| r.source_type == "vector");
    let has_tree = results.iter().any(|r| r.source_type == "tree");
    let has_graph = results.iter().any(|r| r.source_type == "graph");

    match (has_vector, has_tree, has_graph) {
        (true, false, false) => "vector",
        (false, true, false) => "tree",
        (false, false, true) => "graph",
        (false, false, false) => "none",
        _ => "hybrid",
    }
}

// ── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::qdrant::RetrievalResult;

    fn make_result(title: &str, score: f32, source: &str) -> RetrievalResult {
        RetrievalResult {
            content: format!("Content for {}", title),
            title: title.to_string(),
            score,
            source_type: source.to_string(),
            metadata: json!({}),
        }
    }

    // ── EnsembleWeights ───────────────────────────────

    #[test]
    fn test_default_weights() {
        let w = EnsembleWeights::default();
        assert_eq!(w.vector, 0.5);
        assert_eq!(w.tree, 0.3);
        assert_eq!(w.graph, 0.2);
        assert!(w.validate().is_ok());
    }

    #[test]
    fn test_weights_validation_ok() {
        let w = EnsembleWeights {
            vector: 0.4,
            tree: 0.4,
            graph: 0.2,
        };
        assert!(w.validate().is_ok());
    }

    #[test]
    fn test_weights_validation_negative() {
        let w = EnsembleWeights {
            vector: -0.1,
            tree: 0.6,
            graph: 0.5,
        };
        assert!(w.validate().is_err());
        assert!(w.validate().unwrap_err().contains("non-negative"));
    }

    #[test]
    fn test_weights_validation_wrong_sum() {
        let w = EnsembleWeights {
            vector: 0.5,
            tree: 0.5,
            graph: 0.5,
        };
        assert!(w.validate().is_err());
        assert!(w.validate().unwrap_err().contains("1.0"));
    }

    #[test]
    fn test_weights_normalize() {
        let mut w = EnsembleWeights {
            vector: 1.0,
            tree: 1.0,
            graph: 1.0,
        };
        w.normalize();
        assert!((w.vector - 0.333).abs() < 0.01);
        assert!((w.tree - 0.333).abs() < 0.01);
        assert!((w.graph - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_weights_serialization() {
        let w = EnsembleWeights::default();
        let json = serde_json::to_string(&w).unwrap();
        let deser: EnsembleWeights = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.vector, 0.5);
    }

    // ── rerank_results ────────────────────────────────

    #[test]
    fn test_rerank_applies_weights() {
        let results = vec![
            make_result("VecDoc", 0.9, "vector"),
            make_result("TreeDoc", 0.9, "tree"),
            make_result("GraphDoc", 0.9, "graph"),
        ];
        let weights = EnsembleWeights::default(); // v=0.5, t=0.3, g=0.2

        let ranked = rerank_results(&results, &weights, 10);
        assert_eq!(ranked.len(), 3);
        // Vector should be first (0.9 * 0.5 = 0.45)
        assert_eq!(ranked[0].source_type, "vector");
        // Tree second (0.9 * 0.3 = 0.27)
        assert_eq!(ranked[1].source_type, "tree");
        // Graph third (0.9 * 0.2 = 0.18)
        assert_eq!(ranked[2].source_type, "graph");
    }

    #[test]
    fn test_rerank_deduplicates_by_title() {
        let results = vec![
            make_result("SameDoc", 0.9, "vector"),
            make_result("SameDoc", 0.8, "tree"),
            make_result("OtherDoc", 0.7, "graph"),
        ];
        let weights = EnsembleWeights::default();

        let ranked = rerank_results(&results, &weights, 10);
        assert_eq!(ranked.len(), 2, "Duplicates should be removed");
        // The "SameDoc" entry with higher weighted score should survive
        let same_doc = ranked.iter().find(|r| r.title == "SameDoc").unwrap();
        assert_eq!(same_doc.source_type, "vector");
    }

    #[test]
    fn test_rerank_respects_limit() {
        let results = vec![
            make_result("A", 0.9, "vector"),
            make_result("B", 0.8, "vector"),
            make_result("C", 0.7, "tree"),
            make_result("D", 0.6, "graph"),
        ];
        let weights = EnsembleWeights::default();

        let ranked = rerank_results(&results, &weights, 2);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn test_rerank_empty_input() {
        let ranked = rerank_results(&[], &EnsembleWeights::default(), 10);
        assert!(ranked.is_empty());
    }

    #[test]
    fn test_rerank_graph_heavy_weights() {
        let results = vec![
            make_result("VecDoc", 0.9, "vector"),
            make_result("GraphDoc", 0.9, "graph"),
        ];
        // Graph-heavy weights
        let weights = EnsembleWeights {
            vector: 0.1,
            tree: 0.1,
            graph: 0.8,
        };

        let ranked = rerank_results(&results, &weights, 10);
        // Graph should be first now (0.9 * 0.8 = 0.72 vs 0.9 * 0.1 = 0.09)
        assert_eq!(ranked[0].source_type, "graph");
    }

    // ── source_distribution ───────────────────────────

    #[test]
    fn test_source_distribution() {
        let results = vec![
            make_result("A", 0.9, "vector"),
            make_result("B", 0.8, "vector"),
            make_result("C", 0.7, "tree"),
            make_result("D", 0.6, "graph"),
        ];

        let dist = source_distribution(&results);
        assert_eq!(dist["vector"], 2);
        assert_eq!(dist["tree"], 1);
        assert_eq!(dist["graph"], 1);
        assert_eq!(dist["total"], 4);
    }

    #[test]
    fn test_source_distribution_empty() {
        let dist = source_distribution(&[]);
        assert_eq!(dist["total"], 0);
    }

    // ── determine_mode_used ───────────────────────────

    #[test]
    fn test_mode_single_vector() {
        let results = vec![make_result("A", 0.9, "vector")];
        assert_eq!(determine_mode_used(&results), "vector");
    }

    #[test]
    fn test_mode_mixed_hybrid() {
        let results = vec![
            make_result("A", 0.9, "vector"),
            make_result("B", 0.8, "tree"),
        ];
        assert_eq!(determine_mode_used(&results), "hybrid");
    }

    #[test]
    fn test_mode_all_three_hybrid() {
        let results = vec![
            make_result("A", 0.9, "vector"),
            make_result("B", 0.8, "tree"),
            make_result("C", 0.7, "graph"),
        ];
        assert_eq!(determine_mode_used(&results), "hybrid");
    }

    #[test]
    fn test_mode_empty_none() {
        assert_eq!(determine_mode_used(&[]), "none");
    }

    #[test]
    fn test_mode_single_graph() {
        let results = vec![make_result("A", 0.9, "graph")];
        assert_eq!(determine_mode_used(&results), "graph");
    }
}
