-- Sprint 51 — TMLT (Thai Medical Laboratory Terminology) master + relationships
--
-- TMLT = Thai lab/observation ontology, maintained by THIS-Center / MoPH.
-- Complements LOINC: Thai display names + Thai-specific lab tests that
-- aren't in international LOINC. Used together with LOINC for FHIR
-- Observation.code (LOINC for international wire, TMLT for Thai UI).
--
-- 2-layer concept hierarchy (much simpler than TMT's 8 layers):
--   ITEM   — individual lab test (4,758 in v20260501)
--   PANEL  — grouping of tests (e.g. CBC = WBC + RBC + Hgb + ...; 403 panels)
--
-- Single relationship: PANELtoITEM (444 pairs).
--
-- Tenant model (mirrors loinc_codes / tmt_codes):
--   - tenant_id = NULL    → shared master
--   - tenant_id = <slug>  → per-tenant overrides

CREATE TABLE tmlt_codes (
    tmlt_id           VARCHAR(20)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    -- ITEM or PANEL
    concept_type      VARCHAR(8)   NOT NULL,
    -- Fully Specified Name (canonical display, EN + Thai).
    fsn               TEXT         NOT NULL,
    change_date       DATE         DEFAULT NULL,
    locale_metadata   LONGTEXT     DEFAULT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (tmlt_id, source_version),
    KEY idx_concept_type (concept_type),
    KEY idx_tenant (tenant_id),
    KEY idx_source_version (source_version),
    FULLTEXT KEY ft_fsn (fsn)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE tmlt_relationships (
    panel_id          VARCHAR(20)  NOT NULL,
    item_id           VARCHAR(20)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (panel_id, item_id, source_version),
    KEY idx_panel (panel_id, source_version),
    KEY idx_item  (item_id, source_version),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE tmlt_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_relationships INT         NOT NULL DEFAULT 0,
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
