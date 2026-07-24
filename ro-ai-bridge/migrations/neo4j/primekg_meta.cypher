// Sprint 55 — PrimeKG version pin as a graph node.
//
// PrimeKG is a FROZEN research snapshot (Harvard Dataverse, ad-hoc releases: v1 2021,
// v2 2022, no v3), not a live-updated feed like RxNorm. Pin the loaded version IN the
// graph so /api/v1/knowledge/shared reports what is actually loaded (read by
// Neo4jService::primekg_meta_version) instead of a hardcoded string that can drift from
// reality. Staleness — not drift — is the risk: drugs newer than the snapshot resolve
// via RxNorm/TMT but have no PrimeKG node, and surface as residual in
// scripts/rxnorm_primekg_bridge.py coverage.
//
// Idempotent (MERGE on the singleton (:Meta {kb:'primekg'}) node). Run it as part of the
// PrimeKG load / deploy step with real Neo4j credentials — NOT an ad-hoc prod write:
//
//   kubectl -n asgard-infra exec -i deploy/neo4j -- \
//     cypher-shell -u neo4j -p "$NEO4J_PASSWORD" -f - < primekg_meta.cypher
//
// Bump `version`/`dataset_vintage` here whenever a new PrimeKG snapshot is loaded.

MATCH (n:PrimeKG)
WITH count(n) AS node_count
MERGE (m:Meta {kb: 'primekg'})
SET m.version             = 'primekg-v2',
    m.dataset_vintage     = 2022,
    m.doi                 = '10.7910/DVN/IXA7BM',
    m.source              = 'Harvard Dataverse (Marinka Zitnik lab)',
    m.update_cadence      = 'ad-hoc (v1 2021, v2 2022, no v3 announced)',
    m.node_count_at_load  = node_count,
    m.loaded_at           = datetime()
RETURN m.kb AS kb, m.version AS version, m.dataset_vintage AS vintage,
       m.node_count_at_load AS nodes;
