-- Add avatar_url to ai_npc_persona
ALTER TABLE ai_npc_persona ADD COLUMN avatar_url VARCHAR(255) NULL AFTER display_name;
