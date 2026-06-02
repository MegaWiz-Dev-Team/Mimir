-- ============================================================================
-- Rollback: Asgard Medical Agents Model Standardization
--
-- Restores specialty-specific model variants:
-- - eir-pediatrics: gemma-4-26b → medgemma-27b-text
-- - eir-psychiatry: gemma-4-26b → medgemma-27b-text
-- - eir-emergency: gemma-4-26b → gemma-4-26b Q4
-- - eir-nursing: gemma-4-26b → gemma-4-26b Q4
-- ============================================================================

UPDATE agent_configs
SET model_id = 'medgemma-27b-text'
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir-pediatrics', 'eir-psychiatry');

UPDATE agent_configs
SET model_id = 'gemma-4-26b Q4'
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir-emergency', 'eir-nursing');
