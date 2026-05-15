-- Fix Syn OCR schema: add missing ocr_cloud_flash_enabled column
-- This column gates whether a tenant can escalate to Gemini Flash for OCR

ALTER TABLE tenant_configs ADD COLUMN IF NOT EXISTS ocr_cloud_flash_enabled BOOLEAN NOT NULL DEFAULT FALSE AFTER ocr_phi_strict;

-- Initialize asgard_medical with Flash disabled (use local models only)
UPDATE tenant_configs SET ocr_cloud_flash_enabled = FALSE WHERE tenant_id = 'asgard_medical';

-- Initialize asgard_insurance with Flash disabled (use local models only)
UPDATE tenant_configs SET ocr_cloud_flash_enabled = FALSE WHERE tenant_id = 'asgard_insurance';

-- If the column didn't exist, also add the Pro column if missing
ALTER TABLE tenant_configs ADD COLUMN IF NOT EXISTS ocr_cloud_pro_enabled BOOLEAN NOT NULL DEFAULT FALSE AFTER ocr_cloud_flash_enabled;
ALTER TABLE tenant_configs ADD COLUMN IF NOT EXISTS ocr_monthly_cloud_budget_usd DECIMAL(10, 2) DEFAULT 0.0 AFTER ocr_cloud_pro_enabled;

-- Verify the schema is now correct
SELECT column_name, column_type FROM information_schema.columns
WHERE table_schema = DATABASE() AND table_name = 'tenant_configs'
ORDER BY ordinal_position;
