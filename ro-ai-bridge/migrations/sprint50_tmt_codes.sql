-- Sprint 50 W2.3b — TMT (Thai Medicines Terminology) master + relationships
--
-- TMT = Thai dm+d-style drug ontology, maintained by Thai Health Information
-- Standards Development Center (THIS-Center) under MoPH. Powers FHIR
-- MedicationRequest.medicationCodeableConcept binding.
--
-- 8-layer concept hierarchy (UK NHS dm+d adapted for Thailand):
--   SUBS  — Substance (active ingredient)
--   VTM   — Virtual Therapeutic Moiety (substance + abstract form)
--   GP    — Generic Product (generic + dose form)
--   GPP   — Generic Product Package
--   GPU   — Generic Product Unit (generic + unit dose)
--   TP    — Trade Product (brand)
--   TPP   — Trade Product Package
--   TPU   — Trade Product Unit
--
-- Tenant model (mirrors icd10_codes/loinc_codes):
--   - tenant_id = NULL    → shared master (every tenant)
--   - tenant_id = <slug>  → per-tenant overrides (rare)

CREATE TABLE tmt_codes (
    tmt_id            VARCHAR(20)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    -- Discriminator across the 8 concept layers.
    concept_type      VARCHAR(8)   NOT NULL,
    -- Fully Specified Name (TMT canonical display).
    fsn               TEXT         NOT NULL,
    -- Only populated for TP / TPU (brand-bound concepts).
    manufacturer      TEXT         DEFAULT NULL,
    -- YYYYMMDD from TMT release; nullable for safety.
    change_date       DATE         DEFAULT NULL,
    locale_metadata   LONGTEXT     DEFAULT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (tmt_id, source_version),
    KEY idx_concept_type (concept_type),
    KEY idx_tenant (tenant_id),
    KEY idx_source_version (source_version),
    FULLTEXT KEY ft_fsn (fsn)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 10 relationship tables in the TMT release collapse into one with a
-- `rel_type` discriminator. Encodes the hierarchy:
--   SUBStoVTM, VTMtoGP, GPtoGPU, GPtoTP, GPUtoGPP, GPUtoTPU,
--   GPPtoGPP, GPPtoTPP, TPtoTPU, TPUtoTPP, TPPtoTPP
CREATE TABLE tmt_relationships (
    from_id           VARCHAR(20)  NOT NULL,
    to_id             VARCHAR(20)  NOT NULL,
    rel_type          VARCHAR(20)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (from_id, to_id, rel_type, source_version),
    KEY idx_from (from_id, source_version),
    KEY idx_to   (to_id, source_version),
    KEY idx_rel_type (rel_type),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE tmt_ingest_runs (
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
