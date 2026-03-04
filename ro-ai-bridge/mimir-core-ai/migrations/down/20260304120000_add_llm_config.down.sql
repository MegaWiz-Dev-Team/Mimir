-- Rollback: Remove llm_config column from tenant_configs
ALTER TABLE tenant_configs DROP COLUMN IF EXISTS llm_config;
