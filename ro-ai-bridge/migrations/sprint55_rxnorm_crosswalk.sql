-- Sprint 55 — RxNorm crosswalk: brand/generic → ingredient → PrimeKG (via UNII)
--
-- Graduates the drug resolver from the v1 static seed-map (57 hand-built brands in
-- rxnorm_brand_ingredient.tsv, see ro-ai-domain-medical/docs/NORMALIZER.md) to a full
-- RxNorm ingest, and gives "drug" the normalization layer that "disease" already has
-- (SNOMED→MONDO→PrimeKG). RxNorm is US-gov **public domain** → ships in commercial
-- Asgard, unlike DrugBank. This closes NORMALIZER.md follow-ups §4 (RxNorm→DrugBank
-- crosswalk), §5 (full RRF table), §8 (Thai/TMT lane converges here too).
--
-- Source: RxNorm Full monthly release (RxNorm_full_YYYYMMDD.zip, free UMLS account) —
-- rrf/{RXNCONSO,RXNREL,RXNSAT}.RRF. Ingest filters to LAT='ENG'; keep SAB='RXNORM'
-- atoms as the license-clean core (other source atoms carry per-source flags).
--
-- Tenant model (mirrors icd10_codes/loinc_codes/tmt_codes):
--   tenant_id = NULL   → shared master (every tenant)
--   tenant_id = <slug> → per-tenant override (rare)
--
-- SHARED-KNOWLEDGE RULE: this KB ships WITH a catalog row in
-- GET /api/v1/knowledge/shared (ro-ai-bridge/src/routes/shared_knowledge.rs) in the
-- SAME PR — never a silently-invisible master table.

-- ── 1. Atoms (RXNCONSO) — every name string for a concept, one row per RXAUI ────────
CREATE TABLE rxnorm_atoms (
    rxaui             BIGINT       NOT NULL,          -- atom id (RRF unique) → natural PK
    rxcui             BIGINT       NOT NULL,          -- concept id (many atoms per concept)
    -- Term type: IN (ingredient), PIN (precise ingredient), MIN (multi-ingredient),
    -- BN (brand name), SBD/SCD (branded/clinical drug), SY (synonym incl. INN).
    tty               VARCHAR(12)  NOT NULL,
    sab               VARCHAR(20)  NOT NULL,          -- source; SAB='RXNORM' = clean core
    str               TEXT         NOT NULL,          -- the name string
    -- lower(trim(str)) truncated to 255 for exact/prefix index matching.
    str_norm          VARCHAR(255) NOT NULL,
    suppress          CHAR(1)      NOT NULL DEFAULT 'N',
    source_version    VARCHAR(32)  NOT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (rxaui, source_version),
    KEY idx_rxcui (rxcui),
    KEY idx_tty (tty),
    KEY idx_str_norm (str_norm),                      -- exact/prefix resolution
    KEY idx_sab (sab),
    KEY idx_tenant (tenant_id),
    FULLTEXT KEY ft_str (str)                          -- fuzzy / multi-word fallback
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── 2. Relations (RXNREL) — brand/drug → ingredient closure ─────────────────────────
-- Keep only the ingredient-bearing RELA we traverse: has_ingredient, ingredient_of,
-- tradename_of, has_tradename, consists_of, constitutes, form_of, has_form.
CREATE TABLE rxnorm_rel (
    rxcui1            BIGINT       NOT NULL,
    rela              VARCHAR(32)  NOT NULL,          -- relationship attribute (directional)
    rxcui2            BIGINT       NOT NULL,
    sab               VARCHAR(20)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (rxcui1, rela, rxcui2, source_version),
    KEY idx_from (rxcui1, rela),
    KEY idx_to (rxcui2, rela),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── 3. UNII (RXNSAT ATN='UNII') — chemical-identity bridge to DrugBank/PrimeKG ──────
-- FDA UNII is public domain and language-neutral: the same substance has one UNII
-- across RxNorm, DrugBank, TMT SUBS, INN. This is the join key for the PrimeKG bridge.
CREATE TABLE rxnorm_unii (
    rxcui             BIGINT       NOT NULL,          -- an ingredient concept (TTY IN/PIN)
    unii              VARCHAR(10)  NOT NULL,          -- FDA UNII (10-char)
    source_version    VARCHAR(32)  NOT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (rxcui, unii, source_version),
    KEY idx_unii (unii),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── 4. RxNorm-ingredient → PrimeKG node crosswalk (built by rxnorm_primekg_bridge.py)
-- The resolution target: given a canonical ingredient, the PrimeKG DrugBank node that
-- carries the graph edges (interactions/indications). match_method records HOW it was
-- bridged so provenance + confidence are auditable (never a silent fuzzy match):
--   'name'    — RxNorm IN str == PrimeKG node name (exact, normalized)
--   'inn_syn' — matched via a RxNorm SY (INN) atom (albuterol→salbutamol)
--   'unii'    — matched via UNII == DrugBank UNII (name diverged; chemical identity)
CREATE TABLE rxnorm_primekg_map (
    rxcui             BIGINT       NOT NULL,          -- RxNorm ingredient (IN/PIN)
    unii              VARCHAR(10)  DEFAULT NULL,
    primekg_entity_id VARCHAR(32)  NOT NULL,          -- DrugBank id, e.g. DB00682
    primekg_index     BIGINT       DEFAULT NULL,      -- entity_index (graph handle) if resolved
    primekg_name      TEXT         NOT NULL,
    match_method      VARCHAR(12)  NOT NULL,          -- name | inn_syn | unii
    confidence        DECIMAL(3,2) NOT NULL DEFAULT 1.00,
    source_version    VARCHAR(32)  NOT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (rxcui, primekg_entity_id, source_version),
    KEY idx_entity (primekg_entity_id),
    KEY idx_unii (unii),
    KEY idx_method (match_method),
    KEY idx_tenant (tenant_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── 5. Ingest provenance (mirrors tmt_ingest_runs) ──────────────────────────────────
CREATE TABLE rxnorm_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_atoms        INT          NOT NULL DEFAULT 0,
    rows_rel          INT          NOT NULL DEFAULT 0,
    rows_unii         INT          NOT NULL DEFAULT 0,
    rows_bridge       INT          NOT NULL DEFAULT 0,
    bridge_coverage   DECIMAL(5,2) DEFAULT NULL,      -- % PrimeKG drug nodes bridged
    status            VARCHAR(20)  NOT NULL DEFAULT 'RUNNING',
    notes             TEXT         DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_source_version (source_version),
    KEY idx_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
