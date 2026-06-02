-- ============================================================================
-- Enforce LOCAL-ONLY for all asgard_medical Eir agents
-- ----------------------------------------------------------------------------
-- Purpose: guarantee no cloud-LLM drift on the asgard_medical tenant.
--          All Eir agents MUST run on Heimdall (local MLX), never Gemini/cloud.
--          Aligns the live DB with the canonical seed:
--            Mimir/scripts/seed-asgard-medical-agents.sql
--
-- Policy:  Eir agents on asgard_medical are LOCAL-ONLY (cloud LLM banned),
--          incl. Emergency / ENT / Nursing / Router. Low latency is achieved
--          by tuning local models (shorter max_tokens, lower temperature),
--          NOT by switching to cloud.
--
-- ⚠️ BACKUP FIRST (touches persistent state):
--      mariadb-dump --single-transaction mimir agent_configs \
--        > backup_agent_configs_$(date +%F).sql
--      # verify the dump is non-empty before proceeding
--
-- Run:    mariadb -u mimir -p mimir < enforce-asgard-medical-local-only.sql
-- ============================================================================

-- 1) BEFORE — show any agent NOT on the local provider (the drift, if any)
SELECT '=== BEFORE: non-local asgard_medical agents ===' AS audit;
SELECT name, display_name, provider, model_id
FROM agent_configs
WHERE tenant_id = 'asgard_medical'
  AND (provider <> 'heimdall' OR model_id LIKE 'gemini%' OR model_id LIKE 'gpt%' OR model_id LIKE 'claude%');

-- 2) ENFORCE — force every asgard_medical agent onto the local gateway/model.
--    Idempotent: re-running is a no-op once already local.
UPDATE agent_configs
SET provider = 'heimdall',
    model_id = 'gemma-4-26b'
WHERE tenant_id = 'asgard_medical'
  AND (provider <> 'heimdall' OR model_id LIKE 'gemini%' OR model_id LIKE 'gpt%' OR model_id LIKE 'claude%');

-- 3) AFTER — must return ZERO rows; otherwise investigate.
SELECT '=== AFTER: remaining non-local agents (must be empty) ===' AS audit;
SELECT name, provider, model_id
FROM agent_configs
WHERE tenant_id = 'asgard_medical'
  AND provider <> 'heimdall';

-- 4) FINAL ROSTER — confirm all 20 agents are local.
SELECT '=== asgard_medical roster (expect 20 rows, all heimdall) ===' AS audit;
SELECT name, display_name, provider, model_id, is_published
FROM agent_configs
WHERE tenant_id = 'asgard_medical'
ORDER BY tier, name;
