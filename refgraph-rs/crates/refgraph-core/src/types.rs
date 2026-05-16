//! RefGraph type definitions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw chunk from web extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawChunk {
    pub chunk_id: String,
    pub content: String,
    pub source_url: String,
    pub page_index: Option<usize>,
    pub token_count: usize,
}

/// Extracted entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: String,
    pub text: String,
    pub entity_type: EntityType,
    pub confidence: f32,
    pub sources: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Entity type classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Product,
    Coverage,
    Exclusion,
    Condition,
    Organization,
    Other,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Product => write!(f, "product"),
            Self::Coverage => write!(f, "coverage"),
            Self::Exclusion => write!(f, "exclusion"),
            Self::Condition => write!(f, "condition"),
            Self::Organization => write!(f, "organization"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: String,
    pub confidence: f32,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Compressed reference (pageIndex + position)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedRef {
    pub source_url: String,
    pub page_index: usize,
    pub token_position: usize,
}

/// Consolidation result for single entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidatedEntity {
    pub entity_id: String,
    pub text: String,
    pub entity_type: EntityType,
    pub confidence: f32,
    pub sources: Vec<String>,
    pub compressed_refs: Vec<CompressedRef>,
    pub merged_from: Vec<String>, // Entity IDs that were merged
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Neo4j relationship for graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelationship {
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relationship_type: String,
    pub confidence: f32,
    pub properties: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_display() {
        assert_eq!(EntityType::Product.to_string(), "product");
        assert_eq!(EntityType::Coverage.to_string(), "coverage");
    }

    #[test]
    fn test_entity_creation() {
        let entity = Entity {
            entity_id: "ent_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["prudential.co.th".to_string()],
            metadata: HashMap::new(),
        };
        assert_eq!(entity.entity_type, EntityType::Product);
    }
}
