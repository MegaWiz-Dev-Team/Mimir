-- Sprint 60 — SNOMED CT → ICD-10 ExtendedMap (generic: ICD-10-CM US + ICD-10 WHO).
--
-- (Sprint 59 is the Sprint-58-assets cross-cutting work — see
--  docs/03_implementation_plans/03_20_SNOMED_Assets_CrossCutting_Plan.md.
--  This SNOMED→ICD-10 map is a separate addition, hence Sprint 60.)
--
-- ONE table holds BOTH official RF2 "Extended Map" refsets, discriminated by
-- `target_system`:
--   * 'icd10cm'  — SNOMED CT US Edition → ICD-10-CM (US), refset 6011000124106
--   * 'icd10who' — SNOMED CT International → ICD-10 (WHO),  refset 447562003
-- Both ship in identical RF2 ExtendedMap layout (13 cols), so one schema + one
-- ingest path serves both; adding CM later (once UMLS is approved) is just rows,
-- no code change.
--
-- DISTINCT from sprint54_snomed_icd10_map (which is ICD-10-*TM*, the Thai
-- Modification, with a self-derived WHO→TM bridge). This table holds OFFICIAL
-- rule-based maps, ingested verbatim — NOT 1:1:
--   * one SNOMED concept → 1..N candidate targets, partitioned by mapGroup;
--     within a group candidates are tried in mapPriority order (first rule wins)
--   * each candidate carries a mapRule ("TRUE" | "IFA <id> | <term> |" gender/age/
--     finding gate | "OTHERWISE TRUE") and a mapAdvice (human guidance)
--   * mapCategory states whether the source concept is classifiable at all
--
-- LICENSE: both are produced under the SNOMED Affiliate License already held.
--   * icd10who — SNOMED CT International Edition (Affiliate License covers it).
--   * icd10cm  — additionally requires a UMLS Metathesaurus License (free); the
--     US Edition is distributed via UMLS/MLDS. See DATA_LICENSE.md.
-- For Thai claims use snomed_icd10_map (ICD-10-TM) instead.
--
-- Tenant model (mirrors icd10_codes / snomed_icd10_map): tenant_id=NULL = shared.

-- ── snomed_icd10_extmap: one row per candidate (target_system, concept, group, priority) ──
CREATE TABLE IF NOT EXISTS snomed_icd10_extmap (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    -- Which classification the target belongs to: 'icd10cm' (US) | 'icd10who'.
    target_system   VARCHAR(12)  NOT NULL,
    source_version  VARCHAR(32)  NOT NULL,
    -- RF2 refsetId — 6011000124106 (ICD-10-CM) | 447562003 (ICD-10 WHO complex map).
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
    -- Target ICD-10 code, kept WITH the dot as published (e.g. "E11.9").
    -- NULL when mapCategory says the source concept is not classifiable.
    icd10_code      VARCHAR(16)  DEFAULT NULL,
    -- SNOMED correlation concept id (usually unused).
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
    KEY idx_concept (target_system, concept_id, map_group, map_priority),
    KEY idx_target (target_system, icd10_code),
    KEY idx_category (map_category_id),
    KEY idx_review (needs_review),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── audit trail ──────────────────────────────────────────────────────────────
-- Mirrors loinc_ingest_runs (status 'COMPLETED' + tenant_id column) so the
-- Shared Knowledge screen's last-refresh query (status='COMPLETED' AND
-- tenant_id IS NULL) resolves. One run row per (target_system, source_version).
CREATE TABLE IF NOT EXISTS snomed_icd10_extmap_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    target_system     VARCHAR(12)  NOT NULL,
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
    KEY idx_target_system (target_system),
    KEY idx_source_version (source_version),
    KEY idx_status (status),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
