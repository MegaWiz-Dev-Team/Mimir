-- ============================================================================
-- Issue #185 — Centralized LLM/Embedding Model Configuration
--
-- Adds llm_config JSON column to tenant_configs for per-purpose
-- LLM model slots (chat, rag, pipeline_generator, pipeline_evaluator,
-- judge, embedding) and Heimdall gateway connection settings.
-- ============================================================================

ALTER TABLE tenant_configs
ADD COLUMN IF NOT EXISTS llm_config JSON DEFAULT NULL;

-- Set default LLM config for existing tenants
UPDATE tenant_configs
SET llm_config = JSON_OBJECT(
    'chat', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'rag', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'pipeline_generator', JSON_OBJECT('provider', 'gemini', 'model', 'gemini-2.5-flash'),
    'pipeline_evaluator', JSON_OBJECT('provider', 'gemini', 'model', 'gemini-2.5-flash'),
    'judge', JSON_OBJECT('provider', 'gemini', 'model', 'gemini-2.5-flash'),
    'embedding', JSON_OBJECT('provider', 'ollama', 'model', 'nomic-embed-text'),
    'heimdall_url', '',
    'heimdall_api_key', ''
)
WHERE llm_config IS NULL;
