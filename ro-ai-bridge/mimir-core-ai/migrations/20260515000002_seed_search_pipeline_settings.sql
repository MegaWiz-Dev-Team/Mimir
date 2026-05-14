-- Seed search and pipeline settings for all tenants
-- Provides sensible defaults for RAG retrieval and data pipeline configuration

ALTER TABLE tenant_configs ADD COLUMN IF NOT EXISTS search_settings JSON;
ALTER TABLE tenant_configs ADD COLUMN IF NOT EXISTS pipeline_settings JSON;

-- Update default_tenant with search settings
UPDATE tenant_configs
SET search_settings = JSON_OBJECT(
    'embedding_model', 'bge-m3',
    'top_k', 5,
    'similarity_threshold', 0.7,
    'search_mode', 'hybrid',
    'use_reranking', FALSE,
    'rerank_model', 'gemini-3-flash'
)
WHERE tenant_id = 'default_tenant' AND (search_settings IS NULL OR search_settings = '{}');

-- Update default_tenant with pipeline settings
UPDATE tenant_configs
SET pipeline_settings = JSON_OBJECT(
    'chunk_strategy', 'auto',
    'chunk_size', 512,
    'chunk_overlap', 50,
    'dedup_threshold', 0.95,
    'enable_entity_extraction', TRUE,
    'enable_markdown_metrics', TRUE,
    'quality_control_enabled', TRUE
)
WHERE tenant_id = 'default_tenant' AND (pipeline_settings IS NULL OR pipeline_settings = '{}');
