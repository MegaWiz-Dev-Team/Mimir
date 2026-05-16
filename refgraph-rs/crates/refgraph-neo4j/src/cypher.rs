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
    fn test_upsert_relationship_query_contains_merge() {
        let query = build_upsert_relationship();
        assert!(query.contains("MERGE"));
        assert!(query.contains("REF_REL"));
        assert!(query.contains("rel_type"));
    }

    #[test]
    fn test_embedding_similarity_query_contains_vector_index() {
        let query = build_find_by_embedding_similarity();
        assert!(query.contains("vector.queryNodes"));
        assert!(query.contains("refgraph_embeddings"));
    }
}
