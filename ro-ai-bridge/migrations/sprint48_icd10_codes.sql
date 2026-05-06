-- Sprint 48 — Thai Clinical Coding Foundation (ICD-10 + ICD-10-TM)
--
-- Master reference data for ICD-10 / ICD-10-TM. Treated as slowly-changing
-- dimension: load once per source-version, query often.
--
-- Tenant model:
--   - tenant_id = NULL    → shared master (Thai national standard, all tenants
--                            see the same rows; default for all anamai/MoPH ingests)
--   - tenant_id = <slug>  → reserved for per-tenant overrides/extensions (rare,
--                            e.g. hospital-internal supplemental codes)
--
-- Phase A source: anamai (Department of Health, central MoPH) ICD-10-TM PDF
--   https://backenddc.anamai.moph.go.th/coverpage/d1579eb1c80b878ab62513c060681290.pdf
--   Vintage 2010 (pre ICD-10-TM 2017). License gray (no explicit terms;
--   default = Thai gov public document). Refresh path: B-48a license letter
--   → Bureau of Health Information for ICD-10-TM 2017 master + commercial terms.
--
-- See:
--   - Asgard/skills/icd10-coding/SKILL.md
--   - Mimir/docs/03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md (Sprint 48)
--   - Asgard/legal/2026-05-07_MoPH_ICD-10-TM_License_Request.md (B-48a)

-- ─────────────────────────────────────────────────────────────────────
-- icd10_codes: master code table (bilingual, multi-source-version)
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE icd10_codes (
    -- Primary key is (code, source_version) so multiple ingests can coexist
    -- and we can audit/rollback per-version. Resolution to "current code"
    -- happens at lookup time via active_version filter.
    code              VARCHAR(8)   NOT NULL,
    source_version    VARCHAR(64)  NOT NULL,
    -- Bilingual labels.
    en_label          TEXT         NOT NULL,
    th_label          TEXT         DEFAULT NULL,
    -- WHO ICD-10 chapter (e.g. "I" Roman or "1" Arabic) derived from code prefix.
    -- Block (e.g. A00-A09) is more granular; deferred to a refresh sprint.
    chapter           VARCHAR(8)   DEFAULT NULL,
    block             VARCHAR(16)  DEFAULT NULL,
    -- True if code can be a primary diagnosis on a claim. Default TRUE; we'll
    -- learn billable_flag exceptions when we ingest DRG mapping (B-48g).
    billable_flag     BOOLEAN      NOT NULL DEFAULT TRUE,
    -- DRG group from สปสช. v6 (filled by B-48g; NULL until then).
    drg_id            VARCHAR(16)  DEFAULT NULL,
    -- Source-specific group code (e.g. anamai รหัสกลุมโรค 1..298) and other
    -- locale-specific metadata go here. JSON for forward-compat.
    locale_metadata   LONGTEXT     DEFAULT NULL,
    -- Multi-tenant scoping (NULL = shared master).
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    -- Audit.
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (code, source_version),
    KEY idx_chapter (chapter),
    KEY idx_block (block),
    KEY idx_drg (drg_id),
    KEY idx_tenant (tenant_id),
    KEY idx_source_version (source_version),
    -- Bilingual fulltext for `mode=Prefix` and naive-search fallback. Qdrant
    -- collection icd10-th handles semantic search (B-48f).
    FULLTEXT KEY ft_labels (en_label, th_label)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────
-- icd10_ingest_runs: audit trail for each ingest (which source_version,
-- when, how many rows, who/what triggered, source URL).
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE icd10_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(64)  NOT NULL,
    -- Free-form, e.g. "anamai-moph-2010-pdf", "who-icd10-2019", "moph-tm-2017".
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    -- Counts.
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_updated      INT          NOT NULL DEFAULT 0,
    rows_skipped      INT          NOT NULL DEFAULT 0,
    -- Process.
    status            VARCHAR(20)  NOT NULL DEFAULT 'RUNNING',
    status_message    TEXT         DEFAULT NULL,
    started_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    finished_at       TIMESTAMP    NULL DEFAULT NULL,
    -- Multi-tenant: NULL for shared master ingest.
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    -- Optional: notes from the operator.
    notes             TEXT         DEFAULT NULL,
    PRIMARY KEY (id),
    KEY idx_source_version (source_version),
    KEY idx_status (status),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
