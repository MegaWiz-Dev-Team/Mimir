-- Wire the analyst-* agents to the analytics MCP server (ADR-024 P2).
-- Bifrost reads agent_configs.mcp_servers (JSON array of hermodr targets),
-- resolves each `hermodr-<suffix>` → http://hermodr-<suffix>.asgard.svc:8090/rpc,
-- calls tools/list, and registers the tools for the agent. So pointing the
-- analyst agents at `hermodr-analytics` gives them dataset_list/dataset_profile/
-- run_sql/plot.
--
-- Apply (asgard MariaDB — where agent_configs lives):
--   kubectl exec -i -n asgard <mariadb-pod> -- sh -c \
--     'mariadb -uroot -p"$MYSQL_ROOT_PASSWORD" mimir' < wire-analyst-mcp.sql
-- Then restart Bifrost.
-- Rollback: UPDATE agent_configs SET mcp_servers=NULL WHERE tenant_id='asgard_analytics';

UPDATE agent_configs
SET mcp_servers = '["hermodr-analytics"]'
WHERE tenant_id = 'asgard_analytics'
  AND name IN ('analyst-sql', 'analyst-geo', 'analyst-stats', 'analyst-research', 'analyst-router');

SELECT name, mcp_servers FROM agent_configs WHERE tenant_id = 'asgard_analytics' ORDER BY name;
