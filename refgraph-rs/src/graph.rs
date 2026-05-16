//! Semantic graph builder for entity relationships

use crate::{types::*, error::Result};
use std::collections::HashMap;

/// Semantic graph for entities and relationships
#[derive(Debug, Clone)]
pub struct SemanticGraph {
    entities: HashMap<String, ConsolidatedEntity>,
    relationships: Vec<GraphRelationship>,
}

impl SemanticGraph {
    /// Create new semantic graph
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            relationships: Vec::new(),
        }
    }

    /// Add entity to graph
    pub fn add_entity(&mut self, entity: ConsolidatedEntity) -> Result<()> {
        if entity.entity_id.is_empty() {
            return Err(crate::error::Error::graph("Entity ID cannot be empty"));
        }
        self.entities.insert(entity.entity_id.clone(), entity);
        Ok(())
    }

    /// Get entity by ID
    pub fn get_entity(&self, id: &str) -> Option<&ConsolidatedEntity> {
        self.entities.get(id)
    }

    /// Get all entities
    pub fn entities(&self) -> Vec<ConsolidatedEntity> {
        self.entities.values().cloned().collect()
    }

    /// Add relationship between entities
    pub fn add_relationship(&mut self, rel: GraphRelationship) -> Result<()> {
        // Validate both entities exist
        if !self.entities.contains_key(&rel.source_entity_id) {
            return Err(crate::error::Error::graph(format!(
                "Source entity {} not found",
                rel.source_entity_id
            )));
        }
        if !self.entities.contains_key(&rel.target_entity_id) {
            return Err(crate::error::Error::graph(format!(
                "Target entity {} not found",
                rel.target_entity_id
            )));
        }
        self.relationships.push(rel);
        Ok(())
    }

    /// Get relationships for entity
    pub fn relationships_for_entity(&self, entity_id: &str) -> Vec<&GraphRelationship> {
        self.relationships
            .iter()
            .filter(|r| r.source_entity_id == entity_id || r.target_entity_id == entity_id)
            .collect()
    }

    /// Get all relationships
    pub fn all_relationships(&self) -> &[GraphRelationship] {
        &self.relationships
    }

    /// Build relationships based on entity co-occurrence and semantic similarity
    pub fn build_relationships(&mut self) -> Result<()> {
        let entity_ids: Vec<String> = self.entities.keys().cloned().collect();

        // For each pair of entities, determine if they have a relationship
        for i in 0..entity_ids.len() {
            for j in i + 1..entity_ids.len() {
                let source_id = &entity_ids[i];
                let target_id = &entity_ids[j];

                // Get entities
                let source = self.entities.get(source_id).ok_or_else(|| {
                    crate::error::Error::graph("Entity not found".to_string())
                })?;
                let target = self.entities.get(target_id).ok_or_else(|| {
                    crate::error::Error::graph("Entity not found".to_string())
                })?;

                // Determine relationship type based on entity types
                if let Some(rel_type) = self.determine_relationship_type(&source.entity_type, &target.entity_type) {
                    let confidence = (source.confidence + target.confidence) / 2.0;

                    let rel = GraphRelationship {
                        source_entity_id: source_id.clone(),
                        target_entity_id: target_id.clone(),
                        relationship_type: rel_type,
                        confidence,
                        properties: HashMap::new(),
                    };

                    self.relationships.push(rel);
                }
            }
        }

        Ok(())
    }

    /// Determine relationship type based on entity types
    fn determine_relationship_type(&self, source: &EntityType, target: &EntityType) -> Option<String> {
        match (source, target) {
            (EntityType::Product, EntityType::Coverage) => Some("HAS_COVERAGE".to_string()),
            (EntityType::Product, EntityType::Exclusion) => Some("HAS_EXCLUSION".to_string()),
            (EntityType::Coverage, EntityType::Condition) => Some("REQUIRES_CONDITION".to_string()),
            (EntityType::Exclusion, EntityType::Condition) => Some("EXCLUDES_CONDITION".to_string()),
            (EntityType::Product, EntityType::Organization) => Some("OFFERED_BY".to_string()),
            _ => None,
        }
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            entity_count: self.entities.len(),
            relationship_count: self.relationships.len(),
            average_entity_confidence: self.entities
                .values()
                .map(|e| e.confidence as f64)
                .sum::<f64>() / self.entities.len().max(1) as f64,
        }
    }
}

impl Default for SemanticGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Graph statistics
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub entity_count: usize,
    pub relationship_count: usize,
    pub average_entity_confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_creation() {
        let graph = SemanticGraph::new();
        assert_eq!(graph.entities().len(), 0);
    }

    #[test]
    fn test_add_entity() {
        let mut graph = SemanticGraph::new();
        let entity = ConsolidatedEntity {
            entity_id: "ent_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["prudential.co.th".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let result = graph.add_entity(entity.clone());
        assert!(result.is_ok());
        assert_eq!(graph.entities().len(), 1);
    }

    #[test]
    fn test_graph_stats() {
        let mut graph = SemanticGraph::new();
        let entity = ConsolidatedEntity {
            entity_id: "ent_001".to_string(),
            text: "Test".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.9,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(entity);
        let stats = graph.stats();
        assert_eq!(stats.entity_count, 1);
        assert_eq!(stats.relationship_count, 0);
    }
}
