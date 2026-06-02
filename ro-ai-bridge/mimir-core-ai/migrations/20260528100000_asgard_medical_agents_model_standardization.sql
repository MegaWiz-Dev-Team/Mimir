-- ============================================================================
-- 2026-05-28 — Asgard Medical Agents Model Standardization to gemma-4-26b
--
-- Standardizes all asgard_medical agents to use gemma-4-26b as the default
-- reasoning model, replacing medgemma-27b-text and Q4 quantized variants.
--
-- Rationale: Medical MCQ benchmark (2026-05-22) shows gemma-4-26b as local
-- champion (47.8% HealthBench-Pro) while medgemma-27b underperforms general
-- gemma. Latency benchmarks confirm gemma-4-26b full precision meets ≤2s p50
-- budget on production Heimdall MLX infra.
--
-- Agents affected:
-- - eir-pediatrics: medgemma-27b-text → gemma-4-26b
-- - eir-psychiatry: medgemma-27b-text → gemma-4-26b
-- - eir-emergency: gemma-4-26b Q4 → gemma-4-26b (full precision)
-- - eir-nursing: gemma-4-26b Q4 → gemma-4-26b (full precision)
--
-- Note: This is an idempotent UPDATE. Non-matching rows are unaffected.
-- ============================================================================

UPDATE agent_configs
SET model_id = 'gemma-4-26b'
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir-pediatrics', 'eir-psychiatry', 'eir-emergency', 'eir-nursing');

-- ─── Audit note ──────────────────────────────────────────────────────────
-- These updates are recorded in Tyr audit_events if the agent is queried
-- after migration. Manual audit of agent invocations post-deployment is
-- recommended to confirm model_id propagates to Heimdall correctly.
-- ──────────────────────────────────────────────────────────────────────────
