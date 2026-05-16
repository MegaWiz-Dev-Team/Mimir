//! Deduplication engine using Jaccard similarity

use crate::{types::*, error::Result};
use std::collections::HashSet;

/// Deduplicator for entities using Jaccard similarity
pub struct Deduplicator {
    threshold: f32,
}

impl Deduplicator {
    /// Create new deduplicator with similarity threshold
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Deduplicate entities by semantic similarity
    pub fn deduplicate(&self, entities: Vec<Entity>) -> Result<Vec<ConsolidatedEntity>> {
        if entities.is_empty() {
            return Ok(Vec::new());
        }

        let mut consolidated = Vec::new();
        let mut processed = HashSet::new();

        for (i, entity) in entities.iter().enumerate() {
            if processed.contains(&i) {
                continue;
            }

            let mut merged_from = Vec::new();
            let mut all_sources = entity.sources.clone();
            let mut avg_confidence = entity.confidence;
            let mut merge_count = 1;

            // Find similar entities to merge
            for (j, other) in entities.iter().enumerate().skip(i + 1) {
                if processed.contains(&j) {
                    continue;
                }

                let similarity = self.jaccard_similarity(&entity.text, &other.text);
                if similarity >= self.threshold as f64 {
                    processed.insert(j);
                    merged_from.push(other.entity_id.clone());
                    all_sources.extend(other.sources.clone());
                    avg_confidence += other.confidence;
                    merge_count += 1;
                }
            }

            processed.insert(i);
            avg_confidence /= merge_count as f32;

            let consolidated_entity = ConsolidatedEntity {
                entity_id: entity.entity_id.clone(),
                text: entity.text.clone(),
                entity_type: entity.entity_type,
                confidence: avg_confidence,
                sources: all_sources,
                compressed_refs: Vec::new(),
                merged_from,
                metadata: entity.metadata.clone(),
            };

            consolidated.push(consolidated_entity);
        }

        Ok(consolidated)
    }

    /// Calculate Jaccard similarity between two texts
    fn jaccard_similarity(&self, text1: &str, text2: &str) -> f64 {
        let tokens1 = self.tokenize(text1);
        let tokens2 = self.tokenize(text2);

        if tokens1.is_empty() || tokens2.is_empty() {
            return 0.0;
        }

        let intersection = tokens1.intersection(&tokens2).count();
        let union = tokens1.union(&tokens2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Tokenize text into words (lowercase, alphanumeric only)
    fn tokenize(&self, text: &str) -> HashSet<String> {
        text.to_lowercase()
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_similarity_identical() {
        let dedup = Deduplicator::new(0.95);
        let sim = dedup.jaccard_similarity("hello world", "hello world");
        assert_eq!(sim, 1.0);
    }

    #[test]
    fn test_jaccard_similarity_different() {
        let dedup = Deduplicator::new(0.95);
        let sim = dedup.jaccard_similarity("hello world", "foo bar baz");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_jaccard_similarity_partial() {
        let dedup = Deduplicator::new(0.95);
        let sim = dedup.jaccard_similarity("hello world", "hello there");
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn test_tokenization() {
        let dedup = Deduplicator::new(0.95);
        let tokens = dedup.tokenize("Hello World");
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
    }

    #[test]
    fn test_deduplication() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness Coverage".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["prudential.co.th".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Critical Illness Coverage".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.92,
                sources: vec!["axa.co.th".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 1); // Should merge into 1
        assert_eq!(consolidated[0].merged_from.len(), 1); // Merged 1 entity
    }
}
