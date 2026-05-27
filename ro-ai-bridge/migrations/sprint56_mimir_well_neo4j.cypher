// Sprint 56 — Mimir Well: Neo4j schema (indexes + constraints)
//
// Companion to sprint56_mimir_well_schema.sql. The Neo4j layer holds:
//   - (:Artifact)              mirrors mimir.memory_artifact rows (tenant-scoped)
//   - (:Span)                  pointers to heimdall-trace spans (trace_id, span_id)
//   - (:Artifact)-[:DERIVED_FROM|:USED_IN|:REFINES|:CONTRADICTS]->(:Artifact)
//   - (:Span)-[:TOUCHED]->(:Artifact)   materialized from OTel span attrs
//
// POLE+O sub-labels (asgard_insurance only):
//   :Artifact:Person, :Artifact:Object, :Artifact:Location,
//   :Artifact:Event,  :Artifact:Organization
//
// IMPORTANT: This migration runs `MERGE`-style — idempotent. But before
// applying, take a Neo4j-only backup (ADR-011 §D6):
//
//   cd Asgard/ && TAG=pre-well-neo4j ./scripts/backup-neo4j-only.sh
//
// Apply:
//   cypher-shell -u <user> -p <pass> -d neo4j -f sprint56_mimir_well_neo4j.cypher
// Or via kubectl exec:
//   kubectl exec -i -n asgard-infra neo4j-0 -- \
//     cypher-shell -u neo4j -p "$NEO4J_PASS" < sprint56_mimir_well_neo4j.cypher
//
// Cross-refs:
//   Asgard/docs/decisions/ADR-011-mimir-well-memory-artifacts.md §D1, D2, D4
//   Mimir/ro-ai-bridge/mimir-well/src/touched.rs (Cypher MERGE template)
//   Mimir/ro-ai-bridge/mimir-well/src/pole_o.rs  (POLE+O scope guard)

// ── 1. Uniqueness constraints (Artifact + Span) ─────────────────────────
CREATE CONSTRAINT artifact_id_unique IF NOT EXISTS
  FOR (a:Artifact) REQUIRE a.id IS UNIQUE;

CREATE CONSTRAINT span_lookup_unique IF NOT EXISTS
  FOR (s:Span) REQUIRE (s.trace_id, s.span_id) IS UNIQUE;

// ── 2. Lookup indexes ───────────────────────────────────────────────────
// Artifact list-by-tenant-tier-time (mimir-well reader.search hot path)
CREATE INDEX artifact_tenant_tier IF NOT EXISTS
  FOR (a:Artifact) ON (a.tenant_id, a.tier);

CREATE INDEX artifact_tenant_created IF NOT EXISTS
  FOR (a:Artifact) ON (a.tenant_id, a.created_at);

// Consolidation lifecycle queries
CREATE INDEX artifact_consolidation_state IF NOT EXISTS
  FOR (a:Artifact) ON (a.tenant_id, a.consolidation_state);

// content_hash lookup for auto-merge fast path
CREATE INDEX artifact_content_hash IF NOT EXISTS
  FOR (a:Artifact) ON (a.tenant_id, a.content_hash);

// Span timestamp filter (heimdall-trace deeplink ordering)
CREATE INDEX span_at IF NOT EXISTS
  FOR (s:Span) ON (s.at);

// ── 3. POLE+O sub-label indexes (asgard_insurance scope) ────────────────
// These match the sub-labels emitted by mimir-well/src/pole_o.rs.
// They are independent of the base :Artifact indexes — Neo4j applies the
// most selective. Filter on tenant_id in queries.
CREATE INDEX pole_person_tenant       IF NOT EXISTS FOR (n:Person)       ON (n.tenant_id);
CREATE INDEX pole_object_tenant       IF NOT EXISTS FOR (n:Object)       ON (n.tenant_id);
CREATE INDEX pole_location_tenant     IF NOT EXISTS FOR (n:Location)     ON (n.tenant_id);
CREATE INDEX pole_event_tenant        IF NOT EXISTS FOR (n:Event)        ON (n.tenant_id);
CREATE INDEX pole_organization_tenant IF NOT EXISTS FOR (n:Organization) ON (n.tenant_id);

// ── 4. Full-text index for fallback text search ─────────────────────────
// Used when BGE-M3 vector path is unavailable / for keyword filters.
CREATE FULLTEXT INDEX artifact_content_fulltext IF NOT EXISTS
  FOR (a:Artifact) ON EACH [a.title, a.summary];

// ── 5. Verification — run after apply ───────────────────────────────────
// Expected: 11 indexes + 2 constraints. Inspect with:
//   SHOW INDEXES YIELD name, type, state WHERE name STARTS WITH 'artifact_' OR name STARTS WITH 'pole_' OR name STARTS WITH 'span_';
//   SHOW CONSTRAINTS YIELD name WHERE name IN ['artifact_id_unique','span_lookup_unique'];
