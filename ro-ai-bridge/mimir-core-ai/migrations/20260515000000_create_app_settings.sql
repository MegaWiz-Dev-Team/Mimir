-- Create app_settings table for global configuration
-- Used for system-wide settings like auto_tune_model, judge_model, etc.

CREATE TABLE IF NOT EXISTS app_settings (
    setting_key VARCHAR(100) PRIMARY KEY,
    setting_value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- Seed default global settings
INSERT IGNORE INTO app_settings (setting_key, setting_value, description) VALUES
('auto_tune_model', 'gemini-3-flash', 'LLM model used for prompt/parameter optimization'),
('judge_model', 'gemini-3-flash', 'LLM model used for evaluation scoring (LLM-as-judge)'),
('default_embedding_model', 'bge-m3', 'Default embedding model for new tenants'),
('max_rag_tokens', '2000', 'Maximum tokens for RAG context window'),
('chat_temperature', '0.7', 'Default temperature for chat completions'),
('rag_temperature', '0.5', 'Default temperature for RAG retrievals (lower = more deterministic)');
