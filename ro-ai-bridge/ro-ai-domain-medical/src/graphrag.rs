use anyhow::Result;
use mimir_core_ai::services::neo4j::{GraphRelation, Neo4jService};

/// Search PrimeKG for Drug-Disease concepts utilizing Cypher queries over Mimir's Neo4j backend.
pub async fn search_primekg(
    neo4j_service: &Neo4jService,
    tenant_id: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<GraphRelation>> {
    // We defer to the implemented search_primekg within Neo4jService
    neo4j_service.search_primekg(tenant_id, query, limit).await
}

#[cfg(test)]
mod tests {
    
    use mimir_core_ai::services::neo4j::build_primekg_cypher;

    #[test]
    fn test_primekg_cypher_structure() {
        let cypher = build_primekg_cypher();
        // Just asserting that it pulls Drug and Disease with proper aliases
        assert!(cypher.contains("d1:Drug"));
        assert!(cypher.contains("d2:Disease"));
        assert!(cypher.contains("type(r) AS relation_type"));
    }
}
