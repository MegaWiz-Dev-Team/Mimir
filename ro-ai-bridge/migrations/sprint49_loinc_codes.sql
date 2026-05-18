-- Sprint 49 (W2.3a) — LOINC ValueSet master table
--
-- LOINC = Logical Observation Identifiers Names and Codes (Regenstrief Institute).
-- Powers FHIR `Observation.code` binding in Phase B.3.
--
-- License: Free for any use under the LOINC license; requires (free) account
-- registration at https://loinc.org/downloads/. Master file `Loinc.csv` ships
-- in `LOINC_<version>_Source.zip`.
--
-- Tenant model (mirrors icd10_codes):
--   - tenant_id = NULL   → shared master (everyone sees the same LOINC codes)
--   - tenant_id = <slug> → reserved for per-tenant supplemental codes (rare)

CREATE TABLE loinc_codes (
    -- Primary key (loinc_num, source_version) to keep multi-vintage in parallel.
    -- loinc_num is the canonical "1234-5" identifier; max len in current LOINC
    -- 2.78 is 7 chars but reserve more for future longer IDs.
    loinc_num         VARCHAR(16)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    -- Display names. LONG_COMMON_NAME is the patient-friendly form;
    -- SHORTNAME / COMPONENT are the analytic form.
    long_common_name  TEXT         NOT NULL,
    short_name        TEXT         DEFAULT NULL,
    component         TEXT         DEFAULT NULL,
    -- Six-axis (Property/Time/System/Scale/Method most clinically used).
    property          VARCHAR(64)  DEFAULT NULL,
    time_aspct        VARCHAR(32)  DEFAULT NULL,
    -- 'system' is reserved in some dialects; some LOINC SYSTEM values exceed 100 chars
    -- (specimen + body site + qualifier compound) so use TEXT.
    system_axis       TEXT         DEFAULT NULL,
    scale_typ         VARCHAR(16)  DEFAULT NULL,
    -- METHOD_TYP can be long in LOINC v2.82 (max observed 134 chars).
    method_typ        TEXT         DEFAULT NULL,
    -- LOINC class (e.g. CHEM, HEM/BC) — coarse grouping for UI/categorization.
    class             VARCHAR(64)  DEFAULT NULL,
    -- Status: ACTIVE / DEPRECATED / DISCOURAGED / TRIAL (LOINC STATUS column).
    status            VARCHAR(16)  DEFAULT NULL,
    -- Example UCUM unit (LOINC EXAMPLE_UCUM_UNITS) — informational.
    example_ucum      VARCHAR(64)  DEFAULT NULL,
    -- Free-form metadata (units of measure, related names list, etc.).
    locale_metadata   LONGTEXT     DEFAULT NULL,
    -- Multi-tenant scoping (NULL = shared master).
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    -- Audit.
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (loinc_num, source_version),
    KEY idx_class (class),
    KEY idx_status (status),
    KEY idx_tenant (tenant_id),
    KEY idx_source_version (source_version),
    -- For free-text lookup on Observation.code fallback when codes aren't given.
    FULLTEXT KEY ft_names (long_common_name, short_name, component)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Ingest audit (mirrors icd10_ingest_runs).
CREATE TABLE loinc_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_updated      INT          NOT NULL DEFAULT 0,
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
