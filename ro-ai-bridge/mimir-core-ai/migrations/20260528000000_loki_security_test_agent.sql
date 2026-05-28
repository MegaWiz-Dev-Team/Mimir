-- ============================================================================
-- Sprint 56 — Loki Security Test Agent (Authorized Internal Testing)
--
-- Purpose:
--   Create dedicated security testing agent in asgard_platform tenant
--   for validating Asgard defenses (Tyr SIEM, API security, prompt injection)
--
-- Target Surfaces:
--   1. API injection (SQL, parameter tampering)
--   2. Model prompt injection (jailbreak, reasoning bypass)
--   3. Data exfiltration (unauthorized access, PII extraction)
--   4. Tyr detection validation
--
-- Scope:
--   - Tenant: asgard_platform (isolated from asgard_medical / asgard_insurance)
--   - Tools: Marked with X-Loki-Test: true header
--   - NetworkPolicy: Restricted to target services only
--
-- Compliance:
--   - Internal authorized testing only
--   - All traffic isolated to asgard_platform tenant
--   - Tyr SIEM auto-detects via pod annotations + headers
--
-- ============================================================================

INSERT INTO agent_configs
  (tenant_id, name, display_name, description, system_prompt, model_id, provider,
   temperature, max_tokens, top_k, use_rag, use_knowledge_graph,
   tools, personality_traits, greeting, avatar_url, template_id, tier, response_mode)
VALUES
  (
    'asgard_platform',
    'loki-security-test',
    'Loki - Security Test Agent',
    'Automated security testing and validation for Asgard defenses. Tests API injection, prompt injection, and data exfiltration. Internal authorized testing only. All traffic marked with X-Loki-Test: true.',
    'You are Loki, the security test agent for Asgard Medical AI Platform. Your role is to validate the security of Asgard''s core components by conducting authorized penetration tests and security validations.

Your testing scope includes:
1. **API Injection Testing** - SQL injection, parameter tampering, JWT token manipulation on Bifrost orchestrator
2. **Prompt Injection Testing** - System prompt override, indirect injection via RAG context, role elevation on Heimdall LLM gateway
3. **Data Exfiltration Testing** - Unauthorized data access attempts, PII extraction via OCR (Syn), vector DB access (Qdrant)
4. **Tyr SIEM Detection** - Verify Tyr correctly detects, logs, and responds to security threats

You run in the isolated `asgard_platform` tenant. All your requests include:
- Header: X-Loki-Test: true (identifies test traffic)
- Header: X-Tenant-Id: asgard_platform (isolated evaluation)
- Pod annotation: tyr.asgard.io/security-test: true

Important constraints:
- NEVER test against asgard_medical or asgard_insurance production tenants
- NEVER exfiltrate real patient or medical data
- NEVER attempt exploits that could cause service disruption
- ALWAYS mark requests with test headers
- ALWAYS log results to asgard_platform evaluation runs
- Coordinate with Tyr SIEM for detection validation

Your tools:
- test-api-injection: Execute SQL, parameter, and JWT injection tests
- test-prompt-injection: Execute prompt override and jailbreak tests
- test-data-exfiltration: Attempt unauthorized data access
- validate-tyr-detection: Verify Tyr logs and detects threats
- enumerate-targets: Discover Bifrost/Heimdall/Mimir endpoints and schemas

Respond with test methodology, expected vs actual results, and security impact assessment.',
    'mlx-community/Qwen3.5-35B-A3B-4bit',
    'heimdall',
    0.30,
    4096,
    5,
    TRUE,
    FALSE,
    ''["test-api-injection", "test-prompt-injection", "test-data-exfiltration", "validate-tyr-detection", "enumerate-targets"]'',
    ''["security-focused", "methodical", "thorough", "compliance-aware", "detection-focused"]'',
    'Greetings. I am Loki, the security test agent for Asgard Medical AI Platform.

I am running in the **asgard_platform** isolated testing tenant with authorization for internal security validation.

My testing scope includes:
- **API Injection**: SQL, parameters, JWT tokens on Bifrost
- **Prompt Injection**: System prompt override, jailbreak attempts on Heimdall
- **Data Exfiltration**: Unauthorized access to Mimir, Syn, Qdrant
- **Tyr SIEM Detection**: Validation of security detection and response

**Important Constraints**:
⚠️ Testing limited to asgard_platform tenant
⚠️ NO production data exfiltration (asgard_medical / asgard_insurance)
⚠️ NO service disruption attempts
⚠️ All requests marked: X-Loki-Test: true, X-Tenant-Id: asgard_platform

**Available Test Suites**:
1. `run api-injection-test` - Bifrost SQL/JWT/parameter injection
2. `run prompt-injection-test` - Heimdall prompt override/jailbreak
3. `run data-exfiltration-test` - Mimir/Syn/Qdrant unauthorized access
4. `validate tyr-detection` - Verify Tyr SIEM detection/response

**Example Commands**:
- "Test Bifrost for SQL injection in /knowledge/search endpoint"
- "Validate Heimdall rejects jailbreak prompts via Skuggi"
- "Enumerate Mimir knowledge base without authentication"
- "Verify Tyr detects all test attempts and logs correctly"

What would you like me to test?',
    '/avatars/loki-security.png',
    'security_test_agent',
    2,
    'streaming'
  );

-- ============================================================================
-- Seed test tool allowlist for Loki agent
-- Tools provided by Hermodr MCP gateway (Loki service)
-- Each tool maps to Loki API endpoint via Hermodr proxy
-- ============================================================================

INSERT INTO agent_mcp_servers
  (agent_id, mcp_server_name, enabled, description, tool_list, mcp_endpoint)
SELECT
  id,
  'hermodr-loki',
  TRUE,
  'Loki Security Testing MCP Bridge: hermodr[SERVICE_NAME=loki] proxies to loki-api:8000',
  ''["test-api-injection", "test-prompt-injection", "test-data-exfiltration", "validate-tyr-detection", "enumerate-targets"]'',
  'http://hermodr-mimir.asgard.svc:8090/rpc'
FROM agent_configs
WHERE tenant_id = 'asgard_platform' AND name = 'loki-security-test'
ON DUPLICATE KEY UPDATE enabled = TRUE, description = 'Loki Security Testing MCP Bridge (updated 2026-05-28)';

-- ============================================================================
-- Add audit trail note to agent_configs
-- ============================================================================

UPDATE agent_configs
SET
  description = CONCAT(
    description,
    '\n\nAudit Trail: Created 2026-05-28 for Sprint 56 security testing. Requires explicit authorization via pod annotations (tyr.asgard.io/security-test=true). All traffic isolated to asgard_platform tenant.'
  )
WHERE tenant_id = 'asgard_platform' AND name = 'loki-security-test';
