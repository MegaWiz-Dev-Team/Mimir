//! Entity extraction module

use crate::{types::*, error::Result};
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
            if text.contains(product) {
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

        if found.is_empty() { None } else { Some(found) }
    }

    /// Extract coverage entities
    fn extract_coverages(&self, text: &str) -> Option<Vec<Entity>> {
        let coverages = vec!["Heart Attack", "Stroke", "Diabetes"];
        let mut found = Vec::new();

        for coverage in coverages {
            if text.contains(coverage) {
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

        if found.is_empty() { None } else { Some(found) }
    }

    /// Extract exclusion entities
    fn extract_exclusions(&self, text: &str) -> Option<Vec<Entity>> {
        let exclusions = vec!["Pre-existing Condition", "Experimental Treatment"];
        let mut found = Vec::new();

        for exclusion in exclusions {
            if text.contains(exclusion) {
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

        if found.is_empty() { None } else { Some(found) }
    }

    /// Detect language in text (en or th)
    pub fn detect_language(&self, text: &str) -> String {
        // MVP: Simple heuristic (count Thai unicode ranges)
        let thai_count = text
            .chars()
            .filter(|c| (*c as u32) >= 0x0E00 && (*c as u32) <= 0x0E7F)
            .count();

        if thai_count > text.len() / 4 {
            "th".to_string()
        } else {
            "en".to_string()
        }
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
}
