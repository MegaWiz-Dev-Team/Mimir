-- Sprint 58 — SNOMED refset membership (IPS, GP/FP) + EDQM dose-form map.
--
-- SOURCE / PROVENANCE
--   Distributor : SNOMED International, via the Member Licensing & Distribution
--                 Service (MLDS) — https://mlds.ihtsdotools.org/#/userDashboard
--   Member      : Thailand (downloaded under the IHTSDO Affiliate License 2023,
--                 license PDFs co-located at $MIMIR_KB/SnomedCT/*.pdf)
--   License gate : SNOMED Affiliate License (commercial-cleared per DATA_LICENSE.md);
--                  SNOMED ≤180d upgrade obligation applies to every package below.
--
-- Three SNOMED International packages, all already on disk under
-- $MIMIR_KB/SnomedCT/, ingested by scripts/snomed_refset_ingest.py:
--
--   IPS  SnomedCT_IPS_PRODUCTION_20250930      simple refset, ~12.7k members
--   GPFP SnomedCT_GPFP_PRODUCTION_20260331      simple refset (reasons-for-encounter
--                                               + health issues primary-care subset)
--   EDQM SnomedCT_SNOMEDEDQMMapPackage_20250930 SimpleMap, 328 dose-form rows
--
-- Design choice (mirrors how tmt_relationships collapsed 10 release tables into one
-- with a `rel_type` discriminator): IPS and GP/FP are BOTH simple refsets — a flat
-- (refset, concept) membership list — so they share ONE generic table keyed by a
-- short `refset_key`. Future simple refsets (NCPT, ICNP…) add a key, not a table.
--
-- These tables hold NO new concept text. They are MEMBERSHIP/MAP overlays that join
-- against the existing International Edition surface (snomed_descriptions). That is
-- why the IPS package ships a 2-row Concept file — the concepts live in International,
-- which we already ingest. Ingest order therefore REQUIRES International descriptions
-- present first (snomed_descriptions populated by snomed_icd10_map_ingest.py --desc-file).
--
-- Tenant model (mirrors icd10_codes/tmt_codes/snomed_icd10_map): tenant_id=NULL = shared.

-- ── snomed_refset_members: generic simple-refset membership (IPS, GP/FP, …) ──────
CREATE TABLE IF NOT EXISTS snomed_refset_members (
    -- Short human discriminator: 'ips' | 'gpfp' (and future refsets).
    refset_key      VARCHAR(16)  NOT NULL,
    -- The SNOMED refset concept id from RF2 refsetId column (e.g. IPS=816080008).
    refset_id       VARCHAR(20)  NOT NULL,
    -- RF2 referencedComponentId = the member SNOMED concept.
    concept_id      VARCHAR(20)  NOT NULL,
    source_version  VARCHAR(32)  NOT NULL,
    active          TINYINT(1)   NOT NULL DEFAULT 1,
    tenant_id       VARCHAR(50)  DEFAULT NULL,
    created_at      TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (refset_key, concept_id, source_version),
    KEY idx_refset (refset_key, active),
    KEY idx_concept (concept_id),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── snomed_edqm_dose_map: SNOMED dose-form concept → EDQM standard term code ──────
-- Source: der2_ssRefset_EDQMSimpleMapSnapshot (referencedComponentId=SNOMED dose-form
-- concept, mapTarget=EDQM code, correlationId=equivalence strength).
-- Purpose: give TMT GP/GPU dose forms (currently free-text inside tmt_codes.fsn) a
-- coded target for FHIR Medication.doseForm. The TMT→SNOMED-dose-form resolution is a
-- SEPARATE step (text match, see snomed_tmt_dose_link below); this table is the
-- SNOMED↔EDQM half only and is exact (328 rows, no review needed).
CREATE TABLE IF NOT EXISTS snomed_edqm_dose_map (
    snomed_concept_id VARCHAR(20) NOT NULL,   -- SNOMED dose-form concept (referencedComponentId)
    edqm_code         VARCHAR(20) NOT NULL,   -- EDQM Standard Terms code (mapTarget)
    -- SNOMED correlation concept: 447557004 exact | 447559001 broad | 447558009 narrow.
    correlation_id    VARCHAR(20) DEFAULT NULL,
    source_version    VARCHAR(32) NOT NULL,
    active            TINYINT(1)  NOT NULL DEFAULT 1,
    tenant_id         VARCHAR(50) DEFAULT NULL,
    created_at        TIMESTAMP   DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (snomed_concept_id, edqm_code, source_version),
    KEY idx_edqm (edqm_code),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── snomed_tmt_dose_link: resolved TMT GP/GPU → SNOMED dose-form concept ──────────
-- Populated by the loader's text-match pass: the dose-form fragment of a TMT GP/GPU
-- FSN is matched to a SNOMED dose-form concept's FSN (semantic_tag '(dose form)').
-- match_method records how (exact|normalized|fuzzy) and confidence gates FHIR use;
-- low-confidence rows are needs_review=1 and excluded from automated coding.
CREATE TABLE IF NOT EXISTS snomed_tmt_dose_link (
    tmt_id            VARCHAR(20) NOT NULL,
    snomed_concept_id VARCHAR(20) NOT NULL,
    match_method      VARCHAR(16) NOT NULL,   -- exact | normalized | fuzzy
    confidence        DECIMAL(4,3) NOT NULL DEFAULT 0.000,
    needs_review      TINYINT(1)  NOT NULL DEFAULT 0,
    source_version    VARCHAR(32) NOT NULL,
    tenant_id         VARCHAR(50) DEFAULT NULL,
    created_at        TIMESTAMP   DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tmt_id, source_version),
    KEY idx_snomed (snomed_concept_id),
    KEY idx_review (needs_review),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── audit trail (mirrors snomed_map_ingest_runs / tmt_ingest_runs) ───────────────
CREATE TABLE IF NOT EXISTS snomed_refset_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    refset_key        VARCHAR(16)  NOT NULL,   -- ips | gpfp | edqm | tmt_dose_link
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    -- Provenance URL — MLDS portal (https://mlds.ihtsdotools.org/) the package came from.
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_skipped      INT          NOT NULL DEFAULT 0,
    rows_review       INT          NOT NULL DEFAULT 0,
    status            VARCHAR(16)  NOT NULL DEFAULT 'running',
    notes             TEXT,
    started_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    finished_at       TIMESTAMP    NULL DEFAULT NULL,
    PRIMARY KEY (id),
    KEY idx_refset_key (refset_key),
    KEY idx_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
