-- Add MCP servers to agents for dynamic tool discovery
-- Enables Bifrost to call external MCP services (e.g., Hermodr Mimir knowledge tools)

-- eir-sleep (Sleep Medicine Specialist) — enable PubMed + ICD-10 via MCP
UPDATE agent_configs
SET mcp_servers = JSON_ARRAY('http://hermodr-mimir.asgard.svc:8090/rpc'),
    updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name = 'eir-sleep'
  AND (mcp_servers IS NULL OR mcp_servers = 'null');

-- eir-ent (ENT Specialist) — enable PubMed + ICD-10 via MCP
UPDATE agent_configs
SET mcp_servers = JSON_ARRAY('http://hermodr-mimir.asgard.svc:8090/rpc'),
    updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name = 'eir-ent'
  AND (mcp_servers IS NULL OR mcp_servers = 'null');

-- eir-cardio (Cardiology Specialist) — enable PubMed + ICD-10 via MCP
UPDATE agent_configs
SET mcp_servers = JSON_ARRAY('http://hermodr-mimir.asgard.svc:8090/rpc'),
    updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name = 'eir-cardio'
  AND (mcp_servers IS NULL OR mcp_servers = 'null');

-- eir-pediatrics (Pediatrics Specialist) — enable PubMed + ICD-10 via MCP
UPDATE agent_configs
SET mcp_servers = JSON_ARRAY('http://hermodr-mimir.asgard.svc:8090/rpc'),
    updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name = 'eir-pediatrics'
  AND (mcp_servers IS NULL OR mcp_servers = 'null');

-- Generic eir — enable PubMed + ICD-10 via MCP
UPDATE agent_configs
SET mcp_servers = JSON_ARRAY('http://hermodr-mimir.asgard.svc:8090/rpc'),
    updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name = 'eir'
  AND (mcp_servers IS NULL OR mcp_servers = 'null');
