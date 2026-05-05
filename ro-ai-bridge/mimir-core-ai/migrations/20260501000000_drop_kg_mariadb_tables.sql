-- Migration: Drop MariaDB KG tables after full Neo4j cutover
--
-- Prerequisites before running this migration:
--   1. USE_NEO4J_GRAPH=true deployed and stable for >= 14 days
--   2. No active KG extraction jobs (check kg_extraction_runs WHERE status = 'running')
--   3. Neo4j data verified: entity count >= MariaDB entity count
--
-- kg_extraction_runs is intentionally kept — it tracks pipeline job history.
-- kg_relations must be dropped before kg_entities (foreign key constraint).

DROP TABLE IF EXISTS kg_relations;
DROP TABLE IF EXISTS kg_entities;
