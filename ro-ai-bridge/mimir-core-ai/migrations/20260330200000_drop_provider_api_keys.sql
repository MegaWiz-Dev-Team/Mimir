-- Remove the provider_api_keys column from tenant_configs.
-- LLM provider API keys are now managed via HashiCorp Vault (tenant-specific paths)
-- and are loaded at runtime via vault.rs get_tenant_secrets().
-- This column was already excluded from SELECT queries in iam.rs and had NULL data only.
ALTER TABLE tenant_configs DROP COLUMN IF EXISTS provider_api_keys;
