-- ============================================================================
-- Sprint 14 — Pipeline Settings Column (Issue: Settings Pipeline Fixes)
--
-- Adds pipeline_settings JSON column to tenant_configs for per-tenant
-- pipeline configuration (chunk_strategy, chunk_size, chunk_overlap,
-- dedup_threshold).
--
-- Migrates any existing chunk settings from search_settings into the
-- new column, then cleans them from search_settings.
-- ============================================================================

ALTER TABLE tenant_configs
ADD COLUMN IF NOT EXISTS pipeline_settings JSON DEFAULT NULL;

-- Migrate existing chunk settings from search_settings → pipeline_settings
UPDATE tenant_configs
SET pipeline_settings = JSON_OBJECT(
    'chunk_strategy', COALESCE(JSON_UNQUOTE(JSON_EXTRACT(search_settings, '$.chunk_strategy')), 'auto'),
    'chunk_size', COALESCE(JSON_EXTRACT(search_settings, '$.chunk_size'), 512),
    'chunk_overlap', COALESCE(JSON_EXTRACT(search_settings, '$.chunk_overlap'), 50),
    'dedup_threshold', COALESCE(JSON_EXTRACT(search_settings, '$.dedup_threshold'), 0)
)
WHERE pipeline_settings IS NULL;

-- Clean migrated keys from search_settings (leave only search-specific fields)
UPDATE tenant_configs
SET search_settings = JSON_REMOVE(
    COALESCE(search_settings, '{}'),
    '$.chunk_strategy', '$.chunk_size', '$.chunk_overlap', '$.dedup_threshold'
)
WHERE JSON_EXTRACT(search_settings, '$.chunk_strategy') IS NOT NULL;
