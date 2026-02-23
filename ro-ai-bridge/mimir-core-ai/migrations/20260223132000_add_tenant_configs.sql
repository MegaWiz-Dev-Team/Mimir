-- Create tenant_configs table
CREATE TABLE IF NOT EXISTS tenant_configs (
    tenant_id VARCHAR(50) PRIMARY KEY,
    
    -- LLM & Provider Settings
    default_provider VARCHAR(50) DEFAULT 'ollama',
    default_model VARCHAR(50) DEFAULT 'llama3.2',
    provider_api_keys JSON, -- JSON dictionary of provider api keys
    
    -- Pipeline & QA Rules
    qa_rules JSON,          
    
    -- RAG Setup
    system_prompt TEXT,     
    
    -- Flags & Limits
    max_daily_tokens BIGINT DEFAULT 100000,
    is_dedicated_vector_db BOOLEAN DEFAULT false,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Initialize default tenant config
INSERT IGNORE INTO tenant_configs (tenant_id) VALUES ('default_tenant');
