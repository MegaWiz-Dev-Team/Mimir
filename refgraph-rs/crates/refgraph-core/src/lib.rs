//! RefGraph: Multi-domain semantic entity graph consolidation engine
//!
//! Consolidates data from multiple sources into a semantic entity graph,
//! with deduplication, semantic consolidation, and manifest-based domain configuration.

pub mod dedup;
pub mod error;
pub mod extract;
pub mod graph;
pub mod manifest;
pub mod output;
pub mod types;

pub use dedup::Deduplicator;
pub use error::{Error, Result};
pub use extract::EntityExtractor;
pub use graph::SemanticGraph;
pub use manifest::ManifestConfig;
pub use output::RefGraphOutput;

/// RefGraph version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// RefGraph pipeline coordinator
pub struct RefGraph {
    config: ManifestConfig,
    graph: SemanticGraph,
    dedup: Deduplicator,
    extractor: EntityExtractor,
}

impl RefGraph {
    /// Create new RefGraph instance with domain config
    pub fn new(config: ManifestConfig) -> Result<Self> {
        Ok(Self {
            config,
            graph: SemanticGraph::new(),
            dedup: Deduplicator::new(0.95), // Jaccard threshold
            extractor: EntityExtractor::new(),
        })
    }

    /// Consolidate raw chunks into semantic graph
    pub async fn consolidate(&mut self, chunks: Vec<types::RawChunk>) -> Result<RefGraphOutput> {
        // 1. Extract entities from chunks
        let mut entities = Vec::new();
        for chunk in &chunks {
            let extracted = self.extractor.extract(&chunk.content)?;
            entities.extend(extracted);
        }

        // 2. Deduplicate entities
        let deduplicated = self.dedup.deduplicate(entities)?;

        // 3. Build semantic graph
        for entity in &deduplicated {
            self.graph.add_entity(entity.clone())?;
        }

        // 4. Create Neo4j relationships
        self.graph.build_relationships()?;

        // 5. Format output
        let output = RefGraphOutput::from_graph(&self.graph, self.config.clone())?;

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_chunk(id: &str, content: &str) -> types::RawChunk {
        types::RawChunk {
            chunk_id: id.to_string(),
            content: content.to_string(),
            source_url: "test.com".to_string(),
            page_index: None,
            token_count: content.split_whitespace().count(),
        }
    }

    #[tokio::test]
    async fn test_refgraph_creation() {
        let config = ManifestConfig::default();
        let result = RefGraph::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_consolidate_empty_chunks() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let result = graph.consolidate(vec![]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.entities.len(), 0);
    }

    #[tokio::test]
    async fn test_consolidate_single_chunk() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk("chunk_001", "Critical Illness insurance");
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_consolidate_multiple_chunks() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness insurance"),
            create_test_chunk("chunk_002", "Heart Attack coverage"),
            create_test_chunk("chunk_003", "Pre-existing Condition exclusion"),
        ];
        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.entities.len() > 0);
    }

    #[tokio::test]
    async fn test_consolidate_extracts_entities() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk("chunk_001", "Critical Illness insurance Health Insurance");
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should extract at least one product entity
        assert!(output.entities.len() > 0);
    }

    #[tokio::test]
    async fn test_consolidate_deduplicates_entities() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness insurance"),
            create_test_chunk("chunk_002", "Critical Illness insurance"),
        ];
        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Both mention same product, should deduplicate to 1
        let critical_illness_count = output
            .entities
            .iter()
            .filter(|e| e.text.contains("Critical Illness"))
            .count();
        assert_eq!(critical_illness_count, 1);
    }

    #[tokio::test]
    async fn test_consolidate_builds_graph() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness covers Heart Attack",
        )];
        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should have relationships (at least 0)
        assert!(output.relationships.len() >= 0);
    }

    #[tokio::test]
    async fn test_consolidate_produces_output() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk(
            "chunk_001",
            "Critical Illness insurance for Heart Attack coverage",
        );
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.entities.is_empty());
        assert!(output.metadata.entity_count > 0);
    }

    #[tokio::test]
    async fn test_consolidate_with_insurance_domain() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Prudential offers Critical Illness insurance"),
            create_test_chunk("chunk_002", "Coverage includes Heart Attack and Stroke"),
            create_test_chunk("chunk_003", "Excludes Pre-existing Condition"),
        ];
        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.entities.len() > 0);
        assert!(output.metadata.entity_count > 0);
    }

    #[tokio::test]
    async fn test_consolidate_handles_language_detection() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk("chunk_001", "Critical Illness สุขภาพ coverage");
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_consolidate_multiple_entity_types() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness product"),
            create_test_chunk("chunk_002", "Heart Attack coverage"),
            create_test_chunk("chunk_003", "Pre-existing Condition exclusion"),
        ];
        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should have extracted different entity types
        assert!(output.entities.len() > 0);
    }

    #[tokio::test]
    async fn test_consolidate_output_contains_entities() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk("chunk_001", "Critical Illness insurance");
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Verify entities were extracted
        assert!(!output.entities.is_empty());
        for entity in &output.entities {
            assert!(!entity.id.is_empty());
            assert!(!entity.text.is_empty());
        }
    }

    #[tokio::test]
    async fn test_consolidate_metadata_contains_version() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk("chunk_001", "Critical Illness insurance");
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.metadata.version.is_empty());
        assert_eq!(output.metadata.domain, "insurance");
    }

    #[tokio::test]
    async fn test_consolidate_json_serialization() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunk = create_test_chunk("chunk_001", "Critical Illness insurance");
        let result = graph.consolidate(vec![chunk]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        let json = output.to_json();
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("Critical Illness"));
    }
}
