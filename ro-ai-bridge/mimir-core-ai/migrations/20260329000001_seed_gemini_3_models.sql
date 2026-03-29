-- Seed additional Gemini 3 AI Models
INSERT IGNORE INTO ai_models (model_id, provider, model_type, capabilities) VALUES 
('gemini-3.1-pro-preview', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3.1-pro-preview-customtools', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3-flash-preview', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3.1-flash-lite-preview', 'google', 'llm', '{"tools":true, "vision":false, "reasoning":false}'),
('gemini-3-pro-image-preview', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3.1-flash-image-preview', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3.1-flash-live-preview', 'google', 'llm', '{"tools":true, "vision":true, "reasoning":true}');
