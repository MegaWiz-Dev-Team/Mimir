-- Align pii_redactions with the Sprint 50 / Skuggi full schema.
--
-- The earlier sqlx migration `20260509000000_skuggi_pii_redactions.sql`
-- created a *partial* `pii_redactions` table on the assumption that the
-- manual ops migration `ro-ai-bridge/migrations/sprint50_syn_skuggi_foundation.sql`
-- had run first to lay down the full Sprint 50 shape. In the asgard-infra
-- mimir deployment that manual SQL was never applied, so the table is
-- missing the columns both the Heimdall writer and the mimir-api reader
-- require:
--
--   writer  Heimdall/gateway/src/tenant_config.rs::insert_audit
--   reader  Mimir/ro-ai-bridge/src/routes/admin_skuggi.rs::list_redactions
--
-- Both expect: id VARCHAR(36), pii_mode_used, detection_tier, decision,
-- pii_total_count, latency_ms, text_pii_count, image_face_count,
-- image_text_box_count, pii_types_found, ocr_document_id,
-- image_sha256_pre/_post.
--
-- The existing table has 0 rows (writer fails silently because the column
-- list and id type don't match), so DROP + CREATE is safe and produces a
-- clean schema. If this migration is ever re-run against an environment
-- where the full Sprint 50 schema is already present, the IF EXISTS guard
-- + identical CREATE is idempotent in shape.

DROP TABLE IF EXISTS pii_redactions;

CREATE TABLE pii_redactions (
    id                  VARCHAR(36)   NOT NULL,
    tenant_id           VARCHAR(50)   NOT NULL,
    ocr_document_id     VARCHAR(36)   DEFAULT NULL,

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

    pii_types_found     LONGTEXT      DEFAULT NULL,

    request_id          VARCHAR(64)   DEFAULT NULL
        COMMENT 'Heimdall request correlation id (X-Request-Id) — null for Syn OCR rows',
    provider            VARCHAR(40)   DEFAULT NULL
        COMMENT 'gemini / openai / openrouter / local — null for image rows',
    model               VARCHAR(120)  DEFAULT NULL
        COMMENT 'Model id sent to provider — null for image rows',
    detections          LONGTEXT      DEFAULT NULL
        COMMENT 'JSON array of {category,count} from Heimdall Tier 1/Tier 2',

    pii_mode_used       VARCHAR(30)   NOT NULL
        COMMENT 'off | detect-only | mask-and-send | block-on-pii',
    surface             VARCHAR(20)   NOT NULL DEFAULT 'image'
        COMMENT 'image (Syn OCR) | text (Heimdall Skuggi) | both',
    detection_tier      VARCHAR(20)   DEFAULT NULL
        COMMENT 'tier1 | tier1+tier2 | tier2',
    decision            VARCHAR(20)   NOT NULL
        COMMENT 'redacted | blocked | passed | passed_through | error',

    pii_total_count     INT           DEFAULT 0,
    blocked             TINYINT(1)    NOT NULL DEFAULT 0
        COMMENT '1 when mode=block-on-pii and pii_total_count>0',
    payload_bytes       INT           DEFAULT NULL,
    redacted_bytes      INT           DEFAULT NULL,
    duration_us         INT           DEFAULT NULL,
    latency_ms          INT           DEFAULT NULL,

    created_at          TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id),
    KEY idx_tenant_created (tenant_id, created_at DESC),
    KEY idx_ocr_doc (ocr_document_id),
    KEY idx_decision (decision),
    KEY idx_blocked_created (blocked, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;