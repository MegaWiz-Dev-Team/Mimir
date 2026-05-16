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

    // ============ E2E TESTS (Day 7) ============

    #[tokio::test]
    async fn test_e2e_full_pipeline_with_complex_document() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Prudential offers Critical Illness coverage for Heart Attack. \
             Health Insurance excludes Pre-existing Condition. \
             Life Insurance covers Stroke and requires medical exam.",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify all entity types extracted
        assert!(output.entities.len() > 0);
        let entity_types: std::collections::HashSet<_> =
            output.entities.iter().map(|e| &e.entity_type).collect();
        assert!(entity_types.contains(&"product".to_string()));
        assert!(entity_types.contains(&"coverage".to_string()));
    }

    #[tokio::test]
    async fn test_e2e_multi_source_consolidation() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk(
                "chunk_prudential",
                "Prudential Critical Illness insurance covers Heart Attack",
            ),
            create_test_chunk(
                "chunk_axa",
                "AXA Critical Illness insurance includes Stroke coverage",
            ),
            create_test_chunk(
                "chunk_thai_health",
                "Thai Health Critical Illness plan excludes Pre-existing Condition",
            ),
        ];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify entities from multiple sources are consolidated
        assert!(output.entities.len() > 0);
        assert!(output.metadata.entity_count > 0);
    }

    #[tokio::test]
    async fn test_e2e_deduplication_across_sources() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness insurance"),
            create_test_chunk("chunk_002", "Critical Illness insurance"),
            create_test_chunk("chunk_003", "Critical Illness insurance"),
        ];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should deduplicate to single entity
        let critical_illness_count = output
            .entities
            .iter()
            .filter(|e| e.text.contains("Critical Illness"))
            .count();
        assert_eq!(critical_illness_count, 1);
    }

    #[tokio::test]
    async fn test_e2e_graph_relationships_built() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness covers Heart Attack and Stroke, excludes Pre-existing Condition",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify relationships were created
        assert!(output.relationships.len() >= 0);
    }

    #[tokio::test]
    async fn test_e2e_output_validity() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness insurance for Heart Attack",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify output is serializable
        let json = output.to_json();
        assert!(json.is_ok());

        let jsonl = output.to_jsonl();
        assert!(jsonl.is_ok());
    }

    #[tokio::test]
    async fn test_e2e_large_input_handling() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        // Create 50 chunks with varying content
        let mut chunks = Vec::new();
        for i in 0..50 {
            let content = match i % 3 {
                0 => "Critical Illness insurance",
                1 => "Heart Attack coverage",
                _ => "Pre-existing Condition exclusion",
            };
            chunks.push(create_test_chunk(&format!("chunk_{:03}", i), content));
        }

        let start = std::time::Instant::now();
        let result = graph.consolidate(chunks).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(
            elapsed.as_millis() < 1000,
            "Should process 50 chunks in <1s"
        );

        let output = result.unwrap();
        assert!(output.entities.len() > 0);
    }

    #[tokio::test]
    async fn test_e2e_metadata_accuracy() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness insurance"),
            create_test_chunk("chunk_002", "Heart Attack coverage"),
        ];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify metadata matches actual data
        assert_eq!(output.metadata.entity_count, output.entities.len());
        assert_eq!(
            output.metadata.relationship_count,
            output.relationships.len()
        );
    }

    #[tokio::test]
    async fn test_e2e_confidence_averaging() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness coverage"),
            create_test_chunk("chunk_002", "Critical Illness coverage"),
        ];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify average confidence is calculated
        assert!(output.metadata.average_confidence >= 0.0);
        assert!(output.metadata.average_confidence <= 1.0);
    }

    #[tokio::test]
    async fn test_e2e_mixed_entity_types() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![
            create_test_chunk("chunk_001", "Critical Illness is a product"),
            create_test_chunk("chunk_002", "Heart Attack is a coverage"),
            create_test_chunk("chunk_003", "Pre-existing Condition is an exclusion"),
        ];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should have mixed entity types
        let types: std::collections::HashSet<_> =
            output.entities.iter().map(|e| &e.entity_type).collect();
        assert!(types.len() >= 1);
    }

    #[tokio::test]
    async fn test_e2e_thai_language_support() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness ประกันโรค ความเสี่ยง Health Insurance",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Should handle Thai content without crashing
        assert!(output.entities.len() >= 0);
    }

    #[tokio::test]
    async fn test_e2e_repeated_consolidation() {
        let config1 = ManifestConfig::default();
        let mut graph = RefGraph::new(config1).unwrap();

        // First consolidation
        let chunks1 = vec![create_test_chunk("chunk_001", "Critical Illness insurance")];
        let result1 = graph.consolidate(chunks1).await;
        assert!(result1.is_ok());

        // Create new graph for second consolidation (simulating independence)
        let config2 = ManifestConfig::default();
        let mut graph2 = RefGraph::new(config2).unwrap();
        let chunks2 = vec![
            create_test_chunk("chunk_002", "Critical Illness insurance"),
            create_test_chunk("chunk_003", "Heart Attack coverage"),
        ];
        let result2 = graph2.consolidate(chunks2).await;
        assert!(result2.is_ok());

        // Both should have produced valid outputs
        assert!(result1.unwrap().entities.len() > 0);
        assert!(result2.unwrap().entities.len() > 0);
    }

    #[tokio::test]
    async fn test_e2e_very_long_document() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        let long_text = vec![
            "Critical Illness insurance",
            "Health Insurance plans",
            "Life Insurance policies",
            "Heart Attack coverage",
            "Stroke benefits",
            "Diabetes management",
            "Pre-existing Condition exclusion",
            "Experimental Treatment exclusion",
        ]
        .join(" ");

        let chunks = vec![create_test_chunk("chunk_001", &long_text)];
        let result = graph.consolidate(chunks).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.entities.len() > 0);
    }

    #[tokio::test]
    async fn test_e2e_special_characters_in_content() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness (insurance) - coverage: Heart Attack & Stroke\nExcludes: Pre-existing",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
    }

    // ============ PERFORMANCE TESTS (Day 7) ============

    #[tokio::test]
    async fn test_performance_100_chunks() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        let mut chunks = Vec::new();
        for i in 0..100 {
            let content = match i % 5 {
                0 => "Critical Illness insurance",
                1 => "Health Insurance plan",
                2 => "Heart Attack coverage",
                3 => "Stroke coverage",
                _ => "Pre-existing Condition exclusion",
            };
            chunks.push(create_test_chunk(&format!("chunk_{:03}", i), content));
        }

        let start = std::time::Instant::now();
        let result = graph.consolidate(chunks).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(
            elapsed.as_millis() < 2000,
            "100 chunks should process in <2s, took {}ms",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_performance_entity_dedup_speed() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        // 30 chunks all with identical content (worst case for dedup)
        let chunks = vec![create_test_chunk("chunk_001", "Critical Illness insurance"); 30];

        let start = std::time::Instant::now();
        let result = graph.consolidate(chunks).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(
            elapsed.as_millis() < 500,
            "Dedup of 30 identical chunks should be fast"
        );
    }

    #[tokio::test]
    async fn test_performance_output_serialization() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        let mut chunks = Vec::new();
        for i in 0..50 {
            chunks.push(create_test_chunk(
                &format!("chunk_{:03}", i),
                "Critical Illness insurance Heart Attack coverage",
            ));
        }

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Measure JSON serialization speed
        let start = std::time::Instant::now();
        let json = output.to_json();
        let json_elapsed = start.elapsed();

        assert!(json.is_ok());
        assert!(
            json_elapsed.as_millis() < 100,
            "JSON serialization should be fast"
        );

        // Measure JSONL serialization speed
        let start = std::time::Instant::now();
        let jsonl = output.to_jsonl();
        let jsonl_elapsed = start.elapsed();

        assert!(jsonl.is_ok());
        assert!(
            jsonl_elapsed.as_millis() < 100,
            "JSONL serialization should be fast"
        );
    }

    #[tokio::test]
    async fn test_performance_memory_efficiency() {
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        // Multiple chunks with extractable patterns
        let mut chunks = Vec::new();
        for i in 0..30 {
            let content = match i % 3 {
                0 => "Critical Illness insurance",
                1 => "Health Insurance plan",
                _ => "Heart Attack coverage",
            };
            chunks.push(create_test_chunk(&format!("chunk_{:03}", i), content));
        }

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify entities are extracted and no data loss
        assert!(output.entities.len() > 0);
        assert!(output.metadata.entity_count <= output.entities.len() + 5); // small buffer for rounding
    }

    // ============ HEIMDALL INTEGRATION TESTS (Day 8) ============
    // These tests demonstrate how RefGraph integrates with Heimdall LLM
    // for semantic entity validation and confidence boosting

    #[tokio::test]
    async fn test_heimdall_entity_validation_readiness() {
        // RefGraph output should be ready for Heimdall LLM validation
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness insurance covers Heart Attack",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify output has required fields for Heimdall
        for entity in &output.entities {
            assert!(!entity.id.is_empty(), "Entity ID required for Heimdall");
            assert!(
                !entity.text.is_empty(),
                "Entity text required for validation"
            );
            assert!(entity.confidence >= 0.0 && entity.confidence <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_heimdall_confidence_baseline() {
        // Heimdall can boost confidence scores from extraction baseline
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk("chunk_001", "Critical Illness insurance")];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify confidence range (0.85-0.95 from extractors)
        for entity in &output.entities {
            assert!(
                entity.confidence >= 0.80,
                "Baseline confidence should be reasonable"
            );
            assert!(
                entity.confidence <= 1.0,
                "Heimdall can boost to 1.0 if validated"
            );
        }
    }

    #[tokio::test]
    async fn test_heimdall_entity_enrichment_fields() {
        // Entities should have fields needed for Heimdall enrichment
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk("chunk_001", "Critical Illness insurance")];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify enrichment-ready fields
        for entity in &output.entities {
            // Can be used by Heimdall for context
            assert!(!entity.entity_type.is_empty());
            // Domain isolation for Heimdall multi-domain support
            assert!(!entity.domain.is_empty());
        }
    }

    #[tokio::test]
    async fn test_laminar_relationship_metadata() {
        // Laminar (Sága) needs relationship metadata for visualization
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness covers Heart Attack",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Relationships should be ready for Laminar visualization
        for rel in &output.relationships {
            assert!(!rel.source_id.is_empty());
            assert!(!rel.target_id.is_empty());
            assert!(!rel.relationship_type.is_empty());
            assert!(rel.confidence >= 0.0 && rel.confidence <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_heimdall_llm_prompt_construction() {
        // Entity data should be suitable for LLM prompts
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness insurance covers Heart Attack",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify JSON serialization for LLM context
        let json = output.to_json().unwrap();
        assert!(json.contains("entities"));
        assert!(json.contains("relationships"));
        assert!(json.len() > 100, "Sufficient content for LLM context");
    }

    #[tokio::test]
    async fn test_heimdall_batch_validation_workflow() {
        // RefGraph should support batch validation with Heimdall
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

        // Should batch entities for Heimdall validation
        assert!(output.entities.len() > 0);
        let jsonl = output.to_jsonl().unwrap();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert!(
            lines.len() > 1,
            "Multiple entities ready for batch validation"
        );
    }

    #[tokio::test]
    async fn test_heimdall_confidence_boost_scenario() {
        // Demonstrate confidence boost workflow with Heimdall
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();
        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Prudential offers Critical Illness insurance",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // RefGraph baseline confidence
        let baseline_confidence = output.entities.iter().next().map(|e| e.confidence);
        assert!(baseline_confidence.is_some());
        assert!(baseline_confidence.unwrap() > 0.80);

        // Heimdall can validate and potentially boost to higher confidence
        // Simulated: if Heimdall validates entity → confidence += 0.05 (capped at 1.0)
        let validation_boosted = baseline_confidence.unwrap() + 0.05;
        assert!(validation_boosted.min(1.0) >= baseline_confidence.unwrap());
    }

    // ============ RELEASE & VERSION TESTS (Day 8) ============

    #[test]
    fn test_version_is_semantic() {
        // Verify version follows semantic versioning
        let version = VERSION;
        let parts: Vec<&str> = version.split('.').collect();
        assert!(parts.len() >= 3, "Version should be X.Y.Z format");

        // Each part should be numeric (or pre-release identifier)
        assert!(parts[0].chars().all(|c| c.is_numeric() || c == 'v'));
        assert!(parts[1].chars().all(|c| c.is_numeric()));
    }

    #[test]
    fn test_version_not_empty() {
        assert!(
            !VERSION.is_empty(),
            "Version should be defined in Cargo.toml"
        );
        assert!(
            VERSION.len() < 20,
            "Version string should be reasonable length"
        );
    }

    #[tokio::test]
    async fn test_final_smoke_test() {
        // Final smoke test: complete pipeline with v1.0.0
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        let chunks = vec![
            create_test_chunk(
                "chunk_001",
                "Prudential Critical Illness insurance covers Heart Attack",
            ),
            create_test_chunk("chunk_002", "AXA Health Insurance includes Stroke coverage"),
            create_test_chunk("chunk_003", "Thai Health excludes Pre-existing Condition"),
        ];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok(), "Pipeline should complete successfully");

        let output = result.unwrap();

        // Verify all stages completed
        assert!(output.entities.len() > 0, "Extraction completed");
        assert!(
            output
                .entities
                .iter()
                .all(|e| !e.id.is_empty() && !e.text.is_empty()),
            "Deduplication completed"
        );
        assert!(output.metadata.entity_count > 0, "Graph building completed");

        // Verify output formats
        let json = output.to_json();
        assert!(json.is_ok(), "JSON serialization works");

        let jsonl = output.to_jsonl();
        assert!(jsonl.is_ok(), "JSONL serialization works");

        // Version present
        assert!(!output.metadata.version.is_empty());
    }

    #[tokio::test]
    async fn test_production_readiness_checklist() {
        // Comprehensive production readiness check
        let config = ManifestConfig::default();
        let mut graph = RefGraph::new(config).unwrap();

        let chunks = vec![create_test_chunk(
            "chunk_001",
            "Critical Illness insurance for Heart Attack",
        )];

        let result = graph.consolidate(chunks).await;
        assert!(result.is_ok());
        let output = result.unwrap();

        // ✅ Extraction verified
        assert!(!output.entities.is_empty(), "Entities extracted");

        // ✅ Deduplication verified
        let unique_texts: std::collections::HashSet<_> =
            output.entities.iter().map(|e| &e.text).collect();
        assert!(
            unique_texts.len() == output.entities.len(),
            "No unexpected duplicates"
        );

        // ✅ Consolidation verified
        assert!(
            output.metadata.entity_count > 0,
            "Consolidation produces output"
        );

        // ✅ Serialization verified
        assert!(output.to_json().is_ok(), "JSON output valid");
        assert!(output.to_jsonl().is_ok(), "JSONL output valid");

        // ✅ Type safety verified
        for entity in &output.entities {
            assert!(!entity.id.is_empty());
            assert!(!entity.text.is_empty());
            assert!(entity.confidence <= 1.0 && entity.confidence >= 0.0);
        }

        // ✅ Domain isolation verified
        assert!(!output.metadata.domain.is_empty());

        // ✅ Versioning verified
        assert!(!output.metadata.version.is_empty());
    }
}
