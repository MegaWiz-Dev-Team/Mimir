-- Seed additional AI Models (OpenAI and Heimdall)
INSERT IGNORE INTO ai_models (model_id, provider, model_type, capabilities) VALUES 
('gpt-4o', 'openai', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gpt-4o-mini', 'openai', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('o1', 'openai', 'llm', '{"tools":false, "vision":false, "reasoning":true}'),
('o3-mini', 'openai', 'llm', '{"tools":true, "vision":false, "reasoning":true}'),
('mlx-community/Qwen3.5-35B-A3B-4bit', 'heimdall', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('mlx-community/Meta-Llama-3-8B-Instruct-4bit', 'heimdall', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('llama3.2', 'ollama', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('llama3.1', 'ollama', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('mistral', 'ollama', 'llm', '{"tools":false, "vision":false, "reasoning":false}'),
('deepseek-r1:14b', 'ollama', 'llm', '{"tools":false, "vision":false, "reasoning":true}'),
('deepseek-r1:32b', 'ollama', 'llm', '{"tools":false, "vision":false, "reasoning":true}'),
('qwen2.5:14b', 'ollama', 'llm', '{"tools":true, "vision":false, "reasoning":false}');
