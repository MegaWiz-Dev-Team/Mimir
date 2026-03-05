-- Rollback: Agent Template Migration
DELETE FROM agent_configs WHERE name IN ('mimir', 'sage_ariel', 'fortune_teller', 'blacksmith') AND tenant_id = 'default';
ALTER TABLE agent_configs DROP COLUMN IF EXISTS tier;
ALTER TABLE agent_configs DROP COLUMN IF EXISTS response_mode;
