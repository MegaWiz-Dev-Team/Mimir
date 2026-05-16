//! Mimir output formatter for RAG ingestion

use crate::{error::Result, graph::SemanticGraph, manifest::ManifestConfig};
use serde::{Deserialize, Serialize};

/// Output format for Mimir RAG ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefGraphOutput {
    /// Consolidated entities ready for embedding
    pub entities: Vec<RefGraphEntity>,

    /// Neo4j relationships for graph ingestion
    pub relationships: Vec<RefGraphRelationship>,

    /// Metadata about the consolidation
    pub metadata: ConsolidationMetadata,
}

/// Entity formatted for Mimir
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefGraphEntity {
    pub id: String,
    pub text: String,
    pub entity_type: String,
    pub confidence: f32,
    pub sources: Vec<String>,
    pub merged_from: Vec<String>,
    pub compressed_refs: Vec<String>, // JSON-serialized refs
    pub domain: String,
}

/// Relationship formatted for Neo4j
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefGraphRelationship {
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: String,
    pub confidence: f32,
    pub properties: std::collections::HashMap<String, String>,
}

/// Consolidation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationMetadata {
    pub domain: String,
    pub timestamp: String,
    pub entity_count: usize,
    pub relationship_count: usize,
    pub average_confidence: f32,
    pub version: String,
}

impl RefGraphOutput {
    /// Create RefGraphOutput from semantic graph
    pub fn from_graph(
        graph: &SemanticGraph,
        config: ManifestConfig,
    ) -> Result<Self> {
        let timestamp = chrono::Utc::now().to_rfc3339();

        // Convert entities
        let mut entities = Vec::new();
        for entity in graph.entities() {
            let compressed_refs = entity
                .compressed_refs
                .iter()
                .map(|r| serde_json::to_string(r).unwrap_or_default())
                .collect();

            entities.push(RefGraphEntity {
                id: entity.entity_id.clone(),
                text: entity.text.clone(),
                entity_type: entity.entity_type.to_string(),
                confidence: entity.confidence,
                sources: entity.sources.clone(),
                merged_from: entity.merged_from.clone(),
                compressed_refs,
                domain: config.domain.clone(),
            });
        }

        // Convert relationships
        let mut relationships = Vec::new();
        for rel in graph.all_relationships() {
            relationships.push(RefGraphRelationship {
                source_id: rel.source_entity_id.clone(),
                target_id: rel.target_entity_id.clone(),
                relationship_type: rel.relationship_type.clone(),
                confidence: rel.confidence,
                properties: rel.properties.clone(),
            });
        }

        let stats = graph.stats();
        let metadata = ConsolidationMetadata {
            domain: config.domain.clone(),
            timestamp,
            entity_count: stats.entity_count,
            relationship_count: stats.relationship_count,
            average_confidence: stats.average_entity_confidence as f32,
            version: crate::VERSION.to_string(),
        };

        Ok(Self {
            entities,
            relationships,
            metadata,
        })
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Convert to JSONL format (one entity per line)
    pub fn to_jsonl(&self) -> Result<String> {
        let mut lines = Vec::new();

        // Metadata line
        lines.push(serde_json::to_string(&serde_json::json!({
            "type": "metadata",
            "data": self.metadata
        }))?);

        // Entity lines
        for entity in &self.entities {
            lines.push(serde_json::to_string(&serde_json::json!({
                "type": "entity",
                "data": entity
            }))?);
        }

        // Relationship lines
        for rel in &self.relationships {
            lines.push(serde_json::to_string(&serde_json::json!({
                "type": "relationship",
                "data": rel
            }))?);
        }

        Ok(lines.join("\n"))
    }

    /// Save to JSON file
    pub fn save_json(&self, path: &str) -> Result<()> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Save to JSONL file
    pub fn save_jsonl(&self, path: &str) -> Result<()> {
        let jsonl = self.to_jsonl()?;
        std::fs::write(path, jsonl)?;
        Ok(())
    }

    /// Load from JSON file
    pub fn load_json(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let output = serde_json::from_str(&content)?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mimir_entity_creation() {
        let entity = RefGraphEntity {
            id: "ent_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: "product".to_string(),
            confidence: 0.95,
            sources: vec!["prudential.co.th".to_string()],
            merged_from: vec![],
            compressed_refs: vec![],
            domain: "insurance".to_string(),
        };

        assert_eq!(entity.entity_type, "product");
    }

    #[test]
    fn test_to_json() {
        let output = RefGraphOutput {
            entities: vec![],
            relationships: vec![],
            metadata: ConsolidationMetadata {
                domain: "insurance".to_string(),
                timestamp: "2026-05-16T00:00:00Z".to_string(),
                entity_count: 0,
                relationship_count: 0,
                average_confidence: 0.0,
                version: "0.1.0".to_string(),
            },
        };

        let json = output.to_json();
        assert!(json.is_ok());
    }

    #[test]
    fn test_to_jsonl() {
        let output = RefGraphOutput {
            entities: vec![RefGraphEntity {
                id: "ent_001".to_string(),
                text: "Test".to_string(),
                entity_type: "product".to_string(),
                confidence: 0.95,
                sources: vec!["test.com".to_string()],
                merged_from: vec![],
                compressed_refs: vec![],
                domain: "insurance".to_string(),
            }],
            relationships: vec![],
            metadata: ConsolidationMetadata {
                domain: "insurance".to_string(),
                timestamp: "2026-05-16T00:00:00Z".to_string(),
                entity_count: 1,
                relationship_count: 0,
                average_confidence: 0.95,
                version: "0.1.0".to_string(),
            },
        };

        let jsonl = output.to_jsonl();
        assert!(jsonl.is_ok());
        let jsonl_str = jsonl.unwrap();
        let lines: Vec<&str> = jsonl_str.lines().collect();
        assert_eq!(lines.len(), 2); // metadata + entity (no relationships)
    }
}
