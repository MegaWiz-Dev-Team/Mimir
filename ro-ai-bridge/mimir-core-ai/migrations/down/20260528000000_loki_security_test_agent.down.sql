-- ============================================================================
-- Rollback: Loki Security Test Agent
-- ============================================================================

DELETE FROM agent_mcp_servers
WHERE agent_id IN (
  SELECT id FROM agent_configs
  WHERE tenant_id = 'asgard_platform' AND name = 'loki-security-test'
);

DELETE FROM agent_configs
WHERE tenant_id = 'asgard_platform' AND name = 'loki-security-test';
