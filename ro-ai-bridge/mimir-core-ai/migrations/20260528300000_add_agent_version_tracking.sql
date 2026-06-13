-- ============================================================================
-- Feature: Agent Version Tracking for Fine-Tuning
--
-- Date: 2026-05-28
-- Purpose: Track agent model versions to support fine-tuning iterations
--          and enable rollback to previous agent configurations
--
-- Schema Changes:
--   - agent_version: SemVer format (e.g., "1.0.0", "1.1.0-beta")
--   - version_updated_at: timestamp of last version change
-- ============================================================================

ALTER TABLE agent_configs
ADD COLUMN IF NOT EXISTS agent_version VARCHAR(20) NOT NULL DEFAULT '1.0.0'
  COMMENT 'SemVer: base-model.fine-tune-iteration.patch (e.g., 1.0.0 = gemma-4-26b baseline)',
ADD COLUMN IF NOT EXISTS version_updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
  ON UPDATE CURRENT_TIMESTAMP
  COMMENT 'Timestamp of last version change (fine-tune, rollback, etc)';

-- Add index for version tracking queries
CREATE INDEX IF NOT EXISTS idx_agent_version
ON agent_configs(tenant_id, agent_version);

-- Verify schema
SELECT COLUMN_NAME, DATA_TYPE, COLUMN_COMMENT
FROM INFORMATION_SCHEMA.COLUMNS
WHERE TABLE_NAME='agent_configs'
AND COLUMN_NAME IN ('agent_version', 'version_updated_at')
ORDER BY ORDINAL_POSITION;
