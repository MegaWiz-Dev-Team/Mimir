-- Seed LLM configuration slots for all tenants
-- Each tenant gets a default configuration with sensible defaults

ALTER TABLE tenant_configs ADD COLUMN IF NOT EXISTS llm_config JSON;

-- Update default_tenant with comprehensive LLM config
UPDATE tenant_configs
SET llm_config = JSON_OBJECT(
    'chat', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'rag', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'pipeline_generator', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'pipeline_extractor', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'pipeline_evaluator', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
    'judge', JSON_OBJECT('provider', 'gemini', 'model', 'gemini-3-flash'),
    'embedding', JSON_OBJECT('provider', 'heimdall', 'model', 'bge-m3'),
    'heimdall_url', 'http://localhost:30081'
)
WHERE tenant_id = 'default_tenant' AND llm_config IS NULL;
