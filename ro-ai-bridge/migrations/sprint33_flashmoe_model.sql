-- Add Flash-MoE Model to the Database
INSERT INTO ai_models (model_id, provider, model_type, is_active, capabilities, metadata) 
VALUES (
    'Qwen3.5-397B-MoE', 
    'flashmoe', 
    'llm', 
    true, 
    '{"chat": true, "rag": true}', 
    '{"description": "Ultra-large 397B MoE running directly from SSD via Flash-MoE engine. Speed ~1.5 - 2.0 TPS"}'
) 
ON DUPLICATE KEY UPDATE is_active = true;
