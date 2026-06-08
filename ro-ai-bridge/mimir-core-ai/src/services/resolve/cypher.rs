//! Pure Cypher builders for entity resolution / deduplication (Phase 1).
//!
//! Each builder returns a static query string and is unit-tested by asserting on
//! its substrings — the same convention as the builders in `services::neo4j`.
//! Every builder is tenant-scoped: `tenant_id` must appear so a merge or a
//! review edge can never cross tenants (single Mac mini per tenant, but the
//! graph is multi-tenant internally).
//!
//! Phase 1 is flag-only: there is no node-merge or tombstone builder here. The
//! strongest action is proposing a `DUPLICATE_OF` review edge for a human.

/// Store the ingest-time embedding (and its model/dim stamp) on an `:Entity`.
/// Written separately from `build_upsert_entity_cypher` so re-ingest (which
/// overwrites `properties`) never clobbers the embedding.
pub fn build_store_embedding_cypher() -> &'static str {
    "MATCH (n:Entity {name: $name, entity_type: $entity_type, tenant_id: $tenant_id}) \
     SET n.embedding = $embedding, n.embed_model = $embed_model, n.embed_dim = $embed_dim \
     RETURN elementId(n) AS node_id"
}

/// Persist the resolved canonical name + alias set on an `:Entity`.
pub fn build_set_canonical_and_aliases_cypher() -> &'static str {
    "MATCH (n:Entity {name: $name, entity_type: $entity_type, tenant_id: $tenant_id}) \
     SET n.canonical_name = $canonical_name, n.aliases = $aliases \
     RETURN elementId(n) AS node_id"
}

/// Fetch same-type candidates for resolution/dedup within a tenant. Matches the
/// `:Entity` label only, so global `:PrimeKG` nodes are excluded.
pub fn build_find_candidates_cypher() -> &'static str {
    "MATCH (n:Entity) \
     WHERE n.tenant_id = $tenant_id AND n.entity_type = $entity_type \
     RETURN n.name AS name, \
            coalesce(n.canonical_name, n.name) AS canonical_name, \
            coalesce(n.aliases, []) AS aliases, \
            n.entity_type AS entity_type, \
            n.embedding AS embedding \
     LIMIT $limit"
}

/// Propose a duplicate pair for human review: `(duplicate)-[:DUPLICATE_OF]->(canonical)`.
/// Uses `MERGE` keyed on the pair + `tenant_id` so re-proposing the same pair is
/// idempotent (the dream pass and repeated ingests do not pile up edges).
pub fn build_flag_duplicate_cypher() -> &'static str {
    "MATCH (b:Entity {name: $src_name, entity_type: $entity_type, tenant_id: $tenant_id}) \
     MATCH (a:Entity {name: $dst_name, entity_type: $entity_type, tenant_id: $tenant_id}) \
     MERGE (b)-[d:DUPLICATE_OF {tenant_id: $tenant_id}]->(a) \
     ON CREATE SET d.status = 'pending', \
                   d.confidence = $confidence, \
                   d.score_embed = $score_embed, \
                   d.score_fuzzy = $score_fuzzy, \
                   d.score_method = $score_method, \
                   d.embed_model = $embed_model, \
                   d.embed_dim = $embed_dim, \
                   d.code_match = $code_match, \
                   d.proposed_by = $proposed_by, \
                   d.proposed_at = datetime() \
     RETURN elementId(d) AS rel_id"
}

/// The human review queue: pending duplicate proposals for a tenant, highest
/// confidence first.
pub fn build_review_queue_cypher() -> &'static str {
    "MATCH (b:Entity)-[d:DUPLICATE_OF {tenant_id: $tenant_id}]->(a:Entity) \
     WHERE d.status = 'pending' \
     RETURN coalesce(a.canonical_name, a.name) AS canonical_name, \
            b.name AS duplicate_name, \
            d.confidence AS confidence, \
            d.score_method AS method, \
            d.code_match AS code_match, \
            elementId(d) AS rel_id \
     ORDER BY d.confidence DESC \
     LIMIT $limit"
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builders_are_tenant_scoped() {
        for q in [
            build_store_embedding_cypher(),
            build_set_canonical_and_aliases_cypher(),
            build_find_candidates_cypher(),
            build_flag_duplicate_cypher(),
            build_review_queue_cypher(),
        ] {
            assert!(q.contains("tenant_id"), "builder must be tenant-scoped: {q}");
        }
    }

    #[test]
    fn store_embedding_sets_model_and_dim() {
        let q = build_store_embedding_cypher();
        assert!(q.contains("n.embedding = $embedding"));
        assert!(q.contains("n.embed_model = $embed_model"));
        assert!(q.contains("n.embed_dim = $embed_dim"));
        assert!(!q.contains("n.properties"), "must not touch the properties blob");
    }

    #[test]
    fn set_canonical_sets_both_fields() {
        let q = build_set_canonical_and_aliases_cypher();
        assert!(q.contains("n.canonical_name = $canonical_name"));
        assert!(q.contains("n.aliases = $aliases"));
    }

    #[test]
    fn find_candidates_is_type_gated_and_limited() {
        let q = build_find_candidates_cypher();
        assert!(q.contains("n.entity_type = $entity_type"), "type-gated");
        assert!(q.contains("LIMIT $limit"));
        assert!(!q.contains(":PrimeKG"), "global PrimeKG nodes must be excluded");
        assert!(q.contains("coalesce(n.canonical_name, n.name)"));
    }

    #[test]
    fn flag_duplicate_is_idempotent_and_pending() {
        let q = build_flag_duplicate_cypher();
        assert!(q.contains("MERGE (b)-[d:DUPLICATE_OF {tenant_id: $tenant_id}]->(a)"), "idempotent merge keyed on pair+tenant");
        assert!(q.contains("ON CREATE SET d.status = 'pending'"));
        assert!(!q.contains("SAME_AS"), "must not overload the Entity->PrimeKG SAME_AS edge");
        // audit / explainability fields
        for f in ["d.confidence", "d.score_embed", "d.score_fuzzy", "d.code_match", "d.proposed_by", "d.proposed_at"] {
            assert!(q.contains(f), "missing audit field {f}");
        }
    }

    #[test]
    fn review_queue_orders_by_confidence_desc() {
        let q = build_review_queue_cypher();
        assert!(q.contains("d.status = 'pending'"));
        assert!(q.contains("ORDER BY d.confidence DESC"));
        assert!(q.contains("LIMIT $limit"));
    }
}