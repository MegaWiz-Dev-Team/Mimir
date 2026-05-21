-- Sprint 54 — SNOMED CT → ICD-10-TM mapping (POC: insurance + medical).
--
-- Pipeline this powers:
--   clinical text ──FULLTEXT──▶ SNOMED concept ──map──▶ WHO ICD-10 ──bridge──▶ ICD-10-TM
--   (snomed_descriptions)        (snomed_icd10_map)        (self-derived from icd10_codes)
--
-- Source of the map: SNOMED ExtendedMap, pre-transformed by MoPH officer into
-- the flat per-(concept,gender,age) schema (data/cts_transformed/*.txt). We
-- normalize that here: explode pipe-delimited targets into one row each, parse
-- the per-target advice verb into a role, and resolve each WHO target to an
-- ICD-10-TM code by joining the existing icd10_codes master.
--
-- The WHO→TM bridge is SELF-DERIVED (no external valueset needed): ICD-10-TM is
-- a superset of WHO ICD-10 at the 3–5 char level, so exact-match (dot-stripped)
-- covers ~92% and 3–4 char roll-up brings it to ~94%. The In10TM/IsTerminal
-- flags from a ClaML valueset are a future quality upgrade, not a prerequisite.
--
-- Tenant model (mirrors icd10_codes/tmt_codes): tenant_id=NULL → shared master.

-- ── snomed_descriptions: text → concept search surface ──────────────────────
CREATE TABLE IF NOT EXISTS snomed_descriptions (
    concept_id      VARCHAR(20)  NOT NULL,
    source_version  VARCHAR(32)  NOT NULL,
    term            TEXT         NOT NULL,
    -- 'fsn' (fully specified name) or 'synonym'.
    term_type       VARCHAR(8)   NOT NULL,
    -- Semantic tag parsed from FSN, e.g. "(disorder)", "(finding)", "(procedure)".
    semantic_tag    VARCHAR(64)  DEFAULT NULL,
    active          TINYINT(1)   NOT NULL DEFAULT 1,
    tenant_id       VARCHAR(50)  DEFAULT NULL,
    created_at      TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    KEY idx_concept (concept_id),
    KEY idx_semtag (semantic_tag),
    FULLTEXT KEY ft_term (term)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── snomed_icd10_map: normalized concept → ICD map (one row per target) ──────
CREATE TABLE IF NOT EXISTS snomed_icd10_map (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    source_version  VARCHAR(32)  NOT NULL,
    concept_id      VARCHAR(20)  NOT NULL,
    -- Conditional keys: NULL = applies to any. Pre-split by the MoPH transform.
    gender          VARCHAR(1)   DEFAULT NULL,   -- 'M' | 'F' | NULL(any)
    age_group       VARCHAR(16)  DEFAULT NULL,   -- neonatal|pediatric|adolescent|adult|geriatric|NULL(any)
    -- WHO ICD-10 target, kept WITH the dot as it appears upstream (e.g. "A00.0").
    icd10_who       VARCHAR(10)  NOT NULL,
    -- Resolved ICD-10-TM code, dot-stripped (e.g. "A000"); NULL when absent in TM.
    icd10_tm        VARCHAR(10)  DEFAULT NULL,
    -- How icd10_tm was derived: 'exact' | 'rollup' | 'absent'.
    match_tier      VARCHAR(8)   NOT NULL,
    -- Reconstructed from the advice verb: 'mandatory'(ALWAYS) | 'conditional'(IF…CHOOSE) | 'advisory'.
    target_role     VARCHAR(12)  NOT NULL,
    map_advice      TEXT,
    -- Set when category was cannot-classify / context-dependent, or TM match absent.
    needs_review    TINYINT(1)   NOT NULL DEFAULT 0,
    tenant_id       VARCHAR(50)  DEFAULT NULL,
    created_at      TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_concept (concept_id, gender, age_group),
    KEY idx_who (icd10_who),
    KEY idx_tm (icd10_tm),
    KEY idx_review (needs_review)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── audit trail (mirrors icd10_ingest_runs / tmt_ingest_runs) ────────────────
CREATE TABLE IF NOT EXISTS snomed_map_ingest_runs (
    id                 VARCHAR(36)  NOT NULL,
    source_version     VARCHAR(32)  NOT NULL,
    source_label       VARCHAR(100) NOT NULL,
    started_at         TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    finished_at        TIMESTAMP    NULL DEFAULT NULL,
    rows_descriptions  INT          DEFAULT 0,
    rows_map           INT          DEFAULT 0,
    rows_tm_exact      INT          DEFAULT 0,
    rows_tm_rollup     INT          DEFAULT 0,
    rows_tm_absent     INT          DEFAULT 0,
    status             VARCHAR(16)  DEFAULT 'running',
    notes              TEXT,
    PRIMARY KEY (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
