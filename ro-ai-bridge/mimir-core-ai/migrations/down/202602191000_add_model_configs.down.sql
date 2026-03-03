-- Rollback: 202602191000_add_model_configs.sql
-- Drops ai_models and removes model_id from ai_npc_persona

ALTER TABLE ai_npc_persona DROP FOREIGN KEY fk_persona_model;
ALTER TABLE ai_npc_persona DROP COLUMN model_id;
DROP TABLE IF EXISTS ai_models;
