//! Manifest configuration for domain-specific rules

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Domain manifest with consolidation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestConfig {
    /// Domain name (insurance, medical, legal, finance)
    pub domain: String,

    /// Minimum confidence threshold for entities
    pub confidence_threshold: f32,

    /// Jaccard similarity threshold for deduplication
    pub dedup_threshold: f32,

    /// Entity type confidence requirements
    pub entity_thresholds: HashMap<String, f32>,

    /// Language configuration
    pub languages: Vec<String>,

    /// Source mappings
    pub sources: HashMap<String, SourceConfig>,

    /// Custom metadata rules
    pub metadata_rules: HashMap<String, String>,
}

/// Source-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Source URL pattern
    pub url_pattern: String,

    /// Rate limit (requests per second)
    pub rate_limit: Option<f32>,

    /// Custom user agent
    pub user_agent: Option<String>,

    /// Domain-specific rules
    pub rules: HashMap<String, String>,
}

impl Default for ManifestConfig {
    fn default() -> Self {
        let mut entity_thresholds = HashMap::new();
        entity_thresholds.insert("product".to_string(), 0.85);
        entity_thresholds.insert("coverage".to_string(), 0.80);
        entity_thresholds.insert("exclusion".to_string(), 0.75);
        entity_thresholds.insert("condition".to_string(), 0.70);

        Self {
            domain: "insurance".to_string(),
            confidence_threshold: 0.72,
            dedup_threshold: 0.95,
            entity_thresholds,
            languages: vec!["en".to_string(), "th".to_string()],
            sources: HashMap::new(),
            metadata_rules: HashMap::new(),
        }
    }
}

impl ManifestConfig {
    /// Load manifest from JSON file
    pub fn from_file(path: &str) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save manifest to JSON file
    pub fn to_file(&self, path: &str) -> crate::error::Result<()> {
        let content = serde_json::to_string_pretty(&self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get confidence threshold for entity type
    pub fn get_entity_threshold(&self, entity_type: &str) -> f32 {
        self.entity_thresholds
            .get(entity_type)
            .copied()
            .unwrap_or(self.confidence_threshold)
    }

    /// Create insurance domain config
    pub fn insurance() -> Self {
        let mut config = Self::default();
        config.domain = "insurance".to_string();

        // Add common insurance sources
        config.sources.insert(
            "prudential".to_string(),
            SourceConfig {
                url_pattern: "prudential.co.th/*".to_string(),
                rate_limit: Some(0.5), // 2 second delays
                user_agent: Some("Mozilla/5.0 (RefGraph/1.0)".to_string()),
                rules: HashMap::new(),
            },
        );

        config.sources.insert(
            "axa".to_string(),
            SourceConfig {
                url_pattern: "axa.co.th/*".to_string(),
                rate_limit: Some(0.5),
                user_agent: Some("Mozilla/5.0 (RefGraph/1.0)".to_string()),
                rules: HashMap::new(),
            },
        );

        config
    }

    /// Create medical domain config
    pub fn medical() -> Self {
        let mut config = Self::default();
        config.domain = "medical".to_string();
        config.languages = vec!["en".to_string()];

        let mut medical_thresholds = HashMap::new();
        medical_thresholds.insert("symptom".to_string(), 0.80);
        medical_thresholds.insert("treatment".to_string(), 0.85);
        medical_thresholds.insert("diagnosis".to_string(), 0.90);

        config.entity_thresholds = medical_thresholds;
        config
    }

    /// Validate configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.domain.is_empty() {
            return Err(crate::error::Error::ConfigError("Domain cannot be empty".to_string()));
        }

        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            return Err(crate::error::Error::ConfigError(
                "Confidence threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        if !(0.0..=1.0).contains(&self.dedup_threshold) {
            return Err(crate::error::Error::ConfigError(
                "Dedup threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ManifestConfig::default();
        assert_eq!(config.domain, "insurance");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_insurance_config() {
        let config = ManifestConfig::insurance();
        assert_eq!(config.domain, "insurance");
        assert_eq!(config.languages.len(), 2);
        assert!(config.sources.contains_key("prudential"));
    }

    #[test]
    fn test_medical_config() {
        let config = ManifestConfig::medical();
        assert_eq!(config.domain, "medical");
        assert_eq!(config.languages, vec!["en"]);
    }

    #[test]
    fn test_get_entity_threshold() {
        let config = ManifestConfig::default();
        assert_eq!(config.get_entity_threshold("product"), 0.85);
        assert_eq!(config.get_entity_threshold("coverage"), 0.80);
        assert_eq!(config.get_entity_threshold("unknown"), 0.72); // default
    }

    #[test]
    fn test_validation() {
        let mut config = ManifestConfig::default();
        assert!(config.validate().is_ok());

        config.confidence_threshold = 1.5; // Invalid
        assert!(config.validate().is_err());
    }
}
