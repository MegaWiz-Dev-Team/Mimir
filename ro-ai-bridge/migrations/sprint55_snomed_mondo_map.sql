-- Sprint 55 — SNOMED CT ↔ MONDO crosswalk (for PrimeKG entity resolution).
-- Source: MONDO SSSOM (mondo.sssom.tsv, CC-BY-4.0). MONDO is a disease-ontology
-- hub: this single file also maps to Orphanet/ICD10CM/ICD11/DOID/OMIM/UMLS/MeSH/
-- NCIT. We ingest the SNOMED (SCTID) ↔ MONDO subset here; PrimeKG disease nodes
-- are MONDO-coded (entity_id = numeric MONDO id), so this bridges
-- text → SNOMED concept → MONDO → PrimeKG node.
CREATE TABLE IF NOT EXISTS snomed_mondo_map (
    snomed_id     VARCHAR(20)  NOT NULL,   -- SCTID numeric
    mondo_id      INT          NOT NULL,   -- numeric MONDO id (MONDO:0005015 → 5015)
    predicate     VARCHAR(32)  NOT NULL,   -- skos:exactMatch | broadMatch | ...
    mondo_label   VARCHAR(255) DEFAULT NULL,
    source_version VARCHAR(32) DEFAULT NULL,
    tenant_id     VARCHAR(50)  DEFAULT NULL,
    created_at    TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    KEY idx_snomed (snomed_id),
    KEY idx_mondo (mondo_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
