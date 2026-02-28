-- ============================================================================
-- Sprint 12 — Search Settings Persistence (Issue #138)
--
-- Adds search_settings JSON column to tenant_configs for per-tenant
-- search configuration (embedding_model, top_k, similarity_threshold,
-- search_mode).
-- ============================================================================

ALTER TABLE tenant_configs
ADD COLUMN IF NOT EXISTS search_settings JSON DEFAULT NULL;

-- Set default search settings for existing tenants
UPDATE tenant_configs
SET search_settings = JSON_OBJECT(
    'embedding_model', 'nomic-embed-text',
    'top_k', 5,
    'similarity_threshold', 0.7,
    'search_mode', 'hybrid'
)
WHERE search_settings IS NULL;
