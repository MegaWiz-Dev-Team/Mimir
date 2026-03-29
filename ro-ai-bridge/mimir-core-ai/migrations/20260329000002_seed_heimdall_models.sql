-- Seed Heimdall Self-hosted Models (Medical Catalog)
INSERT IGNORE INTO ai_models (model_id, provider, model_type, capabilities) VALUES 
('lmstudio-community/medgemma-4b-it-MLX-4bit', 'heimdall', 'llm', '{"tools":false, "vision":false, "reasoning":false}'),
('mlx-community/Qwen3.5-35B-A3B-4bit', 'heimdall', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('mlx-community/Qwen3.5-27B-4bit', 'heimdall', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('mlx-community/Qwen3.5-9B-MLX-4bit', 'heimdall', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('mlx-community/Meta-Llama-3-8B-Instruct-4bit', 'heimdall', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('nomic-embed-text', 'ollama', 'embedding', '{"tools":false, "vision":false, "reasoning":false}');
