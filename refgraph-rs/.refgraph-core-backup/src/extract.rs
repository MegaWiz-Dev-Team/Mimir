//! Entity extraction module

use crate::{error::Result, types::*};
use regex;
use std::collections::HashMap;

/// Entity extractor for identifying entities in text
pub struct EntityExtractor {
    // Note: In production, integrate with:
    // - spaCy via FFI (English NER)
    // - pythainlp via Python bridge (Thai NER)
    // - For MVP: pattern-based extraction
}

impl EntityExtractor {
    /// Create new entity extractor
    pub fn new() -> Self {
        Self {}
    }

    /// Extract entities from text
    pub fn extract(&self, text: &str) -> Result<Vec<Entity>> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        let mut entities = Vec::new();

        // MVP: Pattern-based extraction for insurance domain
        // In production: use ML models via FFI

        // Extract product mentions (e.g., "Critical Illness", "Health Insurance")
        if let Some(ents) = self.extract_products(text) {
            entities.extend(ents);
        }

        // Extract coverage mentions
        if let Some(ents) = self.extract_coverages(text) {
            entities.extend(ents);
        }

        // Extract exclusions
        if let Some(ents) = self.extract_exclusions(text) {
            entities.extend(ents);
        }

        Ok(entities)
    }

    /// Extract product entities
    fn extract_products(&self, text: &str) -> Option<Vec<Entity>> {
        let products = vec!["Critical Illness", "Health Insurance", "Life Insurance"];
        let mut found = Vec::new();

        for product in products {
            if Self::contains_word_boundary(text, product) {
                found.push(Entity {
                    entity_id: format!("prod_{}", product.to_lowercase().replace(' ', "_")),
                    text: product.to_string(),
                    entity_type: EntityType::Product,
                    confidence: 0.92,
                    sources: Vec::new(), // Set by consolidator
                    metadata: HashMap::new(),
                });
            }
        }

        if found.is_empty() {
            None
        } else {
            Some(found)
        }
    }

    /// Extract coverage entities
    fn extract_coverages(&self, text: &str) -> Option<Vec<Entity>> {
        let coverages = vec!["Heart Attack", "Stroke", "Diabetes"];
        let mut found = Vec::new();

        for coverage in coverages {
            // Use word boundaries to avoid partial matches
            if Self::contains_word_boundary(text, coverage) {
                found.push(Entity {
                    entity_id: format!("cov_{}", coverage.to_lowercase().replace(' ', "_")),
                    text: coverage.to_string(),
                    entity_type: EntityType::Coverage,
                    confidence: 0.88,
                    sources: Vec::new(),
                    metadata: HashMap::new(),
                });
            }
        }

        if found.is_empty() {
            None
        } else {
            Some(found)
        }
    }

    /// Extract exclusion entities
    fn extract_exclusions(&self, text: &str) -> Option<Vec<Entity>> {
        let exclusions = vec!["Pre-existing Condition", "Experimental Treatment"];
        let mut found = Vec::new();

        for exclusion in exclusions {
            if Self::contains_word_boundary(text, exclusion) {
                found.push(Entity {
                    entity_id: format!("exc_{}", exclusion.to_lowercase().replace(' ', "_")),
                    text: exclusion.to_string(),
                    entity_type: EntityType::Exclusion,
                    confidence: 0.85,
                    sources: Vec::new(),
                    metadata: HashMap::new(),
                });
            }
        }

        if found.is_empty() {
            None
        } else {
            Some(found)
        }
    }

    /// Detect language in text (en or th)
    pub fn detect_language(&self, text: &str) -> String {
        // MVP: Simple heuristic (count Thai unicode ranges)
        let thai_count = text
            .chars()
            .filter(|c| (*c as u32) >= 0x0E00 && (*c as u32) <= 0x0E7F)
            .count();

        // If at least 10% Thai characters, classify as Thai
        // This handles mixed Thai-English text
        if thai_count > text.len() / 10 {
            "th".to_string()
        } else {
            "en".to_string()
        }
    }

    /// Check if text contains a word with word boundaries
    fn contains_word_boundary(text: &str, word: &str) -> bool {
        let pattern = format!(r"\b{}\b", word);
        regex::Regex::new(&pattern)
            .map(|re| re.is_match(text))
            .unwrap_or_else(|_| text.contains(word))
    }
}

impl Default for EntityExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_products() {
        let extractor = EntityExtractor::new();
        let text = "We offer Critical Illness and Health Insurance";
        let entities = extractor.extract(text).unwrap();

        assert!(entities.iter().any(|e| e.text == "Critical Illness"));
        assert!(entities.iter().any(|e| e.text == "Health Insurance"));
    }

    #[test]
    fn test_extract_coverages() {
        let extractor = EntityExtractor::new();
        let text = "Covers Heart Attack and Stroke";
        let entities = extractor.extract(text).unwrap();

        assert!(entities.iter().any(|e| e.text == "Heart Attack"));
        assert!(entities.iter().any(|e| e.text == "Stroke"));
    }

    #[test]
    fn test_language_detection_english() {
        let extractor = EntityExtractor::new();
        let lang = extractor.detect_language("Hello world");
        assert_eq!(lang, "en");
    }

    #[test]
    fn test_language_detection_thai() {
        let extractor = EntityExtractor::new();
        let lang = extractor.detect_language("สวัสดีชาวโลก");
        assert_eq!(lang, "th");
    }

    #[test]
    fn test_empty_text() {
        let extractor = EntityExtractor::new();
        let entities = extractor.extract("").unwrap();
        assert_eq!(entities.len(), 0);
    }

    #[test]
    fn test_extract_exclusions() {
        let extractor = EntityExtractor::new();
        let text = "Excludes Pre-existing Condition and Experimental Treatment";
        let entities = extractor.extract(text).unwrap();

        assert!(entities.iter().any(|e| e.text == "Pre-existing Condition"));
        assert!(entities.iter().any(|e| e.text == "Experimental Treatment"));
    }

    #[test]
    fn test_extract_multiple_types() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness covers Heart Attack but excludes Pre-existing Condition";
        let entities = extractor.extract(text).unwrap();

        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Product));
        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Coverage));
        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Exclusion));
    }

    #[test]
    fn test_no_matches() {
        let extractor = EntityExtractor::new();
        let text = "Random insurance text with no keywords";
        let entities = extractor.extract(text).unwrap();
        assert_eq!(entities.len(), 0);
    }

    #[test]
    fn test_entity_id_format() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness coverage";
        let entities = extractor.extract(text).unwrap();

        let product = entities
            .iter()
            .find(|e| e.text == "Critical Illness")
            .unwrap();
        assert!(product.entity_id.starts_with("prod_"));
    }

    #[test]
    fn test_confidence_scores() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness and Heart Attack";
        let entities = extractor.extract(text).unwrap();

        let product = entities
            .iter()
            .find(|e| e.text == "Critical Illness")
            .unwrap();
        assert!(product.confidence > 0.85 && product.confidence <= 1.0);

        let coverage = entities.iter().find(|e| e.text == "Heart Attack").unwrap();
        assert!(coverage.confidence > 0.80 && coverage.confidence <= 1.0);
    }

    #[test]
    fn test_thai_mixed_text() {
        let extractor = EntityExtractor::new();
        let text = "สุขภาพ Health Insurance and ประกันชีวิต";
        let lang = extractor.detect_language(text);
        assert_eq!(lang, "th");
    }

    #[test]
    fn test_case_sensitivity() {
        let extractor = EntityExtractor::new();
        let text = "critical illness insurance";
        let entities = extractor.extract(text).unwrap();
        // Case sensitive - should not match
        assert_eq!(entities.len(), 0);
    }

    #[test]
    fn test_partial_match_not_extracted() {
        let extractor = EntityExtractor::new();
        let text = "Cardiac disease is different from Heart Attack";
        let entities = extractor.extract(text).unwrap();
        // Should only extract "Heart Attack", not "Cardiac"
        assert!(entities.iter().any(|e| e.text == "Heart Attack"));
        assert!(!entities.iter().any(|e| e.text == "Cardiac"));
    }

    #[test]
    fn test_entity_type_variants() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness Heart Attack Pre-existing Condition";
        let entities = extractor.extract(text).unwrap();

        assert!(entities.len() >= 3);
        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Product));
        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Coverage));
        assert!(entities
            .iter()
            .any(|e| e.entity_type == EntityType::Exclusion));
    }

    #[test]
    fn test_duplicate_mentions() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness Critical Illness insurance";
        let entities = extractor.extract(text).unwrap();
        // Should still only have one product entity (no duplicates)
        let critical_count = entities
            .iter()
            .filter(|e| e.text == "Critical Illness")
            .count();
        assert_eq!(critical_count, 1);
    }
}
