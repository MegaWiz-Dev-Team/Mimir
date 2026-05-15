-- Complete Syn OCR Database Schema Migration
-- Run this to set up all required tables and columns for Syn API v0.2.0

USE mimir;

-- ═════════════════════════════════════════════════════════════════════════════
-- 1. Update tenant_configs with OCR columns
-- ═════════════════════════════════════════════════════════════════════════════

ALTER TABLE tenant_configs
ADD COLUMN IF NOT EXISTS ocr_phi_strict BOOLEAN NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS ocr_cloud_flash_enabled BOOLEAN NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS ocr_cloud_pro_enabled BOOLEAN NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS ocr_monthly_cloud_budget_usd DECIMAL(10, 2) NOT NULL DEFAULT 0.00,
ADD COLUMN IF NOT EXISTS pii_custom_patterns LONGTEXT;

-- ═════════════════════════════════════════════════════════════════════════════
-- 2. Create ocr_documents table (audit log)
-- ═════════════════════════════════════════════════════════════════════════════

DROP TABLE IF EXISTS ocr_documents;

CREATE TABLE ocr_documents (
  id VARCHAR(36) PRIMARY KEY COMMENT 'audit_id',
  tenant_id VARCHAR(50) NOT NULL,
  filename VARCHAR(255),
  extracted_text LONGTEXT,
  engine_used VARCHAR(50),
  router_reason VARCHAR(100),
  status VARCHAR(50),
  error_message LONGTEXT,
  image_sha256 VARCHAR(64),
  original_image LONGBLOB,
  confidence FLOAT,
  bbox_count INT,
  cost_usd DECIMAL(10, 4) DEFAULT 0,
  latency_ms INT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
  INDEX idx_tenant (tenant_id),
  INDEX idx_created (created_at),
  INDEX idx_engine (engine_used)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ═════════════════════════════════════════════════════════════════════════════
-- 3. Initialize asgard_medical and asgard_insurance tenants if needed
-- ═════════════════════════════════════════════════════════════════════════════

INSERT INTO tenants (id, name, domain, created_at, updated_at)
SELECT 'asgard_medical', 'Asgard Medical', 'medical.asgard.local', NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM tenants WHERE id = 'asgard_medical');

INSERT INTO tenants (id, name, domain, created_at, updated_at)
SELECT 'asgard_insurance', 'Asgard Insurance', 'insurance.asgard.local', NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM tenants WHERE id = 'asgard_insurance');

-- ═════════════════════════════════════════════════════════════════════════════
-- 4. Create tenant configs for OCR tenants (local models only)
-- ═════════════════════════════════════════════════════════════════════════════

INSERT INTO tenant_configs (
  tenant_id,
  ocr_phi_strict,
  ocr_cloud_flash_enabled,
  ocr_cloud_pro_enabled,
  ocr_monthly_cloud_budget_usd,
  pii_mode,
  created_at,
  updated_at
)
SELECT 'asgard_medical', 1, 0, 0, 0.00, 'block-on-pii', NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM tenant_configs WHERE tenant_id = 'asgard_medical')
UNION ALL
SELECT 'asgard_insurance', 1, 0, 0, 0.00, 'block-on-pii', NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM tenant_configs WHERE tenant_id = 'asgard_insurance');

-- ═════════════════════════════════════════════════════════════════════════════
-- 5. Verify schema
-- ═════════════════════════════════════════════════════════════════════════════

SELECT '✅ Schema setup complete' as status;
SELECT COUNT(*) as ocr_documents_count FROM ocr_documents;
SELECT tenant_id, ocr_phi_strict, ocr_cloud_flash_enabled, pii_mode
FROM tenant_configs
WHERE tenant_id LIKE 'asgard%'
ORDER BY tenant_id;
