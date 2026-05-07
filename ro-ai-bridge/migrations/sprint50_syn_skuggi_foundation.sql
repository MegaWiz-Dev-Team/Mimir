-- Sprint 50 + 50b — Syn S1 OCR Foundation + Skuggi PII Guardrail
--
-- Day-1 schema:
--   1. ocr_documents (B-50e)         — audit row per OCR call (any engine)
--   2. pii_redactions (B-50b-1)      — audit row per Skuggi redaction
--   3. tenant_configs extension      — cloud opt-in flags + PII mode
--                                      + monthly cloud budget cap
--                                      + tenant-scoped PII regex extensions
--
-- Subsequent migrations add:
--   sprint50_engine_health           when engine sidecars deploy (B-50a/k)
--   sprint50_smart_router_metrics    when router goes live (B-50b)
--
-- See:
--   Mimir/docs/03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md (Sprint 50 + 50b)
--   Asgard/docs/architecture/ADR-006-Syn-OCR-Stack.md (4-tier OCR)
--   Asgard/docs/architecture/ADR-007-Skuggi-PII-Guardrail.md (Skuggi modes)

-- ─────────────────────────────────────────────────────────────────────
-- 1. ocr_documents — audit per OCR extraction
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE ocr_documents (
    id                VARCHAR(36)   NOT NULL,
    tenant_id         VARCHAR(50)   NOT NULL,
    -- Image fingerprint (SHA-256). Allows repeat-call detection +
    -- audit replay without storing the image bytes here.
    image_sha256      CHAR(64)      NOT NULL,
    image_path        TEXT          DEFAULT NULL
        COMMENT 'Vault path or S3 URI; NULL if image not retained',
    -- Engine that processed this call. Matches ADR-006 4-tier:
    --   chandra-local | paddleocr-local | gemini-3-flash | gemini-3.1-pro
    -- + smart-router meta when the router auto-picked.
    engine_used       VARCHAR(40)   NOT NULL,
    engine_version    VARCHAR(40)   DEFAULT NULL,
    -- Why this engine was picked — useful for router audit + iteration.
    -- e.g. "manual_override", "doc_type=handwriting", "confidence_below_0.7"
    router_reason     VARCHAR(100)  DEFAULT NULL,

    -- Outputs
    extracted_text    LONGTEXT      DEFAULT NULL
        COMMENT 'Full text from OCR. NULL on engine failure.',
    confidence        DECIMAL(4,3)  DEFAULT NULL
        COMMENT 'Engine self-reported confidence 0.0-1.0',
    bbox_count        INT           DEFAULT NULL
        COMMENT 'Number of text bounding boxes returned',
    -- Cost in USD for this call. 0 for local; non-zero only for cloud.
    cost_usd          DECIMAL(8,5)  NOT NULL DEFAULT 0.00000,
    latency_ms        INT           DEFAULT NULL,

    -- Skuggi cross-link: if PII redaction occurred BEFORE engine call,
    -- pii_redactions.id will reference this row by image_sha256_pre.
    pii_redacted      BOOLEAN       NOT NULL DEFAULT FALSE,

    -- Status: succeeded | engine_failed | pii_blocked | budget_exceeded |
    --         pii_strict_block (pii_mode=block-on-pii hit)
    status            VARCHAR(30)   NOT NULL DEFAULT 'succeeded',
    status_message    TEXT          DEFAULT NULL,

    created_at        TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    requested_by      VARCHAR(100)  DEFAULT NULL
        COMMENT 'JWT subject (user id) of the requester',
    PRIMARY KEY (id),
    KEY idx_tenant_created (tenant_id, created_at DESC),
    KEY idx_image_sha256 (image_sha256),
    KEY idx_engine (engine_used),
    KEY idx_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────
-- 2. pii_redactions — Skuggi audit per redaction event
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE pii_redactions (
    id                  VARCHAR(36)   NOT NULL,
    tenant_id           VARCHAR(50)   NOT NULL,
    -- Optional FK to ocr_documents.id when the redaction was for an OCR
    -- call. NULL when redaction was for non-OCR LLM call (text chat,
    -- voice STT future Sprint 52, etc.).
    ocr_document_id     VARCHAR(36)   DEFAULT NULL,

    -- What got redacted.
    image_sha256_pre    CHAR(64)      DEFAULT NULL
        COMMENT 'Original image hash (NULL if text-only redaction)',
    image_sha256_post   CHAR(64)      DEFAULT NULL
        COMMENT 'Post-blur image hash (NULL if text-only)',
    text_pii_count      INT           DEFAULT 0
        COMMENT 'Number of text PII spans redacted',
    image_face_count    INT           DEFAULT 0
        COMMENT 'Number of faces blurred',
    image_text_box_count INT          DEFAULT 0
        COMMENT 'Number of Thai-ID/MRN regions blurred',

    -- JSON array of PII type tags found, e.g.
    --   ["thai_national_id", "phone", "person_name", "face", "mrn"]
    pii_types_found     LONGTEXT      DEFAULT NULL,

    -- Mode this run used (off / detect-only / mask-and-send / block-on-pii)
    pii_mode_used       VARCHAR(30)   NOT NULL,
    -- Where the redaction happened: image (Sprint 50 OCR) | text | both
    surface             VARCHAR(20)   NOT NULL DEFAULT 'image',
    -- Tier 1 (Rust regex) or Tier 2 (PyThaiNLP) handled it
    detection_tier      VARCHAR(20)   DEFAULT NULL,

    -- Decision: redacted | blocked (pii_mode=block-on-pii) | passed_through
    --           (no PII found) | error
    decision            VARCHAR(20)   NOT NULL,

    latency_ms          INT           DEFAULT NULL,
    created_at          TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY idx_tenant_created (tenant_id, created_at DESC),
    KEY idx_ocr_doc (ocr_document_id),
    KEY idx_decision (decision)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────
-- 3. tenant_configs extension — Sprint 50 cloud opt-in + Skuggi PII mode
-- ─────────────────────────────────────────────────────────────────────
ALTER TABLE tenant_configs
    -- Sprint 50 cloud OCR opt-in (default OFF — strict opt-in per ADR-006)
    ADD COLUMN ocr_cloud_flash_enabled    BOOLEAN     NOT NULL DEFAULT FALSE
        COMMENT 'Enable Tier 2 Gemini 3 Flash for cloud OCR fallback',
    ADD COLUMN ocr_cloud_pro_enabled      BOOLEAN     NOT NULL DEFAULT FALSE
        COMMENT 'Enable Tier 3 Gemini 3.1 Pro for high-stakes cloud OCR (requires Flash also enabled)',
    ADD COLUMN ocr_phi_strict             BOOLEAN     NOT NULL DEFAULT TRUE
        COMMENT 'Hard block: never send to cloud regardless of opt-in',
    ADD COLUMN ocr_monthly_cloud_budget_usd DECIMAL(10,2) NOT NULL DEFAULT 0.00
        COMMENT 'Hard cap; 0 = no budget set; cloud calls reject when exceeded',

    -- Skuggi (Sprint 50b) — PII guardrail mode + custom regex extensions
    ADD COLUMN pii_mode                   VARCHAR(30) NOT NULL DEFAULT 'mask-and-send'
        COMMENT 'off | detect-only | mask-and-send (default) | block-on-pii — see ADR-007',
    ADD COLUMN pii_custom_patterns        LONGTEXT    DEFAULT NULL
        COMMENT 'JSON array of tenant-specific regex extensions (e.g. hospital-specific MRN format)';
