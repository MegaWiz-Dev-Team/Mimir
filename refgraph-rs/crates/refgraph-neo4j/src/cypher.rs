//! Cypher query builders for Neo4j

/// Build Cypher query to upsert an entity node
pub fn build_upsert_entity() -> &'static str {
    "MERGE (n:RefEntity {entity_id: $entity_id, domain: $domain})
     ON CREATE SET n += $props, n.created_at = datetime()
     ON MATCH SET  n += $props, n.updated_at = datetime()
     RETURN elementId(n) AS node_id"
}

/// Build Cypher query to upsert a relationship
pub fn build_upsert_relationship() -> &'static str {
    "MATCH (a:RefEntity {entity_id: $source_id})
     MATCH (b:RefEntity {entity_id: $target_id})
     MERGE (a)-[r:REF_REL {rel_type: $rel_type, domain: $domain}]->(b)
     ON CREATE SET r += $props, r.created_at = datetime()
     ON MATCH SET  r += $props, r.updated_at = datetime()
     RETURN elementId(r) AS rel_id"
}

/// Build Cypher query to find entities by vector similarity
pub fn build_find_by_embedding_similarity() -> &'static str {
    "CALL db.index.vector.queryNodes('refgraph_embeddings', $k, $embedding)
     YIELD node, score
     WHERE node.domain = $domain AND score >= $threshold
     RETURN node.entity_id, node.text, score
     ORDER BY score DESC"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_entity_query_contains_merge() {
        let query = build_upsert_entity();
        assert!(query.contains("MERGE"));
        assert!(query.contains("RefEntity"));
        assert!(query.contains("entity_id"));
    }

    #[test]
    fn test_upsert_entity_creates_timestamp() {
        let query = build_upsert_entity();
        assert!(query.contains("created_at"));
        assert!(query.contains("datetime()"));
    }

    #[test]
    fn test_upsert_entity_updates_timestamp() {
        let query = build_upsert_entity();
        assert!(query.contains("updated_at"));
    }

    #[test]
    fn test_upsert_entity_returns_node_id() {
        let query = build_upsert_entity();
        assert!(query.contains("elementId(n) AS node_id"));
    }

    #[test]
    fn test_upsert_entity_uses_domain_filter() {
        let query = build_upsert_entity();
        assert!(query.contains("domain"));
    }

    #[test]
    fn test_upsert_relationship_query_contains_merge() {
        let query = build_upsert_relationship();
        assert!(query.contains("MERGE"));
        assert!(query.contains("REF_REL"));
        assert!(query.contains("rel_type"));
    }

    #[test]
    fn test_upsert_relationship_matches_source_entity() {
        let query = build_upsert_relationship();
        assert!(query.contains("MATCH (a:RefEntity {entity_id: $source_id})"));
    }

    #[test]
    fn test_upsert_relationship_matches_target_entity() {
        let query = build_upsert_relationship();
        assert!(query.contains("MATCH (b:RefEntity {entity_id: $target_id})"));
    }

    #[test]
    fn test_upsert_relationship_creates_directed_edge() {
        let query = build_upsert_relationship();
        assert!(query.contains("(a)-[r:REF_REL"));
        assert!(query.contains("]->(b)"));
    }

    #[test]
    fn test_upsert_relationship_returns_rel_id() {
        let query = build_upsert_relationship();
        assert!(query.contains("elementId(r) AS rel_id"));
    }

    #[test]
    fn test_upsert_relationship_uses_domain_filter() {
        let query = build_upsert_relationship();
        assert!(query.contains("domain: $domain"));
    }

    #[test]
    fn test_embedding_similarity_query_contains_vector_index() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("vector.queryNodes"));
        assert!(query.contains("refgraph_embeddings"));
    }

    #[test]
    fn test_embedding_similarity_uses_k_parameter() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("$k"));
    }

    #[test]
    fn test_embedding_similarity_uses_embedding_parameter() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("$embedding"));
    }

    #[test]
    fn test_embedding_similarity_filters_by_domain() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("domain = $domain"));
    }

    #[test]
    fn test_embedding_similarity_filters_by_threshold() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("score >= $threshold"));
    }

    #[test]
    fn test_embedding_similarity_orders_by_score() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("ORDER BY score DESC"));
    }

    #[test]
    fn test_embedding_similarity_returns_entity_id() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("entity_id"));
    }

    #[test]
    fn test_embedding_similarity_returns_text() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("text"));
    }

    #[test]
    fn test_embedding_similarity_returns_score() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("score"));
    }

    #[test]
    fn test_upsert_entity_query_length_reasonable() {
        let query = build_upsert_entity();
        assert!(query.len() > 50);
        assert!(query.len() < 500);
    }

    #[test]
    fn test_upsert_relationship_query_length_reasonable() {
        let query = build_upsert_relationship();
        assert!(query.len() > 100);
        assert!(query.len() < 500);
    }

    #[test]
    fn test_embedding_similarity_query_length_reasonable() {
        let query = build_find_by_embedding_similarity();
        assert!(query.len() > 100);
        assert!(query.len() < 500);
    }

    #[test]
    fn test_upsert_entity_uses_on_create_set() {
        let query = build_upsert_entity();
        assert!(query.contains("ON CREATE SET"));
    }

    #[test]
    fn test_upsert_entity_uses_on_match_set() {
        let query = build_upsert_entity();
        assert!(query.contains("ON MATCH SET"));
    }

    #[test]
    fn test_upsert_relationship_uses_on_create_set() {
        let query = build_upsert_relationship();
        assert!(query.contains("ON CREATE SET"));
    }

    #[test]
    fn test_upsert_relationship_uses_on_match_set() {
        let query = build_upsert_relationship();
        assert!(query.contains("ON MATCH SET"));
    }

    #[test]
    fn test_queries_are_static_strings() {
        // Verify queries are static and can be called multiple times
        let q1 = build_upsert_entity();
        let q2 = build_upsert_entity();
        assert_eq!(q1, q2);
        assert_eq!(q1.as_ptr(), q2.as_ptr()); // Same memory location
    }
}
