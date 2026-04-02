INSERT INTO ai_models (model_id, provider, model_type, is_active, capabilities)
VALUES ('paripolt/Qwen3.5-27B-Opus-Reasoning-MLX-4bit', 'heimdall', 'llm', true, '{"reasoning":true,"tools":true,"vision":false}')
ON DUPLICATE KEY UPDATE is_active = true;
