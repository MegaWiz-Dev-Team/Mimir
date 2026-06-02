-- Sprint 60 — SNOMED CT → ICD-10-CM map (US Edition official ExtendedMap).
--
-- (Sprint 59 is the Sprint-58-assets cross-cutting work — see
--  docs/03_implementation_plans/03_20_SNOMED_Assets_CrossCutting_Plan.md.
--  This ICD-10-CM map is a separate addition, hence Sprint 60.)
--
-- DISTINCT from sprint54_snomed_icd10_map (which is ICD-10-*TM*, the Thai
-- Modification, with a self-derived WHO→TM bridge). This table holds the
-- OFFICIAL SNOMED CT → ICD-10-CM map shipped inside the SNOMED CT US Edition
-- by the U.S. NLM as an RF2 "Extended Map" refset (refsetId 6011000124106).
--
-- It is a RULE-BASED map, NOT 1:1:
--   * one SNOMED concept → 1..N candidate ICD-10-CM targets, partitioned by
--     mapGroup; within a group the candidates are tried in mapPriority order
--     (first rule that matches wins)
--   * each candidate carries a mapRule (machine condition: "TRUE", or
--     "IFA <conceptId> | <term> |" gender/age/finding gate, or "OTHERWISE TRUE")
--     and a mapAdvice (human guidance, e.g. "ALWAYS E11.9")
--   * mapCategory states whether the source concept is classifiable at all
--
-- LICENSE: the SNOMED CT US Edition and its ICD-10-CM map are produced by the
-- U.S. NLM and distributed via UMLS / MLDS. Use requires a UMLS Metathesaurus
-- License (free) IN ADDITION to the SNOMED Affiliate License already held.
-- This is a US (ICD-10-CM) artifact — for Thai claims use snomed_icd10_map
-- (ICD-10-TM). See DATA_LICENSE.md.
--
-- Tenant model (mirrors icd10_codes / snomed_icd10_map): tenant_id=NULL = shared.

-- ── snomed_icd10cm_map: one row per candidate (concept, group, priority) ─────
CREATE TABLE IF NOT EXISTS snomed_icd10cm_map (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    source_version  VARCHAR(32)  NOT NULL,
    -- RF2 refsetId — 6011000124106 for the SNOMED CT to ICD-10-CM map.
    refset_id       VARCHAR(20)  NOT NULL,
    -- RF2 referencedComponentId = the source SNOMED concept.
    concept_id      VARCHAR(20)  NOT NULL,
    -- A source concept maps to one or more groups; within a group the candidate
    -- targets are tried in mapPriority order (first matching rule wins).
    map_group       INT          NOT NULL DEFAULT 1,
    map_priority    INT          NOT NULL DEFAULT 1,
    -- Machine-readable condition: "TRUE" (unconditional) | "IFA <conceptId> | <term> |"
    -- (gender/age/finding gate) | "OTHERWISE TRUE" (closes a conditional group).
    map_rule        TEXT,
    -- Human-readable guidance, e.g. "ALWAYS E11.9" / "CONSIDER ADDITIONAL CODE".
    map_advice      TEXT,
    -- Target ICD-10-CM code, kept WITH the dot as published (e.g. "E11.9").
    -- NULL when mapCategory says the source concept is not classifiable.
    icd10cm_code    VARCHAR(16)  DEFAULT NULL,
    -- SNOMED correlation concept id (usually unused in the CM map).
    correlation_id  VARCHAR(20)  DEFAULT NULL,
    -- RF2 mapCategoryId (a SNOMED concept) + decoded short label.
    map_category_id VARCHAR(20)  DEFAULT NULL,
    map_category    VARCHAR(48)  DEFAULT NULL,
    -- 1 when the concept is not properly classified, the target is empty, or the
    -- category is cannot-classify / context-dependent / ambiguous / not-mappable.
    needs_review    TINYINT(1)   NOT NULL DEFAULT 0,
    active          TINYINT(1)   NOT NULL DEFAULT 1,
    tenant_id       VARCHAR(50)  DEFAULT NULL,
    created_at      TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_concept (concept_id, map_group, map_priority),
    KEY idx_target (icd10cm_code),
    KEY idx_category (map_category_id),
    KEY idx_review (needs_review),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── audit trail ──────────────────────────────────────────────────────────────
-- Mirrors loinc_ingest_runs (status 'COMPLETED' + tenant_id column) rather than
-- the sprint54/58 SNOMED audit tables, so the Shared Knowledge screen's
-- last-refresh query (status='COMPLETED' AND tenant_id IS NULL) actually resolves.
CREATE TABLE IF NOT EXISTS snomed_icd10cm_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_review       INT          NOT NULL DEFAULT 0,
    rows_skipped      INT          NOT NULL DEFAULT 0,
    concepts_mapped   INT          NOT NULL DEFAULT 0,
    status            VARCHAR(20)  NOT NULL DEFAULT 'RUNNING',
    status_message    TEXT         DEFAULT NULL,
    started_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    finished_at       TIMESTAMP    NULL DEFAULT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    notes             TEXT         DEFAULT NULL,
    PRIMARY KEY (id),
    KEY idx_source_version (source_version),
    KEY idx_status (status),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
