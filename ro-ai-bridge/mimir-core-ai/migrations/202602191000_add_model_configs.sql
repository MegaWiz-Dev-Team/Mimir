-- Migration: Create AI Models Registry and Update Persona
-- Author: Antigravity
-- Date: 2026-02-19

-- 1. Create AI Models Registry Table
CREATE TABLE IF NOT EXISTS ai_models (
    model_id VARCHAR(100) PRIMARY KEY COMMENT 'Unique model identifier e.g. qwen2.5:32b',
    provider VARCHAR(50) NOT NULL COMMENT 'ollama, google, openai, azure',
    model_type VARCHAR(30) NOT NULL DEFAULT 'llm' COMMENT 'llm, embedding, reranker',
    is_active BOOLEAN DEFAULT TRUE,
    capabilities JSON COMMENT '{"tools":true, "vision":false, "reasoning":true}',
    metadata JSON COMMENT '{"vram_required_gb":22, "context_window":128000}',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_provider (provider),
    INDEX idx_type (model_type),
    INDEX idx_active (is_active)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

-- 2. Add model_id to ai_npc_persona
ALTER TABLE ai_npc_persona 
ADD COLUMN model_id VARCHAR(100) AFTER tier,
ADD CONSTRAINT fk_persona_model FOREIGN KEY (model_id) REFERENCES ai_models(model_id) ON DELETE SET NULL;

-- 3. Seed Initial Data
INSERT INTO ai_models (model_id, provider, model_type, capabilities) VALUES 
('qwen2.5:32b', 'ollama', 'llm', '{"tools":true, "reasoning":true}'),
('llama3.2:1b', 'ollama', 'llm', '{"tools":false, "reasoning":false}'),
('llama3.2:3b', 'ollama', 'llm', '{"tools":true, "reasoning":false}'),
('bge-m3', 'ollama', 'embedding', '{"multilingual":true}'),
('gemini-2.5-flash-lite', 'google', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('gemini-2.5-flash', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-2.5-pro', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}');
