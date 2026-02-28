-- Rollback: 20260221164800_add_new_models.sql
-- Reverse model ID renaming and remove added models
-- NOTE: Data migration — best-effort rollback

DELETE FROM ai_models WHERE model_id IN ('nomic-embed-text', 'qwen2.5:14b', 'llama3.2:1b');
UPDATE ai_models SET model_id = 'llama3.2:3b' WHERE model_id = 'llama3.2';
