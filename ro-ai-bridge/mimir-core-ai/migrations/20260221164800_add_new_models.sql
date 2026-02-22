-- Add the new newly downloaded models and update the existing llama3.2 to correctly map to llama3.2 instead of llama3.2:3b

-- Update existing llama3.2:3b to standard llama3.2
UPDATE ai_models SET model_id = 'llama3.2' WHERE model_id = 'llama3.2:3b';

-- Insert new models
INSERT INTO ai_models (model_id, provider, model_type, capabilities) VALUES 
('deepseek-r1:1.5b', 'ollama', 'llm', '{"tools":true, "reasoning":true}'),
('gemma:2b', 'ollama', 'llm', '{"tools":false, "reasoning":false}'),
('phi3.5', 'ollama', 'llm', '{"tools":false, "reasoning":false}')
ON DUPLICATE KEY UPDATE capabilities = VALUES(capabilities);
