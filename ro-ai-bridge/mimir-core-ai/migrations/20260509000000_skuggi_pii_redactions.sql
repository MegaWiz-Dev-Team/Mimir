-- 🌑 Skuggi PII Guardrail — extend pii_redactions for Heimdall (Sprint 50b).
--
-- The pii_redactions table already exists from Sprint 50 (Syn OCR layer,
-- surface='image'). Sprint 50b ADR-007 adds the Heimdall middleware layer
-- which audits *text* redactions on cloud-bound LLM calls. Same table,
-- discriminated by `surface` ('image' = Syn OCR, 'text' = Heimdall Skuggi).
--
-- This migration only ADDS columns the Heimdall path needs that aren't
-- already there. All ALTER are idempotent via IF NOT EXISTS guards.

CREATE TABLE IF NOT EXISTS pii_redactions (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    surface VARCHAR(30) DEFAULT 'image',
    total_count INT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_tenant (tenant_id)
);

ALTER TABLE pii_redactions
    ADD COLUMN IF NOT EXISTS request_id      VARCHAR(64)  NULL  COMMENT 'Heimdall request correlation id (X-Request-Id) — null for Syn OCR rows',
    ADD COLUMN IF NOT EXISTS provider        VARCHAR(40)  NULL  COMMENT 'gemini / openai / openrouter / local — null for image rows',
    ADD COLUMN IF NOT EXISTS model           VARCHAR(120) NULL  COMMENT 'Model id sent to provider — null for image rows',
    ADD COLUMN IF NOT EXISTS detections      JSON         NULL  COMMENT '[{"category":"thai_national_id","count":1}, …] — Heimdall Tier 1/Tier 2 categories',
    ADD COLUMN IF NOT EXISTS blocked         TINYINT(1)   NOT NULL DEFAULT 0 COMMENT '1 when mode=block-on-pii and total_count>0',
    ADD COLUMN IF NOT EXISTS payload_bytes   INT          NULL  COMMENT 'Original payload size (bytes)',
    ADD COLUMN IF NOT EXISTS redacted_bytes  INT          NULL  COMMENT 'Redacted payload size (bytes)',
    ADD COLUMN IF NOT EXISTS duration_us     INT          NULL  COMMENT 'Skuggi processing time microseconds';

-- New index to speed up "blocked calls in last 24h" compliance queries.
ALTER TABLE pii_redactions
    ADD INDEX IF NOT EXISTS idx_blocked_created (blocked, created_at);

-- Make `surface` more useful by recording 'text' for Heimdall rows; the
-- existing default 'image' stays for Syn OCR. No data migration — old
-- rows are correctly labelled 'image' by Sprint 50 inserts.

-- Ensure tenant_configs.pii_mode has a sensible default for new tenants.
-- Existing rows keep whatever they had; only the column default changes.
ALTER TABLE tenant_configs
    ADD COLUMN IF NOT EXISTS pii_mode VARCHAR(30) NOT NULL DEFAULT 'mask-and-send'
    COMMENT 'off | detect-only | mask-and-send | block-on-pii (Skuggi mode, ADR-007)';
