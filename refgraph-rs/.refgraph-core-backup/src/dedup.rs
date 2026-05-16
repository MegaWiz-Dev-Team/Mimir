//! Deduplication engine using Jaccard similarity

use crate::{error::Result, types::*};
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
                entity_type: entity.entity_type.clone(),
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

    #[test]
    fn test_deduplication_empty_list() {
        let dedup = Deduplicator::new(0.95);
        let result = dedup.deduplicate(vec![]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_deduplication_single_entity() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![Entity {
            entity_id: "ent_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["prudential.co.th".to_string()],
            metadata: std::collections::HashMap::new(),
        }];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 1);
        assert_eq!(consolidated[0].merged_from.len(), 0);
    }

    #[test]
    fn test_deduplication_no_similarity() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["prudential.co.th".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Heart Attack".to_string(),
                entity_type: EntityType::Coverage,
                confidence: 0.92,
                sources: vec!["axa.co.th".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 2);
    }

    #[test]
    fn test_deduplication_merges_sources() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["prudential.co.th".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.90,
                sources: vec!["axa.co.th".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 1);
        assert_eq!(consolidated[0].sources.len(), 2);
        assert!(consolidated[0]
            .sources
            .contains(&"prudential.co.th".to_string()));
        assert!(consolidated[0].sources.contains(&"axa.co.th".to_string()));
    }

    #[test]
    fn test_deduplication_averages_confidence() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.90,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 1.00,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated[0].confidence, 0.95);
    }

    #[test]
    fn test_deduplication_case_insensitive() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "CRITICAL ILLNESS".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 1);
    }

    #[test]
    fn test_deduplication_custom_threshold() {
        let dedup = Deduplicator::new(0.50);
        // "apple banana" vs "apple cherry": {apple} / {apple, banana, cherry} = 1/3 ≈ 0.33
        // Won't merge at 0.95, but will at 0.50 only if there's higher similarity
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "apple banana".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "apple cherry".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        // With low threshold 0.50, should still not merge (33% < 50%)
        assert_eq!(consolidated.len(), 2);
    }

    #[test]
    fn test_deduplication_multiple_clusters() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_003".to_string(),
                text: "Heart Attack".to_string(),
                entity_type: EntityType::Coverage,
                confidence: 0.95,
                sources: vec!["src3".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_004".to_string(),
                text: "Heart Attack".to_string(),
                entity_type: EntityType::Coverage,
                confidence: 0.95,
                sources: vec!["src4".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 2);
        assert_eq!(consolidated[0].merged_from.len(), 1);
        assert_eq!(consolidated[1].merged_from.len(), 1);
    }

    #[test]
    fn test_deduplication_tracks_merged_entities() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Critical Illness".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.92,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated[0].merged_from.len(), 1);
        assert!(consolidated[0].merged_from.contains(&"ent_002".to_string()));
    }

    #[test]
    fn test_jaccard_empty_strings() {
        let dedup = Deduplicator::new(0.95);
        let sim = dedup.jaccard_similarity("", "");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_tokenization_punctuation_removal() {
        let dedup = Deduplicator::new(0.95);
        let tokens = dedup.tokenize("Hello, World!");
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn test_deduplication_with_numbers() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Plan 2024 Premium".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Plan 2024 Premium".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 1);
    }

    #[test]
    fn test_deduplication_performance_100_entities() {
        let dedup = Deduplicator::new(0.95);
        let mut entities = Vec::new();
        for i in 0..100 {
            let text = if i % 2 == 0 {
                "Critical Illness Insurance"
            } else {
                "Heart Attack Coverage"
            };
            entities.push(Entity {
                entity_id: format!("ent_{:03}", i),
                text: text.to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["test.com".to_string()],
                metadata: std::collections::HashMap::new(),
            });
        }

        let start = std::time::Instant::now();
        let result = dedup.deduplicate(entities);
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 2);
        assert!(
            elapsed.as_millis() < 100,
            "Dedup should be fast for 100 entities"
        );
    }

    #[test]
    fn test_deduplication_whitespace_normalization() {
        let dedup = Deduplicator::new(0.95);
        let entities = vec![
            Entity {
                entity_id: "ent_001".to_string(),
                text: "Critical  Illness   Insurance".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src1".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            Entity {
                entity_id: "ent_002".to_string(),
                text: "Critical Illness Insurance".to_string(),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["src2".to_string()],
                metadata: std::collections::HashMap::new(),
            },
        ];

        let result = dedup.deduplicate(entities);
        assert!(result.is_ok());
        let consolidated = result.unwrap();
        assert_eq!(consolidated.len(), 1, "Whitespace should be normalized");
    }
}
