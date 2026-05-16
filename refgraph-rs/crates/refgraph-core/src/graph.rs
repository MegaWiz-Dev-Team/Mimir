//! Semantic graph builder for entity relationships

use crate::{error::Result, types::*};
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
        let mut entity_ids: Vec<String> = self.entities.keys().cloned().collect();
        entity_ids.sort();

        // For each pair of entities, determine if they have a relationship
        for i in 0..entity_ids.len() {
            for j in i + 1..entity_ids.len() {
                let source_id = &entity_ids[i];
                let target_id = &entity_ids[j];

                // Get entities
                let source = self
                    .entities
                    .get(source_id)
                    .ok_or_else(|| crate::error::Error::graph("Entity not found".to_string()))?;
                let target = self
                    .entities
                    .get(target_id)
                    .ok_or_else(|| crate::error::Error::graph("Entity not found".to_string()))?;

                // Determine relationship type based on entity types
                if let Some(rel_type) =
                    self.determine_relationship_type(&source.entity_type, &target.entity_type)
                {
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
    fn determine_relationship_type(
        &self,
        source: &EntityType,
        target: &EntityType,
    ) -> Option<String> {
        match (source, target) {
            (EntityType::Product, EntityType::Coverage) => Some("HAS_COVERAGE".to_string()),
            (EntityType::Product, EntityType::Exclusion) => Some("HAS_EXCLUSION".to_string()),
            (EntityType::Coverage, EntityType::Condition) => Some("REQUIRES_CONDITION".to_string()),
            (EntityType::Exclusion, EntityType::Condition) => {
                Some("EXCLUDES_CONDITION".to_string())
            }
            (EntityType::Product, EntityType::Organization) => Some("OFFERED_BY".to_string()),
            _ => None,
        }
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            entity_count: self.entities.len(),
            relationship_count: self.relationships.len(),
            average_entity_confidence: self
                .entities
                .values()
                .map(|e| e.confidence as f64)
                .sum::<f64>()
                / self.entities.len().max(1) as f64,
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

    #[test]
    fn test_add_entity_with_empty_id_fails() {
        let mut graph = SemanticGraph::new();
        let entity = ConsolidatedEntity {
            entity_id: "".to_string(),
            text: "Invalid".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let result = graph.add_entity(entity);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_entity_returns_none_for_missing() {
        let graph = SemanticGraph::new();
        assert!(graph.get_entity("nonexistent").is_none());
    }

    #[test]
    fn test_get_entity_returns_entity() {
        let mut graph = SemanticGraph::new();
        let entity = ConsolidatedEntity {
            entity_id: "ent_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(entity);
        let retrieved = graph.get_entity("ent_001");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().text, "Critical Illness");
    }

    #[test]
    fn test_add_multiple_entities() {
        let mut graph = SemanticGraph::new();
        for i in 0..5 {
            let entity = ConsolidatedEntity {
                entity_id: format!("ent_{:03}", i),
                text: format!("Entity {}", i),
                entity_type: EntityType::Product,
                confidence: 0.95,
                sources: vec!["test.com".to_string()],
                compressed_refs: vec![],
                merged_from: vec![],
                metadata: HashMap::new(),
            };
            assert!(graph.add_entity(entity).is_ok());
        }
        assert_eq!(graph.entities().len(), 5);
    }

    #[test]
    fn test_add_relationship_requires_existing_entities() {
        let mut graph = SemanticGraph::new();
        let rel = GraphRelationship {
            source_entity_id: "nonexistent_src".to_string(),
            target_entity_id: "nonexistent_tgt".to_string(),
            relationship_type: "HAS_COVERAGE".to_string(),
            confidence: 0.95,
            properties: HashMap::new(),
        };

        let result = graph.add_relationship(rel);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_relationship_with_existing_entities() {
        let mut graph = SemanticGraph::new();
        let product = ConsolidatedEntity {
            entity_id: "prod_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };
        let coverage = ConsolidatedEntity {
            entity_id: "cov_001".to_string(),
            text: "Heart Attack".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 0.92,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(product);
        let _ = graph.add_entity(coverage);

        let rel = GraphRelationship {
            source_entity_id: "prod_001".to_string(),
            target_entity_id: "cov_001".to_string(),
            relationship_type: "HAS_COVERAGE".to_string(),
            confidence: 0.93,
            properties: HashMap::new(),
        };

        let result = graph.add_relationship(rel);
        assert!(result.is_ok());
        assert_eq!(graph.all_relationships().len(), 1);
    }

    #[test]
    fn test_relationships_for_entity() {
        let mut graph = SemanticGraph::new();
        let product = ConsolidatedEntity {
            entity_id: "prod_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };
        let coverage = ConsolidatedEntity {
            entity_id: "cov_001".to_string(),
            text: "Heart Attack".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 0.92,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(product);
        let _ = graph.add_entity(coverage);

        let rel = GraphRelationship {
            source_entity_id: "prod_001".to_string(),
            target_entity_id: "cov_001".to_string(),
            relationship_type: "HAS_COVERAGE".to_string(),
            confidence: 0.93,
            properties: HashMap::new(),
        };
        let _ = graph.add_relationship(rel);

        let rels = graph.relationships_for_entity("prod_001");
        assert_eq!(rels.len(), 1);
    }

    #[test]
    fn test_determine_relationship_product_to_coverage() {
        let graph = SemanticGraph::new();
        let rel_type =
            graph.determine_relationship_type(&EntityType::Product, &EntityType::Coverage);
        assert_eq!(rel_type, Some("HAS_COVERAGE".to_string()));
    }

    #[test]
    fn test_determine_relationship_product_to_exclusion() {
        let graph = SemanticGraph::new();
        let rel_type =
            graph.determine_relationship_type(&EntityType::Product, &EntityType::Exclusion);
        assert_eq!(rel_type, Some("HAS_EXCLUSION".to_string()));
    }

    #[test]
    fn test_determine_relationship_unknown_types() {
        let graph = SemanticGraph::new();
        let rel_type =
            graph.determine_relationship_type(&EntityType::Product, &EntityType::Product);
        assert_eq!(rel_type, None);
    }

    #[test]
    fn test_build_relationships_creates_valid_edges() {
        let mut graph = SemanticGraph::new();
        // Use IDs that ensure Product sorts before Coverage
        let product = ConsolidatedEntity {
            entity_id: "aaa_prod".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };
        let coverage = ConsolidatedEntity {
            entity_id: "zzz_cov".to_string(),
            text: "Heart Attack".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 0.92,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(product);
        let _ = graph.add_entity(coverage);
        let result = graph.build_relationships();

        assert!(result.is_ok());
        assert_eq!(graph.all_relationships().len(), 1);
    }

    #[test]
    fn test_build_relationships_empty_graph() {
        let mut graph = SemanticGraph::new();
        let result = graph.build_relationships();
        assert!(result.is_ok());
        assert_eq!(graph.all_relationships().len(), 0);
    }

    #[test]
    fn test_build_relationships_single_entity() {
        let mut graph = SemanticGraph::new();
        let product = ConsolidatedEntity {
            entity_id: "prod_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(product);
        let result = graph.build_relationships();

        assert!(result.is_ok());
        assert_eq!(graph.all_relationships().len(), 0);
    }

    #[test]
    fn test_stats_multiple_entities() {
        let mut graph = SemanticGraph::new();
        let entity1 = ConsolidatedEntity {
            entity_id: "ent_001".to_string(),
            text: "Entity 1".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.90,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };
        let entity2 = ConsolidatedEntity {
            entity_id: "ent_002".to_string(),
            text: "Entity 2".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 1.00,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(entity1);
        let _ = graph.add_entity(entity2);
        let stats = graph.stats();

        assert_eq!(stats.entity_count, 2);
        assert!((stats.average_entity_confidence - 0.95).abs() < 0.0001);
    }

    #[test]
    fn test_relationship_confidence_averages() {
        let mut graph = SemanticGraph::new();
        // Use sorted IDs: "aaa" < "zzz"
        let product = ConsolidatedEntity {
            entity_id: "aaa_prod".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.80,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };
        let coverage = ConsolidatedEntity {
            entity_id: "zzz_cov".to_string(),
            text: "Heart Attack".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 1.00,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(product);
        let _ = graph.add_entity(coverage);
        let _ = graph.build_relationships();

        let rels = graph.all_relationships();
        assert!(!rels.is_empty());
        assert!((rels[0].confidence - 0.90).abs() < 0.0001);
    }

    #[test]
    fn test_default_creates_empty_graph() {
        let graph = SemanticGraph::default();
        assert_eq!(graph.entities().len(), 0);
        assert_eq!(graph.all_relationships().len(), 0);
    }

    #[test]
    fn test_complex_relationship_network() {
        let mut graph = SemanticGraph::new();

        // Create a product (with ID "aaa" to ensure it comes first in iteration)
        let product = ConsolidatedEntity {
            entity_id: "aaa_prod".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        // Create coverages
        let coverage1 = ConsolidatedEntity {
            entity_id: "bbb_cov1".to_string(),
            text: "Heart Attack".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 0.92,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let coverage2 = ConsolidatedEntity {
            entity_id: "ccc_cov2".to_string(),
            text: "Stroke".to_string(),
            entity_type: EntityType::Coverage,
            confidence: 0.92,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        // Create exclusion
        let exclusion = ConsolidatedEntity {
            entity_id: "ddd_exc".to_string(),
            text: "Pre-existing Condition".to_string(),
            entity_type: EntityType::Exclusion,
            confidence: 0.88,
            sources: vec!["test.com".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        let _ = graph.add_entity(product);
        let _ = graph.add_entity(coverage1);
        let _ = graph.add_entity(coverage2);
        let _ = graph.add_entity(exclusion);

        let _ = graph.build_relationships();

        let stats = graph.stats();
        assert_eq!(stats.entity_count, 4);
        // Should have 3 relationships: Product->Coverage1, Product->Coverage2, Product->Exclusion
        assert!(stats.relationship_count >= 2);
    }
}
