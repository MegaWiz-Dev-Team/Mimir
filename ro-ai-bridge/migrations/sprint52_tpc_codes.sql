-- Sprint 52 W2.3c — TPC (Thai Procedural Classification) master table
--
-- Bootstrap path: ICD-9-CM Volume 3 (procedures) from US CMS as fallback,
-- since the official Thai TPC release from MoPH Bureau of Health Information
-- is license-blocked and B-48a license letter is unanswered. The Thai TPC
-- is derived from this same ICD-9-CM-TH baseline + ~200 Thai-specific
-- additions; using CMS public-domain version covers ~95% of common
-- procedures.
--
-- When official Thai TPC license arrives, ingest a second source_version
-- like 'tpc-moph-2017' and the multi-version PK lets both coexist for
-- lookup cascade.
--
-- Tenant model (mirrors icd10_codes):
--   - tenant_id = NULL    → shared master
--   - tenant_id = <slug>  → per-tenant overrides

CREATE TABLE tpc_codes (
    -- Code in canonical format: 'XX.YY' (e.g., '00.12', '36.06') or
    -- compact 4-digit ('0012', '3606') when no decimal. We store the
    -- canonical decimal form; query-side normalization handles compact.
    code              VARCHAR(8)   NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    en_label          TEXT         NOT NULL,
    -- Thai display — null in v1 (US baseline); populated when Thai TPC
    -- ingests or when a translation pass runs.
    th_label          TEXT         DEFAULT NULL,
    -- ICD-9-CM chapter range (e.g., '01-05' = Nervous System ops).
    chapter           VARCHAR(8)   DEFAULT NULL,
    -- Block within chapter (e.g., '00.1' is the block for code '00.12').
    block             VARCHAR(8)   DEFAULT NULL,
    -- TRUE if usable as a primary procedure on a claim. Default TRUE.
    -- Some 3-char block headers are non-billable; flagged as needed.
    billable_flag     BOOLEAN      NOT NULL DEFAULT TRUE,
    locale_metadata   LONGTEXT     DEFAULT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (code, source_version),
    KEY idx_chapter (chapter),
    KEY idx_block (block),
    KEY idx_tenant (tenant_id),
    KEY idx_source_version (source_version),
    FULLTEXT KEY ft_labels (en_label, th_label)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE tpc_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_skipped      INT          NOT NULL DEFAULT 0,
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
