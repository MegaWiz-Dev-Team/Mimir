//! Neo4j Knowledge Graph Service — Sprint 17
//!
//! Provides graph operations for entity/relation storage via Neo4j.
//! All queries enforce tenant isolation with `tenant_id` parameter.

use anyhow::{Context, Result};
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
            password: env::var("NEO4J_PASSWORD")
                .unwrap_or_else(|_| "mimir_neo4j_password".to_string()),
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

/// A PrimeKG node fetched for embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimeKGNode {
    pub entity_index: i64,
    pub name: String,
    pub entity_type: String,
    pub source: Option<String>,
}

impl PrimeKGNode {
    /// Format as embeddable text: "Metformin (Drug) [DrugBank]"
    pub fn to_embed_text(&self) -> String {
        match &self.source {
            Some(src) if !src.is_empty() => {
                format!("{} ({}) [{}]", self.name, self.entity_type, src)
            }
            _ => format!("{} ({})", self.name, self.entity_type),
        }
    }
}

/// A neighbor returned from PrimeKG graph traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimeKGNeighbor {
    pub source_name: String,
    pub source_type: String,
    pub neighbor_index: i64,
    pub neighbor_name: String,
    pub neighbor_type: String,
    pub relation_type: String,
    pub direction: String,
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
     MERGE (a)-[r:RELATES_TO {relation_type: $relation_type, tenant_id: $tenant_id}]->(b) \
     ON CREATE SET r.properties = $properties, r.source_id = $source_id, r.created_at = datetime() \
     ON MATCH SET r.properties = $properties \
     RETURN elementId(r) AS rel_id"
}

/// Build Cypher for searching entities by text (tenant + global PrimeKG).
pub fn build_search_entities_cypher() -> &'static str {
    "MATCH (n:Entity) \
     WHERE n.tenant_id = $tenant_id AND NOT n:Tombstoned AND (toLower(n.name) CONTAINS toLower($query) OR toLower(n.entity_type) CONTAINS toLower($query)) \
     RETURN n.name AS name, n.entity_type AS entity_type, n.properties AS properties, elementId(n) AS node_id \
     UNION \
     MATCH (n:PrimeKG) \
     WHERE toLower(n.name) CONTAINS toLower($query) \
     RETURN n.name AS name, n.type AS entity_type, null AS properties, elementId(n) AS node_id \
     ORDER BY name \
     LIMIT $limit"
}

/// Build Cypher for finding shortest path between two entities.
pub fn build_find_paths_cypher() -> &'static str {
    "MATCH (a:Entity {name: $from_name, tenant_id: $tenant_id}), \
           (b:Entity {name: $to_name, tenant_id: $tenant_id}), \
           p = shortestPath((a)-[*..6]-(b)) \
     RETURN nodes(p) AS nodes, relationships(p) AS rels"
}

/// Build Cypher for getting neighbors of an entity (tenant graph + PrimeKG via SAME_AS).
pub fn build_get_neighbors_cypher(depth: u32) -> String {
    let max_depth = depth.min(5); // Cap at 5 for safety
    format!(
        "MATCH (n:Entity {{name: $entity_name, tenant_id: $tenant_id}})-[r*1..{}]-(m:Entity) \
         WHERE m.tenant_id = $tenant_id \
         WITH DISTINCT m, r \
         RETURN m.name AS name, m.entity_type AS entity_type, m.properties AS properties \
         UNION \
         MATCH (n:Entity {{name: $entity_name, tenant_id: $tenant_id}})-[:SAME_AS]->(p:PrimeKG)-[r]-(neighbor:PrimeKG) \
         RETURN neighbor.name AS name, neighbor.type AS entity_type, null AS properties \
         LIMIT $limit",
        max_depth
    )
}

/// Build Cypher for graph stats.
pub fn build_graph_stats_cypher() -> &'static str {
    "MATCH (n:Entity) WHERE n.tenant_id = $tenant_id AND NOT n:Tombstoned \
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
    "MATCH (n:Entity) WHERE n.tenant_id = $tenant_id AND NOT n:Tombstoned \
     WITH n LIMIT $limit \
     OPTIONAL MATCH (n)-[r:RELATES_TO]->(m:Entity {tenant_id: $tenant_id}) \
     RETURN collect(DISTINCT {name: n.name, type: n.entity_type, id: elementId(n)}) AS nodes, \
            collect(DISTINCT {from: n.name, to: m.name, type: r.relation_type, id: elementId(r)}) AS edges"
}

/// Build Cypher for God Nodes (entities with highest degree).
pub fn build_god_nodes_cypher() -> &'static str {
    "MATCH (n:Entity {tenant_id: $tenant_id}) \
     WITH n, size((n)--()) AS degree \
     WHERE degree > 0 \
     RETURN n.name AS name, n.entity_type AS entity_type, degree AS degree_count \
     ORDER BY degree_count DESC \
     LIMIT $limit"
}

/// Build Cypher for Surprising Connections (edges crossing source document boundaries).
pub fn build_surprising_connections_cypher() -> &'static str {
    "MATCH (a:Entity)-[r:RELATES_TO]->(b:Entity) \
     WHERE r.tenant_id = $tenant_id \
       AND a.source_id IS NOT NULL AND b.source_id IS NOT NULL \
       AND a.source_id <> b.source_id \
     RETURN a.name AS from_name, b.name AS to_name, r.relation_type, \
            a.source_id AS from_source_id, b.source_id AS to_source_id \
     LIMIT $limit"
}

/// Build Cypher for full-text entity search via Lucene index (tenant + global PrimeKG).
pub fn build_fulltext_search_cypher() -> &'static str {
    "CALL db.index.fulltext.queryNodes('entity_name_ft', $query) \
     YIELD node, score \
     WHERE node.tenant_id = $tenant_id \
     RETURN node.name AS name, node.entity_type AS entity_type, \
            node.properties AS properties, score \
     UNION \
     CALL db.index.fulltext.queryNodes('primekg_name_ft', $query) \
     YIELD node, score \
     RETURN node.name AS name, node.type AS entity_type, \
            null AS properties, score \
     ORDER BY score DESC \
     LIMIT $limit"
}

/// Build Cypher for 2-hop neighbor expansion (outgoing + incoming).
pub fn build_expand_neighbors_cypher() -> &'static str {
    "MATCH (root:Entity {name: $entity_name, tenant_id: $tenant_id}) \
     MATCH (root)-[r1:RELATES_TO]->(n1:Entity) \
     WHERE r1.tenant_id = $tenant_id AND n1.tenant_id = $tenant_id \
     RETURN n1.name AS name, n1.entity_type AS entity_type, \
            r1.relation_type AS relation_type, 1 AS hop, 'outgoing' AS direction \
     UNION ALL \
     MATCH (root:Entity {name: $entity_name, tenant_id: $tenant_id}) \
     MATCH (root)-[r1:RELATES_TO]->(mid:Entity)-[r2:RELATES_TO]->(n2:Entity) \
     WHERE r1.tenant_id = $tenant_id AND mid.tenant_id = $tenant_id \
       AND r2.tenant_id = $tenant_id AND n2.tenant_id = $tenant_id \
     RETURN n2.name AS name, n2.entity_type AS entity_type, \
            (r1.relation_type + ' -> ' + r2.relation_type) AS relation_type, \
            2 AS hop, 'outgoing_2hop' AS direction \
     UNION ALL \
     MATCH (root:Entity {name: $entity_name, tenant_id: $tenant_id}) \
     MATCH (root)<-[r1:RELATES_TO]-(n1:Entity) \
     WHERE r1.tenant_id = $tenant_id AND n1.tenant_id = $tenant_id \
     RETURN n1.name AS name, n1.entity_type AS entity_type, \
            r1.relation_type AS relation_type, 1 AS hop, 'incoming' AS direction \
     UNION ALL \
     MATCH (root:Entity {name: $entity_name, tenant_id: $tenant_id}) \
     MATCH (root)<-[r1:RELATES_TO]-(mid:Entity)<-[r2:RELATES_TO]-(n2:Entity) \
     WHERE r1.tenant_id = $tenant_id AND mid.tenant_id = $tenant_id \
       AND r2.tenant_id = $tenant_id AND n2.tenant_id = $tenant_id \
     RETURN n2.name AS name, n2.entity_type AS entity_type, \
            (r2.relation_type + ' -> ' + r1.relation_type) AS relation_type, \
            2 AS hop, 'incoming_2hop' AS direction \
     LIMIT $limit"
}

/// Build Cypher for paginated entity listing with optional filters.
pub fn build_list_entities_cypher(has_query: bool, has_type: bool) -> String {
    let mut conds = vec!["n.tenant_id = $tenant_id AND NOT n:Tombstoned".to_string()];
    if has_query {
        conds.push("toLower(n.name) CONTAINS toLower($query)".to_string());
    }
    if has_type {
        conds.push("n.entity_type = $entity_type".to_string());
    }
    format!(
        "MATCH (n:Entity) WHERE {} \
         RETURN n.name AS name, n.entity_type AS entity_type, n.properties AS properties, \
                n.source_id AS source_id, n.chunk_id AS chunk_id, elementId(n) AS node_id \
         ORDER BY n.name SKIP $offset LIMIT $limit",
        conds.join(" AND ")
    )
}

/// Build Cypher for counting entities with optional filters.
pub fn build_count_entities_cypher(has_query: bool, has_type: bool) -> String {
    let mut conds = vec!["n.tenant_id = $tenant_id AND NOT n:Tombstoned".to_string()];
    if has_query {
        conds.push("toLower(n.name) CONTAINS toLower($query)".to_string());
    }
    if has_type {
        conds.push("n.entity_type = $entity_type".to_string());
    }
    format!("MATCH (n:Entity) WHERE {} RETURN count(n) AS total", conds.join(" AND "))
}

/// Build Cypher to search PrimeKG nodes by name substring.
pub fn build_primekg_search_cypher() -> &'static str {
    "MATCH (n:PrimeKG) WHERE toLower(n.name) CONTAINS toLower($query) \
     RETURN n.name AS name, n.type AS entity_type, elementId(n) AS node_id \
     ORDER BY n.name LIMIT $limit"
}

/// Build Cypher to count PrimeKG nodes matching a name query.
pub fn build_primekg_count_cypher() -> &'static str {
    "MATCH (n:PrimeKG) WHERE toLower(n.name) CONTAINS toLower($query) RETURN count(n) AS total"
}

/// Build Cypher for single entity lookup by name.
pub fn build_get_entity_by_name_cypher() -> &'static str {
    "MATCH (n:Entity {name: $name, tenant_id: $tenant_id}) \
     RETURN n.name AS name, n.entity_type AS entity_type, \
            n.source_id AS source_id, elementId(n) AS node_id \
     LIMIT 1"
}

/// Build Cypher for visualization data — returns one row per node, edge data optional.
pub fn build_visualization_data_cypher(has_type: bool) -> String {
    let type_filter = if has_type {
        " AND n.entity_type = $entity_type"
    } else {
        ""
    };
    format!(
        "MATCH (n:Entity) WHERE n.tenant_id = $tenant_id{} \
         WITH n LIMIT $limit \
         WITH collect(n) AS visible \
         UNWIND visible AS n \
         OPTIONAL MATCH (n)-[r:RELATES_TO]->(m:Entity) WHERE m IN visible \
         RETURN n.name AS name, n.entity_type AS entity_type, elementId(n) AS node_id, \
                m.name AS to_name, r.relation_type AS rel_type, elementId(r) AS rel_id",
        type_filter
    )
}

/// Build Cypher for PrimeKG bridge: tenant entities that have SAME_AS edges into PrimeKG.
/// Returns the linked PrimeKG node plus the SAME_AS edge so we can render the bridge.
pub fn build_visualization_primekg_bridge_cypher(has_type: bool) -> String {
    let type_filter = if has_type {
        " AND n.entity_type = $entity_type"
    } else {
        ""
    };
    format!(
        "MATCH (n:Entity) WHERE n.tenant_id = $tenant_id{} \
         WITH n LIMIT $limit \
         MATCH (n)-[s:SAME_AS]->(pk:PrimeKG) \
         RETURN n.name AS tenant_name, pk.name AS primekg_name, \
                coalesce(pk.type, 'Other') AS primekg_type, \
                elementId(s) AS rel_id",
        type_filter
    )
}

/// Build Cypher for 1-hop path between two entities.
pub fn build_direct_path_cypher() -> &'static str {
    "MATCH (a:Entity {name: $from_name, tenant_id: $tenant_id})-[r:RELATES_TO]->(b:Entity {name: $to_name, tenant_id: $tenant_id}) \
     RETURN a.name AS from_name, b.name AS to_name, r.relation_type AS rel_type \
     UNION \
     MATCH (a:Entity {name: $to_name, tenant_id: $tenant_id})-[r:RELATES_TO]->(b:Entity {name: $from_name, tenant_id: $tenant_id}) \
     RETURN a.name AS from_name, b.name AS to_name, r.relation_type AS rel_type \
     LIMIT 5"
}

/// Build Cypher for 2-hop path between two entities.
pub fn build_two_hop_path_cypher() -> &'static str {
    "MATCH (a:Entity {name: $from_name, tenant_id: $tenant_id})-[r1:RELATES_TO]->(mid:Entity)-[r2:RELATES_TO]->(b:Entity {name: $to_name, tenant_id: $tenant_id}) \
     WHERE mid.tenant_id = $tenant_id \
     RETURN a.name AS from_name, mid.name AS mid_name, b.name AS to_name, \
            r1.relation_type AS rel1_type, r2.relation_type AS rel2_type \
     LIMIT 5"
}

/// Build Cypher for PrimeKG Drug-Disease exploration.
pub fn build_primekg_cypher() -> &'static str {
    "MATCH (d1:PrimeKG:Drug)-[r]-(d2:PrimeKG:Disease) \
     WHERE toLower(d1.name) CONTAINS toLower($name) OR toLower(d2.name) CONTAINS toLower($name) \
     RETURN d1.name AS drug, type(r) AS relation_type, d2.name AS disease \
     LIMIT $limit"
}

/// Map entity type to a color for visualization.
pub fn entity_type_color(entity_type: &str) -> &'static str {
    match entity_type.to_lowercase().as_str() {
        "person" => "#4A90D9",       // Blue
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
                Ok(_) => {}
                Err(e) => warn!("Index creation warning (may already exist): {}", e),
            }
        }

        Ok(Self { graph })
    }

    /// Borrow the underlying connection (used by `services::resolve::store`).
    pub fn graph(&self) -> &neo4rs::Graph {
        &self.graph
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

        let mut result = self
            .graph
            .execute(query)
            .await
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

        let mut result = self
            .graph
            .execute(query)
            .await
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

        let mut result = self
            .graph
            .execute(query)
            .await
            .context("Failed to search entities")?;

        let mut entities = Vec::new();
        while let Some(row) = result.next().await? {
            entities.push(GraphEntity {
                id: None,
                name: row.get("name").unwrap_or_default(),
                entity_type: row.get("entity_type").unwrap_or_default(),
                properties: row
                    .get::<String>("properties")
                    .ok()
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
        let query = neo4rs::query(build_graph_stats_cypher()).param("tenant_id", tenant_id);
        let mut result = self
            .graph
            .execute(query)
            .await
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
        let query = neo4rs::query(build_edge_stats_cypher()).param("tenant_id", tenant_id);
        let mut result = self
            .graph
            .execute(query)
            .await
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
    pub async fn delete_entities_by_source(&self, tenant_id: &str, source_id: i64) -> Result<u64> {
        let query = neo4rs::query(build_delete_by_source_cypher())
            .param("tenant_id", tenant_id)
            .param("source_id", source_id);

        let mut result = self
            .graph
            .execute(query)
            .await
            .context("Failed to delete entities by source")?;

        if let Some(row) = result.next().await? {
            let count: i64 = row.get("deleted_count").unwrap_or(0);
            Ok(count as u64)
        } else {
            Ok(0)
        }
    }

    /// Look up a single entity by name.
    pub async fn get_entity_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<Option<GraphEntity>> {
        let query = neo4rs::query(build_get_entity_by_name_cypher())
            .param("name", name)
            .param("tenant_id", tenant_id);

        let mut result = self.graph.execute(query).await?;
        if let Some(row) = result.next().await? {
            let src: i64 = row.get("source_id").unwrap_or(-1);
            Ok(Some(GraphEntity {
                id: None,
                name: row.get("name").unwrap_or_default(),
                entity_type: row.get("entity_type").unwrap_or_default(),
                properties: None,
                tenant_id: tenant_id.to_string(),
                source_id: if src >= 0 { Some(src) } else { None },
                chunk_id: None,
                neo4j_node_id: row.get("node_id").ok(),
            }))
        } else {
            Ok(None)
        }
    }

    /// List entities with optional text search, type filter, and pagination.
    /// When searching by name with no type filter, also queries PrimeKG global nodes
    /// and merges results (tenant entities first, then PrimeKG matches).
    pub async fn list_entities(
        &self,
        tenant_id: &str,
        query: Option<&str>,
        entity_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<GraphEntity>, u64)> {
        let has_query = query.map(|q| !q.is_empty()).unwrap_or(false);
        let has_type = entity_type.map(|t| !t.is_empty()).unwrap_or(false);

        let data_cypher = build_list_entities_cypher(has_query, has_type);
        let count_cypher = build_count_entities_cypher(has_query, has_type);

        let mut data_q = neo4rs::query(&data_cypher)
            .param("tenant_id", tenant_id)
            .param("offset", offset)
            .param("limit", limit);
        let mut count_q = neo4rs::query(&count_cypher).param("tenant_id", tenant_id);

        if has_query {
            let q = query.unwrap();
            data_q = data_q.param("query", q);
            count_q = count_q.param("query", q);
        }
        if has_type {
            let t = entity_type.unwrap();
            data_q = data_q.param("entity_type", t);
            count_q = count_q.param("entity_type", t);
        }

        let mut result = self.graph.execute(data_q).await?;
        let mut entities = Vec::new();
        while let Some(row) = result.next().await? {
            let props_str: Option<String> = row.get("properties").ok().flatten();
            let src: i64 = row.get("source_id").unwrap_or(-1);
            let chunk: i64 = row.get("chunk_id").unwrap_or(-1);
            entities.push(GraphEntity {
                id: None,
                name: row.get("name").unwrap_or_default(),
                entity_type: row.get("entity_type").unwrap_or_default(),
                properties: props_str.and_then(|s| serde_json::from_str(&s).ok()),
                tenant_id: tenant_id.to_string(),
                source_id: if src >= 0 { Some(src) } else { None },
                chunk_id: if chunk >= 0 { Some(chunk) } else { None },
                neo4j_node_id: row.get("node_id").ok(),
            });
        }

        let mut count_result = self.graph.execute(count_q).await?;
        let mut total: u64 = if let Some(row) = count_result.next().await? {
            row.get::<i64>("total").unwrap_or(0).max(0) as u64
        } else {
            0
        };

        // When searching by name (no type filter), also include PrimeKG global nodes.
        // Tenant entities take priority; PrimeKG results are appended and total is summed.
        if has_query && !has_type {
            let q_str = query.unwrap();
            let primekg_data_q = neo4rs::query(build_primekg_search_cypher())
                .param("query", q_str)
                .param("limit", limit);
            let primekg_count_q = neo4rs::query(build_primekg_count_cypher())
                .param("query", q_str);

            if let Ok(mut pk_result) = self.graph.execute(primekg_data_q).await {
                while let Some(row) = pk_result.next().await.unwrap_or(None) {
                    entities.push(GraphEntity {
                        id: None,
                        name: row.get("name").unwrap_or_default(),
                        entity_type: row.get("entity_type").unwrap_or_default(),
                        properties: None,
                        tenant_id: String::new(),
                        source_id: None,
                        chunk_id: None,
                        neo4j_node_id: row.get("node_id").ok(),
                    });
                }
            }
            if let Ok(mut pk_cnt) = self.graph.execute(primekg_count_q).await {
                if let Some(row) = pk_cnt.next().await.unwrap_or(None) {
                    total += row.get::<i64>("total").unwrap_or(0).max(0) as u64;
                }
            }
        }

        Ok((entities, total))
    }

    /// Get visualization data (nodes + edges) for a tenant.
    /// When `include_primekg` is true, also fetches PrimeKG entities linked via SAME_AS
    /// edges so the user can see biomedical context around tenant entities.
    pub async fn get_visualization_data(
        &self,
        tenant_id: &str,
        limit: i64,
        entity_type: Option<&str>,
        include_primekg: bool,
    ) -> Result<GraphVisualizationData> {
        let has_type = entity_type.map(|t| !t.is_empty()).unwrap_or(false);
        let cypher = build_visualization_data_cypher(has_type);

        let mut q = neo4rs::query(&cypher)
            .param("tenant_id", tenant_id)
            .param("limit", limit);
        if has_type {
            q = q.param("entity_type", entity_type.unwrap());
        }

        let mut result = self.graph.execute(q).await?;

        let mut nodes: std::collections::HashMap<String, VisualizationNode> =
            std::collections::HashMap::new();
        let mut edges: Vec<VisualizationEdge> = Vec::new();

        while let Some(row) = result.next().await? {
            let node_id: String = row.get("node_id").unwrap_or_default();
            let name: String = row.get("name").unwrap_or_default();
            let entity_type_str: String = row.get("entity_type").unwrap_or_default();

            nodes.entry(node_id.clone()).or_insert_with(|| VisualizationNode {
                id: name.clone(),
                label: name.clone(),
                entity_type: entity_type_str.clone(),
                color: entity_type_color(&entity_type_str).to_string(),
                size: entity_type_size(&entity_type_str),
            });

            if let Ok(to_name) = row.get::<String>("to_name") {
                if !to_name.is_empty() {
                    let rel_type: String = row.get("rel_type").unwrap_or_default();
                    let rel_id: String = row.get("rel_id").unwrap_or_default();
                    edges.push(VisualizationEdge {
                        id: rel_id,
                        source: name,
                        target: to_name,
                        label: rel_type,
                    });
                }
            }
        }

        if include_primekg {
            let bridge_cypher = build_visualization_primekg_bridge_cypher(has_type);
            let mut bq = neo4rs::query(&bridge_cypher)
                .param("tenant_id", tenant_id)
                .param("limit", limit);
            if has_type {
                bq = bq.param("entity_type", entity_type.unwrap());
            }
            match self.graph.execute(bq).await {
                Ok(mut bridge_result) => {
                    while let Some(row) = bridge_result.next().await? {
                        let tenant_name: String = row.get("tenant_name").unwrap_or_default();
                        let pk_name: String = row.get("primekg_name").unwrap_or_default();
                        let pk_type: String = row.get("primekg_type").unwrap_or_default();
                        let rel_id: String = row.get("rel_id").unwrap_or_default();

                        if pk_name.is_empty() {
                            continue;
                        }

                        nodes
                            .entry(format!("primekg:{}", pk_name))
                            .or_insert_with(|| VisualizationNode {
                                id: pk_name.clone(),
                                label: pk_name.clone(),
                                entity_type: pk_type.clone(),
                                color: entity_type_color(&pk_type).to_string(),
                                size: entity_type_size(&pk_type),
                            });
                        edges.push(VisualizationEdge {
                            id: rel_id,
                            source: tenant_name,
                            target: pk_name,
                            label: "SAME_AS".to_string(),
                        });
                    }
                }
                Err(e) => {
                    warn!(error = %e, "PrimeKG bridge query failed; returning tenant-only result");
                }
            }
        }

        Ok(GraphVisualizationData {
            nodes: nodes.into_values().collect(),
            edges,
        })
    }

    /// Find paths between two named entities (1-hop then 2-hop).
    pub async fn find_paths_by_name(
        &self,
        tenant_id: &str,
        from_name: &str,
        to_name: &str,
    ) -> Result<Vec<PathResult>> {
        // 1-hop
        let q1 = neo4rs::query(build_direct_path_cypher())
            .param("tenant_id", tenant_id)
            .param("from_name", from_name)
            .param("to_name", to_name);

        let mut result = self.graph.execute(q1).await?;
        let mut paths: Vec<PathResult> = Vec::new();

        while let Some(row) = result.next().await? {
            let f: String = row.get("from_name").unwrap_or_default();
            let t: String = row.get("to_name").unwrap_or_default();
            let rel: String = row.get("rel_type").unwrap_or_default();
            paths.push(PathResult {
                nodes: vec![
                    PathNode { name: f.clone(), entity_type: String::new() },
                    PathNode { name: t.clone(), entity_type: String::new() },
                ],
                relationships: vec![PathRelationship { from: f, to: t, relation_type: rel }],
                total_length: 1,
            });
        }

        if !paths.is_empty() {
            return Ok(paths);
        }

        // 2-hop
        let q2 = neo4rs::query(build_two_hop_path_cypher())
            .param("tenant_id", tenant_id)
            .param("from_name", from_name)
            .param("to_name", to_name);

        let mut result2 = self.graph.execute(q2).await?;
        while let Some(row) = result2.next().await? {
            let f: String = row.get("from_name").unwrap_or_default();
            let mid: String = row.get("mid_name").unwrap_or_default();
            let t: String = row.get("to_name").unwrap_or_default();
            let r1: String = row.get("rel1_type").unwrap_or_default();
            let r2: String = row.get("rel2_type").unwrap_or_default();
            paths.push(PathResult {
                nodes: vec![
                    PathNode { name: f.clone(), entity_type: String::new() },
                    PathNode { name: mid.clone(), entity_type: String::new() },
                    PathNode { name: t.clone(), entity_type: String::new() },
                ],
                relationships: vec![
                    PathRelationship { from: f, to: mid.clone(), relation_type: r1 },
                    PathRelationship { from: mid, to: t, relation_type: r2 },
                ],
                total_length: 2,
            });
        }

        Ok(paths)
    }

    /// Count all PrimeKG nodes (for embed progress tracking).
    pub async fn count_primekg_nodes(&self) -> Result<i64> {
        let mut result = self
            .graph
            .execute(neo4rs::query("MATCH (n:PrimeKG) RETURN count(n) AS total"))
            .await?;
        if let Some(row) = result.next().await? {
            Ok(row.get::<i64>("total").unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// Count edges incident on PrimeKG nodes (informational only).
    pub async fn count_primekg_edges(&self) -> Result<i64> {
        let mut result = self
            .graph
            .execute(neo4rs::query("MATCH ()-[r]->(:PrimeKG) RETURN count(r) AS c"))
            .await?;
        if let Some(row) = result.next().await? {
            Ok(row.get::<i64>("c").unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// Fetch a batch of PrimeKG nodes for embedding, ordered by entity_index.
    pub async fn stream_primekg_nodes(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<PrimeKGNode>> {
        let mut result = self
            .graph
            .execute(
                neo4rs::query(
                    "MATCH (n:PrimeKG) \
                     RETURN n.entity_index AS entity_index, n.name AS name, \
                            n.type AS entity_type, n.source AS source \
                     ORDER BY n.entity_index \
                     SKIP $offset LIMIT $limit",
                )
                .param("offset", offset)
                .param("limit", limit),
            )
            .await?;

        let mut nodes = Vec::new();
        while let Some(row) = result.next().await? {
            let idx: i64 = row.get("entity_index").unwrap_or(0);
            if idx <= 0 {
                continue;
            }
            nodes.push(PrimeKGNode {
                entity_index: idx,
                name: row.get("name").unwrap_or_default(),
                entity_type: row.get("entity_type").unwrap_or_default(),
                source: row.get("source").ok().flatten(),
            });
        }
        Ok(nodes)
    }

    /// Search PrimeKG for Drug-Disease relations.
    pub async fn search_primekg(
        &self,
        tenant_id: &str,
        name: &str,
        limit: u32,
    ) -> Result<Vec<GraphRelation>> {
        let query = neo4rs::query(build_primekg_cypher())
            .param("tenant_id", tenant_id)
            .param("name", name)
            .param("limit", limit as i64);

        let mut result = self
            .graph
            .execute(query)
            .await
            .context("Failed to execute PrimeKG search")?;

        let mut relations = Vec::new();
        while let Some(row) = result.next().await? {
            let drug: String = row.get("drug").unwrap_or_default();
            let disease: String = row.get("disease").unwrap_or_default();
            let rel_type: String = row.get("relation_type").unwrap_or_default();

            relations.push(GraphRelation {
                id: None,
                from_entity: drug,
                to_entity: disease,
                relation_type: rel_type,
                properties: None,
                tenant_id: tenant_id.to_string(),
                source_id: None,
                neo4j_rel_id: None,
            });
        }

        Ok(relations)
    }

    /// Get God Nodes (highest degree entities) for a tenant.
    pub async fn get_god_nodes(
        &self,
        tenant_id: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, i64)>> {
        let query = neo4rs::query(build_god_nodes_cypher())
            .param("tenant_id", tenant_id)
            .param("limit", limit);

        let mut result = self.graph.execute(query).await?;
        let mut rows = Vec::new();
        while let Some(row) = result.next().await? {
            let name: String = row.get("name").unwrap_or_default();
            let entity_type: String = row.get("entity_type").unwrap_or_default();
            let degree: i64 = row.get("degree_count").unwrap_or(0);
            rows.push((name, entity_type, degree));
        }
        Ok(rows)
    }

    /// Get Surprising Connections (edges crossing source document boundaries).
    pub async fn get_surprising_connections(
        &self,
        tenant_id: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String, i64, i64)>> {
        let query = neo4rs::query(build_surprising_connections_cypher())
            .param("tenant_id", tenant_id)
            .param("limit", limit);

        let mut result = self.graph.execute(query).await?;
        let mut rows = Vec::new();
        while let Some(row) = result.next().await? {
            let from_name: String = row.get("from_name").unwrap_or_default();
            let to_name: String = row.get("to_name").unwrap_or_default();
            let rel_type: String = row.get("relation_type").unwrap_or_default();
            let from_src: i64 = row.get("from_source_id").unwrap_or(-1);
            let to_src: i64 = row.get("to_source_id").unwrap_or(-1);
            rows.push((from_name, to_name, rel_type, from_src, to_src));
        }
        Ok(rows)
    }

    /// Full-text entity search using the Lucene index.
    /// Returns (name, entity_type, properties_json) tuples ordered by relevance.
    /// Falls back to CONTAINS search if FTS returns no results.
    pub async fn search_entities_ft(
        &self,
        tenant_id: &str,
        query_text: &str,
        limit: u32,
    ) -> Result<Vec<(String, String, Option<String>)>> {
        let query = neo4rs::query(build_fulltext_search_cypher())
            .param("tenant_id", tenant_id)
            .param("query", query_text)
            .param("limit", limit as i64);

        let mut result = self.graph.execute(query).await?;
        let mut rows = Vec::new();
        while let Some(row) = result.next().await? {
            let name: String = row.get("name").unwrap_or_default();
            let entity_type: String = row.get("entity_type").unwrap_or_default();
            let props: Option<String> = row.get("properties").ok().flatten();
            rows.push((name, entity_type, props));
        }

        if rows.is_empty() {
            // Fallback: CONTAINS search when FTS yields nothing
            let fallback = neo4rs::query(build_search_entities_cypher())
                .param("tenant_id", tenant_id)
                .param("query", query_text)
                .param("limit", limit as i64);
            let mut fb = self.graph.execute(fallback).await?;
            while let Some(row) = fb.next().await? {
                let name: String = row.get("name").unwrap_or_default();
                let entity_type: String = row.get("entity_type").unwrap_or_default();
                let props: Option<String> = row.get("properties").ok().flatten();
                rows.push((name, entity_type, props));
            }
        }

        Ok(rows)
    }

    /// Expand 2-hop neighbors for a named entity.
    /// Returns (name, entity_type, relation_type, hop, direction) tuples.
    pub async fn expand_neighbors(
        &self,
        tenant_id: &str,
        entity_name: &str,
        limit: u32,
    ) -> Result<Vec<(String, String, String, i64, String)>> {
        let query = neo4rs::query(build_expand_neighbors_cypher())
            .param("entity_name", entity_name)
            .param("tenant_id", tenant_id)
            .param("limit", limit as i64);

        let mut result = self.graph.execute(query).await?;
        let mut rows = Vec::new();
        while let Some(row) = result.next().await? {
            let name: String = row.get("name").unwrap_or_default();
            let entity_type: String = row.get("entity_type").unwrap_or_default();
            let relation_type: String = row.get("relation_type").unwrap_or_default();
            let hop: i64 = row.get("hop").unwrap_or(1);
            let direction: String = row.get("direction").unwrap_or_default();
            rows.push((name, entity_type, relation_type, hop, direction));
        }
        Ok(rows)
    }

    /// Get PrimeKG neighbors for a given entity_index.
    /// Returns (neighbor_name, neighbor_type, relation_type, direction) tuples.
    /// Used by agents for explicit graph traversal (drug interactions, pathways, etc.)
    pub async fn get_primekg_neighbors_by_index(
        &self,
        entity_index: i64,
        limit: i64,
    ) -> Result<Vec<PrimeKGNeighbor>> {
        let cypher = "\
            MATCH (n:PrimeKG {entity_index: $entity_index})-[r]-(m:PrimeKG) \
            RETURN \
                n.name AS source_name, n.entity_type AS source_type, \
                m.entity_index AS neighbor_index, m.name AS neighbor_name, m.entity_type AS neighbor_type, \
                type(r) AS relation_type, \
                CASE WHEN startNode(r) = n THEN 'outgoing' ELSE 'incoming' END AS direction \
            LIMIT $limit";

        let query = neo4rs::query(cypher)
            .param("entity_index", entity_index)
            .param("limit", limit);

        let mut result = self.graph.execute(query).await.context("PrimeKG neighbor query failed")?;
        let mut neighbors = Vec::new();

        while let Some(row) = result.next().await? {
            neighbors.push(PrimeKGNeighbor {
                source_name: row.get("source_name").unwrap_or_default(),
                source_type: row.get("source_type").unwrap_or_default(),
                neighbor_index: row.get("neighbor_index").unwrap_or(-1),
                neighbor_name: row.get("neighbor_name").unwrap_or_default(),
                neighbor_type: row.get("neighbor_type").unwrap_or_default(),
                relation_type: row.get("relation_type").unwrap_or_default(),
                direction: row.get("direction").unwrap_or_default(),
            });
        }

        Ok(neighbors)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Hermodr MCP — PrimeKG graph-native tools (Sprint 1 follow-up)
    // ═══════════════════════════════════════════════════════════════════════════
    //
    // PrimeKG schema confirmed 2026-05-19:
    //   Node label  :PrimeKG + secondary type label (e.g. :Drug, :Disease)
    //   Properties  entity_index (i64), entity_id (str), name (str),
    //               type (lowercase: "drug" | "disease" | "gene/protein" |
    //               "effect/phenotype" | "anatomy" | "pathway" | …), source
    //   Edge props  source (provenance), display_relation (human label)
    //   NO severity on DRUG_DRUG — PrimeKG doesn't track DDI severity natively
    //
    // Relevant edge types:
    //   INDICATION (18.7K)         disease ← drug         "use this drug for"
    //   CONTRAINDICATION (61.3K)   disease ← drug         "do not use this drug"
    //   OFF_LABEL_USE (5.1K)       disease ← drug         "off-label use"
    //   DRUG_DRUG (2.67M)          drug — drug            interaction (binary)
    //   DISEASE_PHENOTYPE_POSITIVE (300K)  disease → phenotype  symptom-of

    /// Look up a PrimeKG entity by name with optional type filter.
    /// Case-insensitive CONTAINS match on name; type filter exact-match on `n.type`.
    pub async fn primekg_lookup_entity(
        &self,
        name: &str,
        type_filter: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Value>> {
        // Ranking: exact (lowercased) match first, then starts-with prefix,
        // then substring; tie-break by shorter name. Avoids the
        // "diabetes mellitus type 2" query returning the longer
        // "diabetes mellitus type 2 associated cataract" before the canonical
        // "type 2 diabetes mellitus" node.
        let cypher = match type_filter {
            Some(_) => "\
                MATCH (n:PrimeKG) \
                WHERE toLower(n.name) CONTAINS toLower($name) AND n.type = $type \
                RETURN n.entity_index AS entity_index, n.entity_id AS entity_id, \
                       n.name AS name, n.type AS type, n.source AS source, \
                       CASE WHEN toLower(n.name) = toLower($name) THEN 0 \
                            WHEN toLower(n.name) STARTS WITH toLower($name) THEN 1 \
                            ELSE 2 END AS match_rank \
                ORDER BY match_rank ASC, size(n.name) ASC \
                LIMIT $limit",
            None => "\
                MATCH (n:PrimeKG) \
                WHERE toLower(n.name) CONTAINS toLower($name) \
                RETURN n.entity_index AS entity_index, n.entity_id AS entity_id, \
                       n.name AS name, n.type AS type, n.source AS source, \
                       CASE WHEN toLower(n.name) = toLower($name) THEN 0 \
                            WHEN toLower(n.name) STARTS WITH toLower($name) THEN 1 \
                            ELSE 2 END AS match_rank \
                ORDER BY match_rank ASC, size(n.name) ASC \
                LIMIT $limit",
        };
        let mut q = neo4rs::query(cypher)
            .param("name", name)
            .param("limit", limit);
        if let Some(t) = type_filter {
            q = q.param("type", t);
        }
        let mut result = self.graph.execute(q).await
            .context("primekg_lookup_entity query failed")?;
        let mut out = Vec::new();
        while let Some(row) = result.next().await? {
            out.push(serde_json::json!({
                "entity_index": row.get::<i64>("entity_index").ok(),
                "entity_id":    row.get::<String>("entity_id").ok(),
                "name":         row.get::<String>("name").ok(),
                "type":         row.get::<String>("type").ok(),
                "source":       row.get::<String>("source").ok(),
            }));
        }
        Ok(out)
    }

    /// Neighbors of an entity with optional relation-type filter + multi-hop expansion.
    /// `hops` clamped 1..=3, `limit` clamped 1..=100.
    pub async fn primekg_neighbors_filtered(
        &self,
        entity_index: i64,
        relation_types: &[String],
        hops: u32,
        limit: i64,
    ) -> Result<Vec<Value>> {
        let hops = hops.clamp(1, 3);
        // Pattern length 1..=hops via variable-length expansion.
        let rel_filter = if relation_types.is_empty() {
            String::new()
        } else {
            // neo4rs/Cypher: filter the LAST relationship's type via post-match WHERE
            let joined: Vec<String> = relation_types
                .iter()
                .map(|t| format!("type(last(rels)) = '{}'", t.replace('\'', "")))
                .collect();
            format!("AND ({}) ", joined.join(" OR "))
        };
        let cypher = format!(
            "MATCH path = (n:PrimeKG {{entity_index: $entity_index}})-[rels*1..{hops}]-(m:PrimeKG) \
             WHERE n <> m {rel_filter}\
             WITH m, rels, path, length(path) AS hops \
             ORDER BY hops ASC \
             WITH m, collect({{rels: rels, hops: hops}})[0] AS shortest \
             RETURN m.entity_index AS entity_index, m.name AS name, m.type AS type, \
                    [r IN shortest.rels | type(r)] AS path_relations, shortest.hops AS hops \
             ORDER BY shortest.hops ASC, m.name ASC \
             LIMIT $limit",
        );
        let q = neo4rs::query(&cypher)
            .param("entity_index", entity_index)
            .param("limit", limit);
        let mut result = self.graph.execute(q).await
            .context("primekg_neighbors_filtered query failed")?;
        let mut out = Vec::new();
        while let Some(row) = result.next().await? {
            let rels: Vec<String> = row.get("path_relations").unwrap_or_default();
            out.push(serde_json::json!({
                "entity_index":   row.get::<i64>("entity_index").ok(),
                "name":           row.get::<String>("name").ok(),
                "type":           row.get::<String>("type").ok(),
                "path_relations": rels,
                "hops":           row.get::<i64>("hops").unwrap_or(1),
            }));
        }
        Ok(out)
    }

    /// Drug-drug interactions for a single drug. PrimeKG doesn't store severity,
    /// so the optional severity filter is documented as a no-op; we return
    /// `display_relation` and `source` so callers can post-filter heuristically.
    pub async fn primekg_drug_interactions(
        &self,
        drug_index: i64,
        limit: i64,
    ) -> Result<Vec<Value>> {
        // PrimeKG stores DDI as two directed edges per pair (A→B and B→A).
        // Filter to outgoing-from-anchor to dedupe; flipping direction
        // would give the same canonical pair set with reversed `start`.
        let cypher = "\
            MATCH (d:PrimeKG {entity_index: $drug_index})-[r:DRUG_DRUG]->(other:PrimeKG) \
            RETURN DISTINCT other.entity_index AS entity_index, other.name AS name, \
                   other.entity_id AS entity_id, \
                   r.display_relation AS display_relation, r.source AS source \
            ORDER BY other.name ASC \
            LIMIT $limit";
        let q = neo4rs::query(cypher)
            .param("drug_index", drug_index)
            .param("limit", limit);
        let mut result = self.graph.execute(q).await
            .context("primekg_drug_interactions query failed")?;
        let mut out = Vec::new();
        while let Some(row) = result.next().await? {
            out.push(serde_json::json!({
                "entity_index":     row.get::<i64>("entity_index").ok(),
                "entity_id":        row.get::<String>("entity_id").ok(),
                "name":             row.get::<String>("name").ok(),
                "display_relation": row.get::<String>("display_relation").ok(),
                "source":           row.get::<String>("source").ok(),
            }));
        }
        Ok(out)
    }

    /// Drugs associated with a disease, grouped by relation. Returns three
    /// vectors: indication, contraindication, off_label_use. Uses UNION ALL
    /// so each relation gets its own LIMIT — without this, the caller-side
    /// fan-out couldn't guarantee balanced groups (a disease may have 1000s
    /// of CONTRAINDICATION edges that would crowd out INDICATIONs).
    pub async fn primekg_disease_drugs(
        &self,
        disease_index: i64,
        limit_per_rel: i64,
    ) -> Result<Value> {
        let cypher = "\
            CALL { WITH $disease_index AS idx \
                MATCH (d:PrimeKG {entity_index: idx})-[r:INDICATION]-(drug:PrimeKG) \
                RETURN drug.entity_index AS entity_index, drug.entity_id AS entity_id, \
                       drug.name AS name, 'INDICATION' AS relation, r.source AS source \
                LIMIT $limit_per_rel \
            UNION ALL \
                WITH $disease_index AS idx \
                MATCH (d:PrimeKG {entity_index: idx})-[r:CONTRAINDICATION]-(drug:PrimeKG) \
                RETURN drug.entity_index AS entity_index, drug.entity_id AS entity_id, \
                       drug.name AS name, 'CONTRAINDICATION' AS relation, r.source AS source \
                LIMIT $limit_per_rel \
            UNION ALL \
                WITH $disease_index AS idx \
                MATCH (d:PrimeKG {entity_index: idx})-[r:OFF_LABEL_USE]-(drug:PrimeKG) \
                RETURN drug.entity_index AS entity_index, drug.entity_id AS entity_id, \
                       drug.name AS name, 'OFF_LABEL_USE' AS relation, r.source AS source \
                LIMIT $limit_per_rel \
            } RETURN entity_index, entity_id, name, relation, source";
        let q = neo4rs::query(cypher)
            .param("disease_index", disease_index)
            .param("limit_per_rel", limit_per_rel);
        let mut result = self.graph.execute(q).await
            .context("primekg_disease_drugs query failed")?;
        let mut indication = Vec::new();
        let mut contraindication = Vec::new();
        let mut off_label = Vec::new();
        while let Some(row) = result.next().await? {
            let drug = serde_json::json!({
                "entity_index": row.get::<i64>("entity_index").ok(),
                "entity_id":    row.get::<String>("entity_id").ok(),
                "name":         row.get::<String>("name").ok(),
                "source":       row.get::<String>("source").ok(),
            });
            match row.get::<String>("relation").as_deref() {
                Ok("INDICATION")       => indication.push(drug),
                Ok("CONTRAINDICATION") => contraindication.push(drug),
                Ok("OFF_LABEL_USE")    => off_label.push(drug),
                _ => {}
            }
        }
        Ok(serde_json::json!({
            "indication":       indication,
            "contraindication": contraindication,
            "off_label_use":    off_label,
        }))
    }

    /// Reverse-lookup symptoms/phenotypes → candidate diseases via
    /// DISEASE_PHENOTYPE_POSITIVE edges. Ranked by match count (most overlap first).
    pub async fn primekg_symptom_to_disease(
        &self,
        phenotype_names: &[String],
        min_match: u32,
        limit: i64,
    ) -> Result<Vec<Value>> {
        let cypher = "\
            UNWIND $names AS sym_name \
            MATCH (p:PrimeKG {type: 'effect/phenotype'}) \
                WHERE toLower(p.name) = toLower(sym_name) \
            MATCH (d:PrimeKG {type: 'disease'})-[:DISEASE_PHENOTYPE_POSITIVE]->(p) \
            WITH d, count(DISTINCT p) AS match_count, collect(DISTINCT p.name) AS matched \
            WHERE match_count >= $min_match \
            RETURN d.entity_index AS entity_index, d.entity_id AS entity_id, \
                   d.name AS name, match_count, matched \
            ORDER BY match_count DESC, d.name ASC \
            LIMIT $limit";
        let q = neo4rs::query(cypher)
            .param("names", phenotype_names.to_vec())
            .param("min_match", min_match as i64)
            .param("limit", limit);
        let mut result = self.graph.execute(q).await
            .context("primekg_symptom_to_disease query failed")?;
        let mut out = Vec::new();
        while let Some(row) = result.next().await? {
            let matched: Vec<String> = row.get("matched").unwrap_or_default();
            out.push(serde_json::json!({
                "entity_index": row.get::<i64>("entity_index").ok(),
                "entity_id":    row.get::<String>("entity_id").ok(),
                "name":         row.get::<String>("name").ok(),
                "match_count":  row.get::<i64>("match_count").unwrap_or(0),
                "matched":      matched,
            }));
        }
        Ok(out)
    }

    /// Shortest path(s) between two PrimeKG entities. Up to `limit_paths`
    /// distinct paths, each with relation breadcrumb. `max_hops` clamped 1..=4.
    pub async fn primekg_path(
        &self,
        from_index: i64,
        to_index: i64,
        max_hops: u32,
        limit_paths: i64,
    ) -> Result<Vec<Value>> {
        let max_hops = max_hops.clamp(1, 4);
        // Dedupe via signature: identical [entity_index sequence] → same path.
        // allShortestPaths can return logically-equivalent paths twice when
        // intermediate edges are bidirectional / duplicated.
        let cypher = format!(
            "MATCH path = allShortestPaths( \
                (a:PrimeKG {{entity_index: $from}})-[*..{max_hops}]-(b:PrimeKG {{entity_index: $to}}) \
             ) \
             WITH path, \
                  [n IN nodes(path) | n.entity_index] AS sig, \
                  [n IN nodes(path) | {{entity_index: n.entity_index, name: n.name, type: n.type}}] AS nodes_list, \
                  [r IN relationships(path) | type(r)] AS rel_types, \
                  length(path) AS hops \
             WITH sig, head(collect({{nodes_list: nodes_list, rel_types: rel_types, hops: hops}})) AS first \
             RETURN first.nodes_list AS nodes_list, first.rel_types AS rel_types, first.hops AS hops \
             ORDER BY hops ASC \
             LIMIT $limit_paths",
        );
        let q = neo4rs::query(&cypher)
            .param("from", from_index)
            .param("to", to_index)
            .param("limit_paths", limit_paths);
        let mut result = self.graph.execute(q).await
            .context("primekg_path query failed")?;
        let mut out = Vec::new();
        while let Some(row) = result.next().await? {
            let rels: Vec<String> = row.get("rel_types").unwrap_or_default();
            // nodes_list comes back as Vec<BoltType>; pass through serde
            let nodes_json: serde_json::Value = row
                .get::<serde_json::Value>("nodes_list")
                .unwrap_or_else(|_| serde_json::json!([]));
            out.push(serde_json::json!({
                "nodes":     nodes_json,
                "relations": rels,
                "hops":      row.get::<i64>("hops").unwrap_or(0),
            }));
        }
        Ok(out)
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
        unsafe {
            std::env::remove_var("NEO4J_URI");
        }
        unsafe {
            std::env::remove_var("NEO4J_USER");
        }
        unsafe {
            std::env::remove_var("NEO4J_PASSWORD");
        }

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
        assert!(
            cypher.contains("tenant_id: $tenant_id"),
            "Upsert entity must filter by tenant_id"
        );
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
        assert!(
            cypher.contains("tenant_id: $tenant_id"),
            "Upsert relation must filter by tenant_id"
        );
        assert!(cypher.contains("MERGE"), "Must use MERGE for upsert");
        assert!(
            cypher.contains("MATCH (a:Entity"),
            "Must match source entity"
        );
        assert!(
            cypher.contains("MATCH (b:Entity"),
            "Must match target entity"
        );
    }

    // ========================================
    // UT-017d: Search entities Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_search_entities_cypher_tenant_isolation() {
        let cypher = build_search_entities_cypher();
        assert!(
            cypher.contains("n.tenant_id = $tenant_id"),
            "Search must filter by tenant_id"
        );
        assert!(cypher.contains("LIMIT $limit"), "Search must be limited");
        assert!(
            cypher.contains("toLower"),
            "Search must be case-insensitive"
        );
    }

    // ========================================
    // UT-017e: Find paths Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_find_paths_cypher_tenant_isolation() {
        let cypher = build_find_paths_cypher();
        assert!(
            cypher.contains("tenant_id: $tenant_id"),
            "Path finding must filter by tenant_id"
        );
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
        assert!(
            cypher.contains("tenant_id: $tenant_id"),
            "Must filter by tenant_id"
        );

        // Capped depth
        let cypher_capped = build_get_neighbors_cypher(100);
        assert!(
            cypher_capped.contains("*1..5"),
            "Depth should be capped at 5"
        );
    }

    // ========================================
    // UT-017g: Stats Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_stats_cypher_tenant_isolation() {
        let node_stats = build_graph_stats_cypher();
        assert!(
            node_stats.contains("n.tenant_id = $tenant_id"),
            "Node stats must filter by tenant_id"
        );

        let edge_stats = build_edge_stats_cypher();
        assert!(
            edge_stats.contains("a.tenant_id = $tenant_id"),
            "Edge stats must filter by tenant_id"
        );
    }

    // ========================================
    // UT-017h: Delete by source Cypher enforces tenant isolation
    // ========================================
    #[test]
    fn test_delete_by_source_cypher_tenant_isolation() {
        let cypher = build_delete_by_source_cypher();
        assert!(
            cypher.contains("tenant_id: $tenant_id"),
            "Delete must filter by tenant_id"
        );
        assert!(
            cypher.contains("source_id: $source_id"),
            "Delete must filter by source_id"
        );
        assert!(
            cypher.contains("DETACH DELETE"),
            "Must use DETACH DELETE to remove relations"
        );
    }

    // ========================================
    // UT-017h-2: PrimeKG Cypher tenant isolation and nodes
    // ========================================
    #[test]
    fn test_build_primekg_cypher_logic() {
        let cypher = build_primekg_cypher();
        assert!(
            cypher.contains("d1.tenant_id = $tenant_id"),
            "PrimeKG query must filter by tenant_id"
        );
        assert!(
            cypher.contains("Drug"),
            "Must query Drug nodes"
        );
        assert!(
            cypher.contains("Disease"),
            "Must query Disease nodes"
        );
        assert!(
            cypher.contains("LIMIT $limit"),
            "Must query with limit"
        );
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
        assert!(
            cypher.contains("n.tenant_id = $tenant_id"),
            "Visualization must filter by tenant_id"
        );
        assert!(
            cypher.contains("LIMIT $limit"),
            "Visualization must have limit"
        );
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
