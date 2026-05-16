//! RefGraph: Multi-domain semantic entity graph consolidation engine
//!
//! Consolidates data from multiple sources into a semantic entity graph,
//! with deduplication, semantic consolidation, and manifest-based domain configuration.

pub mod error;
pub mod graph;
pub mod dedup;
pub mod manifest;
pub mod extract;
pub mod output;
pub mod types;

pub use error::{Error, Result};
pub use graph::SemanticGraph;
pub use dedup::Deduplicator;
pub use manifest::ManifestConfig;
pub use extract::EntityExtractor;
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

    #[tokio::test]
    async fn test_refgraph_creation() {
        let config = ManifestConfig::default();
        let result = RefGraph::new(config);
        assert!(result.is_ok());
    }
}
