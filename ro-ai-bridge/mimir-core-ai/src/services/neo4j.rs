//! Neo4j Knowledge Graph Service — Sprint 17
//!
//! Provides graph operations for entity/relation storage via Neo4j.
//! All queries enforce tenant isolation with `tenant_id` parameter.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for connecting to Neo4j.
#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
}

impl Neo4jConfig {
    /// Load configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        Self {
            uri: env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string()),
            user: env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string()),
            password: env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "mimir_neo4j_password".to_string()),
        }
    }
}

/// A graph entity (node).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntity {
    pub id: Option<i64>,
    pub name: String,
    pub entity_type: String,
    pub properties: Option<Value>,
    pub tenant_id: String,
    pub source_id: Option<i64>,
    pub chunk_id: Option<i64>,
    pub neo4j_node_id: Option<String>,
}

/// A graph relation (edge).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelation {
    pub id: Option<i64>,
    pub from_entity: String,
    pub to_entity: String,
    pub relation_type: String,
    pub properties: Option<Value>,
    pub tenant_id: String,
    pub source_id: Option<i64>,
    pub neo4j_rel_id: Option<String>,
}

/// Graph statistics per tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_nodes: u64,
    pub total_edges: u64,
    pub nodes_by_type: Vec<TypeCount>,
    pub edges_by_type: Vec<TypeCount>,
}

impl Default for GraphStats {
    fn default() -> Self {
        Self {
            total_nodes: 0,
            total_edges: 0,
            nodes_by_type: vec![],
            edges_by_type: vec![],
        }
    }
}

/// Count by entity/relation type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCount {
    pub type_name: String,
    pub count: u64,
}

/// Path result between two entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResult {
    pub nodes: Vec<PathNode>,
    pub relationships: Vec<PathRelationship>,
    pub total_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathNode {
    pub name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathRelationship {
    pub from: String,
    pub to: String,
    pub relation_type: String,
}

/// Visualization data for Sigma.js frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphVisualizationData {
    pub nodes: Vec<VisualizationNode>,
    pub edges: Vec<VisualizationEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationNode {
    pub id: String,
    pub label: String,
    pub entity_type: String,
    pub color: String,
    pub size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cypher Query Builders (pure functions, testable without Neo4j)
// ═══════════════════════════════════════════════════════════════════════════════

/// Build Cypher for upserting an entity node.
pub fn build_upsert_entity_cypher() -> &'static str {
    "MERGE (n:Entity {name: $name, entity_type: $entity_type, tenant_id: $tenant_id}) \
     ON CREATE SET n.properties = $properties, n.source_id = $source_id, n.chunk_id = $chunk_id, n.created_at = datetime() \
     ON MATCH SET n.properties = $properties \
     RETURN elementId(n) AS node_id"
}

/// Build Cypher for upserting a relation edge.
pub fn build_upsert_relation_cypher() -> &'static str {
    "MATCH (a:Entity {name: $from_name, tenant_id: $tenant_id}) \
     MATCH (b:Entity {name: $to_name, tenant_id: $tenant_id}) \
     MERGE (a)-[r:RELATES_TO {relation_type: $relation_type}]->(b) \
     ON CREATE SET r.properties = $properties, r.source_id = $source_id, r.created_at = datetime() \
     ON MATCH SET r.properties = $properties \
     RETURN elementId(r) AS rel_id"
}

/// Build Cypher for searching entities by text.
pub fn build_search_entities_cypher() -> &'static str {
    "MATCH (n:Entity) \
     WHERE n.tenant_id = $tenant_id AND (toLower(n.name) CONTAINS toLower($query) OR toLower(n.entity_type) CONTAINS toLower($query)) \
     RETURN n.name AS name, n.entity_type AS entity_type, n.properties AS properties, elementId(n) AS node_id \
     ORDER BY n.name \
     LIMIT $limit"
}

/// Build Cypher for finding shortest path between two entities.
pub fn build_find_paths_cypher() -> &'static str {
    "MATCH (a:Entity {name: $from_name, tenant_id: $tenant_id}), \
           (b:Entity {name: $to_name, tenant_id: $tenant_id}), \
           p = shortestPath((a)-[*..6]-(b)) \
     RETURN nodes(p) AS nodes, relationships(p) AS rels"
}

/// Build Cypher for getting neighbors of an entity.
pub fn build_get_neighbors_cypher(depth: u32) -> String {
    let max_depth = depth.min(5); // Cap at 5 for safety
    format!(
        "MATCH (n:Entity {{name: $entity_name, tenant_id: $tenant_id}})-[r*1..{}]-(m:Entity) \
         WHERE m.tenant_id = $tenant_id \
         WITH DISTINCT m, r \
         RETURN m.name AS name, m.entity_type AS entity_type, m.properties AS properties \
         LIMIT $limit",
        max_depth
    )
}

/// Build Cypher for graph stats.
pub fn build_graph_stats_cypher() -> &'static str {
    "MATCH (n:Entity) WHERE n.tenant_id = $tenant_id \
     RETURN n.entity_type AS type, count(n) AS cnt \
     ORDER BY cnt DESC"
}

/// Build Cypher for edge stats.
pub fn build_edge_stats_cypher() -> &'static str {
    "MATCH (a:Entity)-[r:RELATES_TO]->(b:Entity) \
     WHERE a.tenant_id = $tenant_id \
     RETURN r.relation_type AS type, count(r) AS cnt \
     ORDER BY cnt DESC"
}

/// Build Cypher for deleting entities by source.
pub fn build_delete_by_source_cypher() -> &'static str {
    "MATCH (n:Entity {tenant_id: $tenant_id, source_id: $source_id}) \
     DETACH DELETE n \
     RETURN count(n) AS deleted_count"
}

/// Build Cypher for visualization data (nodes + edges).
pub fn build_visualization_cypher() -> &'static str {
    "MATCH (n:Entity) WHERE n.tenant_id = $tenant_id \
     WITH n LIMIT $limit \
     OPTIONAL MATCH (n)-[r:RELATES_TO]->(m:Entity {tenant_id: $tenant_id}) \
     RETURN collect(DISTINCT {name: n.name, type: n.entity_type, id: elementId(n)}) AS nodes, \
            collect(DISTINCT {from: n.name, to: m.name, type: r.relation_type, id: elementId(r)}) AS edges"
}

/// Map entity type to a color for visualization.
pub fn entity_type_color(entity_type: &str) -> &'static str {
    match entity_type.to_lowercase().as_str() {
        "person" => "#4A90D9",      // Blue
        "organization" => "#27AE60", // Green
        "location" => "#E67E22",     // Orange
        "concept" => "#9B59B6",      // Purple
        "event" => "#E74C3C",        // Red
        "product" => "#1ABC9C",      // Teal
        "drug" => "#F39C12",         // Yellow
        "symptom" => "#E91E63",      // Pink
        "item" => "#00BCD4",         // Cyan
        "monster" => "#795548",      // Brown
        _ => "#95A5A6",              // Gray (default)
    }
}

/// Entity type to node size for visualization.
pub fn entity_type_size(entity_type: &str) -> f32 {
    match entity_type.to_lowercase().as_str() {
        "person" | "organization" => 10.0,
        "concept" | "event" => 8.0,
        _ => 6.0,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Neo4jService (uses neo4rs Graph for real operations)
// ═══════════════════════════════════════════════════════════════════════════════

/// Neo4j service wrapper for Knowledge Graph operations.
pub struct Neo4jService {
    graph: neo4rs::Graph,
}

impl Neo4jService {
    /// Connect to Neo4j and create indexes/constraints.
    pub async fn new(config: &Neo4jConfig) -> Result<Self> {
        info!(uri = %config.uri, "Connecting to Neo4j...");
        let graph = neo4rs::Graph::new(&config.uri, &config.user, &config.password)
            .await
            .context("Failed to connect to Neo4j")?;

        info!("✅ Connected to Neo4j");

        // Create indexes for performance
        let index_queries = [
            "CREATE INDEX entity_tenant_idx IF NOT EXISTS FOR (n:Entity) ON (n.tenant_id)",
            "CREATE INDEX entity_name_idx IF NOT EXISTS FOR (n:Entity) ON (n.name)",
            "CREATE INDEX entity_type_idx IF NOT EXISTS FOR (n:Entity) ON (n.entity_type)",
            "CREATE INDEX entity_source_idx IF NOT EXISTS FOR (n:Entity) ON (n.source_id)",
        ];

        for query in &index_queries {
            match graph.run(neo4rs::query(query)).await {
                Ok(_) => {},
                Err(e) => warn!("Index creation warning (may already exist): {}", e),
            }
        }

        Ok(Self { graph })
    }

    /// Try to connect, returning None if Neo4j is unavailable (graceful degradation).
    pub async fn try_new(config: &Neo4jConfig) -> Option<Self> {
        match Self::new(config).await {
            Ok(service) => Some(service),
            Err(e) => {
                warn!("⚠️ Neo4j unavailable (KG features disabled): {}", e);
                None
            }
        }
    }

    /// Upsert an entity node in Neo4j.
    pub async fn upsert_entity(
        &self,
        tenant_id: &str,
        name: &str,
        entity_type: &str,
        properties: Option<&str>,
        source_id: Option<i64>,
        chunk_id: Option<i64>,
    ) -> Result<String> {
        let props_str = properties.unwrap_or("{}");
        let query = neo4rs::query(build_upsert_entity_cypher())
            .param("name", name)
            .param("entity_type", entity_type)
            .param("tenant_id", tenant_id)
            .param("properties", props_str)
            .param("source_id", source_id.unwrap_or(-1))
            .param("chunk_id", chunk_id.unwrap_or(-1));

        let mut result = self.graph.execute(query).await
            .context("Failed to upsert entity")?;

        if let Some(row) = result.next().await? {
            let node_id: String = row.get("node_id").unwrap_or_default();
            Ok(node_id)
        } else {
            Ok(String::new())
        }
    }

    /// Upsert a relation edge in Neo4j.
    pub async fn upsert_relation(
        &self,
        tenant_id: &str,
        from_name: &str,
        to_name: &str,
        relation_type: &str,
        properties: Option<&str>,
        source_id: Option<i64>,
    ) -> Result<String> {
        let props_str = properties.unwrap_or("{}");
        let query = neo4rs::query(build_upsert_relation_cypher())
            .param("from_name", from_name)
            .param("to_name", to_name)
            .param("relation_type", relation_type)
            .param("tenant_id", tenant_id)
            .param("properties", props_str)
            .param("source_id", source_id.unwrap_or(-1));

        let mut result = self.graph.execute(query).await
            .context("Failed to upsert relation")?;

        if let Some(row) = result.next().await? {
            let rel_id: String = row.get("rel_id").unwrap_or_default();
            Ok(rel_id)
        } else {
            Ok(String::new())
        }
    }

    /// Search entities by text query.
    pub async fn search_entities(
        &self,
        tenant_id: &str,
        query_text: &str,
        limit: u32,
    ) -> Result<Vec<GraphEntity>> {
        let query = neo4rs::query(build_search_entities_cypher())
            .param("tenant_id", tenant_id)
            .param("query", query_text)
            .param("limit", limit as i64);

        let mut result = self.graph.execute(query).await
            .context("Failed to search entities")?;

        let mut entities = Vec::new();
        while let Some(row) = result.next().await? {
            entities.push(GraphEntity {
                id: None,
                name: row.get("name").unwrap_or_default(),
                entity_type: row.get("entity_type").unwrap_or_default(),
                properties: row.get::<String>("properties").ok()
                    .and_then(|s| serde_json::from_str(&s).ok()),
                tenant_id: tenant_id.to_string(),
                source_id: None,
                chunk_id: None,
                neo4j_node_id: row.get("node_id").ok(),
            });
        }

        Ok(entities)
    }

    /// Get graph statistics for a tenant.
    pub async fn get_graph_stats(&self, tenant_id: &str) -> Result<GraphStats> {
        let mut stats = GraphStats::default();

        // Node stats
        let query = neo4rs::query(build_graph_stats_cypher())
            .param("tenant_id", tenant_id);
        let mut result = self.graph.execute(query).await
            .context("Failed to get node stats")?;

        while let Some(row) = result.next().await? {
            let type_name: String = row.get("type").unwrap_or_default();
            let count: i64 = row.get("cnt").unwrap_or(0);
            stats.total_nodes += count as u64;
            stats.nodes_by_type.push(TypeCount {
                type_name,
                count: count as u64,
            });
        }

        // Edge stats
        let query = neo4rs::query(build_edge_stats_cypher())
            .param("tenant_id", tenant_id);
        let mut result = self.graph.execute(query).await
            .context("Failed to get edge stats")?;

        while let Some(row) = result.next().await? {
            let type_name: String = row.get("type").unwrap_or_default();
            let count: i64 = row.get("cnt").unwrap_or(0);
            stats.total_edges += count as u64;
            stats.edges_by_type.push(TypeCount {
                type_name,
                count: count as u64,
            });
        }

        Ok(stats)
    }

    /// Delete all entities (and their relations) for a specific source.
    pub async fn delete_entities_by_source(
        &self,
        tenant_id: &str,
        source_id: i64,
    ) -> Result<u64> {
        let query = neo4rs::query(build_delete_by_source_cypher())
            .param("tenant_id", tenant_id)
            .param("source_id", source_id);

        let mut result = self.graph.execute(query).await
            .context("Failed to delete entities by source")?;

        if let Some(row) = result.next().await? {
            let count: i64 = row.get("deleted_count").unwrap_or(0);
            Ok(count as u64)
        } else {
            Ok(0)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-017a: Neo4jConfig defaults
    // ========================================
    #[test]
    fn test_neo4j_config_defaults() {
        // Remove env vars to test defaults
        unsafe { std::env::remove_var("NEO4J_URI"); }
        unsafe { std::env::remove_var("NEO4J_USER"); }
        unsafe { std::env::remove_var("NEO4J_PASSWORD"); }

        let config = Neo4jConfig::from_env();
        assert_eq!(config.uri, "bolt://localhost:7687");
        assert_eq!(config.user, "neo4j");
        assert_eq!(config.password, "mimir_neo4j_password");
    }

    // ========================================
    // UT-017b: Upsert entity Cypher contains tenant isolation
    // ========================================
    #[test]
    fn test_upsert_entity_cypher_has_tenant_id() {
        let cypher = build_upsert_entity_cypher();
        assert!(cypher.contains("tenant_id: $tenant_id"), "Upsert entity must filter by tenant_id");
        assert!(cypher.contains("MERGE"), "Must use MERGE for upsert");
        assert!(cypher.contains("ON CREATE SET"), "Must handle ON CREATE");
        assert!(cypher.contains("ON MATCH SET"), "Must handle ON MATCH");
    }

    // ========================================
    // UT-017c: Upsert relation Cypher contains tenant isolation
    // ========================================
    #[test]
    fn test_upsert_relation_cypher_has_tenant_id() {
        let cypher = build_upsert_relation_cypher();
        assert!(cypher.contains("tenant_id: $tenant_id"), "Upsert relation must filter by tenant_id");
        assert!(cypher.contains("MERGE"), "Must use MERGE for upsert");
        assert!(cypher.contains("MATCH (a:Entity"), "Must match source entity");
        assert!(cypher.contains("MATCH (b:Entity"), "Must match target entity");
    }

    // ========================================
    // UT-017d: Search entities Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_search_entities_cypher_tenant_isolation() {
        let cypher = build_search_entities_cypher();
        assert!(cypher.contains("n.tenant_id = $tenant_id"), "Search must filter by tenant_id");
        assert!(cypher.contains("LIMIT $limit"), "Search must be limited");
        assert!(cypher.contains("toLower"), "Search must be case-insensitive");
    }

    // ========================================
    // UT-017e: Find paths Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_find_paths_cypher_tenant_isolation() {
        let cypher = build_find_paths_cypher();
        assert!(cypher.contains("tenant_id: $tenant_id"), "Path finding must filter by tenant_id");
        assert!(cypher.contains("shortestPath"), "Must use shortestPath");
    }

    // ========================================
    // UT-017f: Get neighbors Cypher depth cap
    // ========================================
    #[test]
    fn test_get_neighbors_depth_cap() {
        // Normal depth
        let cypher = build_get_neighbors_cypher(3);
        assert!(cypher.contains("*1..3"), "Depth 3 should produce *1..3");
        assert!(cypher.contains("tenant_id: $tenant_id"), "Must filter by tenant_id");

        // Capped depth
        let cypher_capped = build_get_neighbors_cypher(100);
        assert!(cypher_capped.contains("*1..5"), "Depth should be capped at 5");
    }

    // ========================================
    // UT-017g: Stats Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_stats_cypher_tenant_isolation() {
        let node_stats = build_graph_stats_cypher();
        assert!(node_stats.contains("n.tenant_id = $tenant_id"), "Node stats must filter by tenant_id");

        let edge_stats = build_edge_stats_cypher();
        assert!(edge_stats.contains("a.tenant_id = $tenant_id"), "Edge stats must filter by tenant_id");
    }

    // ========================================
    // UT-017h: Delete by source Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_delete_by_source_cypher_tenant_isolation() {
        let cypher = build_delete_by_source_cypher();
        assert!(cypher.contains("tenant_id: $tenant_id"), "Delete must filter by tenant_id");
        assert!(cypher.contains("source_id: $source_id"), "Delete must filter by source_id");
        assert!(cypher.contains("DETACH DELETE"), "Must use DETACH DELETE to remove relations");
    }

    // ========================================
    // UT-017i: Entity type colors
    // ========================================
    #[test]
    fn test_entity_type_colors() {
        assert_eq!(entity_type_color("Person"), "#4A90D9");
        assert_eq!(entity_type_color("Organization"), "#27AE60");
        assert_eq!(entity_type_color("Location"), "#E67E22");
        assert_eq!(entity_type_color("Drug"), "#F39C12");
        assert_eq!(entity_type_color("Monster"), "#795548");
        // Case insensitive
        assert_eq!(entity_type_color("person"), "#4A90D9");
        assert_eq!(entity_type_color("PERSON"), "#4A90D9");
        // Unknown type
        assert_eq!(entity_type_color("SomeUnknownType"), "#95A5A6");
    }

    // ========================================
    // UT-017j: Entity type sizes
    // ========================================
    #[test]
    fn test_entity_type_sizes() {
        assert_eq!(entity_type_size("Person"), 10.0);
        assert_eq!(entity_type_size("Organization"), 10.0);
        assert_eq!(entity_type_size("Concept"), 8.0);
        assert_eq!(entity_type_size("Item"), 6.0);
    }

    // ========================================
    // UT-017k: GraphStats default
    // ========================================
    #[test]
    fn test_graph_stats_default() {
        let stats = GraphStats::default();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.total_edges, 0);
        assert!(stats.nodes_by_type.is_empty());
        assert!(stats.edges_by_type.is_empty());
    }

    // ========================================
    // UT-017l: Visualization Cypher tenant isolation
    // ========================================
    #[test]
    fn test_visualization_cypher_tenant_isolation() {
        let cypher = build_visualization_cypher();
        assert!(cypher.contains("n.tenant_id = $tenant_id"), "Visualization must filter by tenant_id");
        assert!(cypher.contains("LIMIT $limit"), "Visualization must have limit");
    }

    // ========================================
    // UT-017m: GraphEntity serialization
    // ========================================
    #[test]
    fn test_graph_entity_serialization() {
        let entity = GraphEntity {
            id: Some(1),
            name: "Aspirin".to_string(),
            entity_type: "Drug".to_string(),
            properties: Some(serde_json::json!({"category": "NSAID"})),
            tenant_id: "test-tenant".to_string(),
            source_id: Some(10),
            chunk_id: Some(5),
            neo4j_node_id: Some("4:abc:123".to_string()),
        };
        let json = serde_json::to_value(&entity).unwrap();
        assert_eq!(json["name"], "Aspirin");
        assert_eq!(json["entity_type"], "Drug");
        assert_eq!(json["properties"]["category"], "NSAID");
        assert_eq!(json["tenant_id"], "test-tenant");
    }

    // ========================================
    // UT-017n: GraphRelation serialization
    // ========================================
    #[test]
    fn test_graph_relation_serialization() {
        let relation = GraphRelation {
            id: Some(1),
            from_entity: "Aspirin".to_string(),
            to_entity: "Headache".to_string(),
            relation_type: "treats".to_string(),
            properties: None,
            tenant_id: "test-tenant".to_string(),
            source_id: Some(10),
            neo4j_rel_id: None,
        };
        let json = serde_json::to_value(&relation).unwrap();
        assert_eq!(json["from_entity"], "Aspirin");
        assert_eq!(json["to_entity"], "Headache");
        assert_eq!(json["relation_type"], "treats");
    }
}
