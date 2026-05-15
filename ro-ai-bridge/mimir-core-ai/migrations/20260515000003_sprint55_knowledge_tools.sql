-- Sprint 55: Knowledge search tools — expose Qdrant collections via Hermodr
-- Add pubmed_search and icd10_search to agent tool allowlists
-- Wire primekg_search and clinical_kb_search to Bifrost overseer

-- Add pubmed_search to all medical agents (base + specialties)
UPDATE agent_configs SET tools = JSON_ARRAY_APPEND(COALESCE(tools, JSON_ARRAY()), '$', 'pubmed_search'),
       updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir', 'eir-cardio', 'eir-sleep', 'eir-ent', 'eir-pediatrics')
  AND (tools IS NULL OR NOT JSON_CONTAINS(tools, '"pubmed_search"'));

-- Add icd10_search to all medical agents (base + specialties)
UPDATE agent_configs SET tools = JSON_ARRAY_APPEND(COALESCE(tools, JSON_ARRAY()), '$', 'icd10_search'),
       updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir', 'eir-cardio', 'eir-sleep', 'eir-ent', 'eir-pediatrics')
  AND (tools IS NULL OR NOT JSON_CONTAINS(tools, '"icd10_search"'));

-- Ensure primekg_search is present in eir-cardio, eir-sleep (already added in earlier sprints but double-check)
UPDATE agent_configs SET tools = JSON_ARRAY_APPEND(COALESCE(tools, JSON_ARRAY()), '$', 'primekg_search'),
       updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir-cardio', 'eir-sleep')
  AND (tools IS NULL OR NOT JSON_CONTAINS(tools, '"primekg_search"'));

-- Ensure clinical_kb_search is present in eir-sleep (drug interactions, guidelines)
UPDATE agent_configs SET tools = JSON_ARRAY_APPEND(COALESCE(tools, JSON_ARRAY()), '$', 'clinical_kb_search'),
       updated_at = NOW()
WHERE tenant_id = 'asgard_medical'
  AND name IN ('eir-sleep')
  AND (tools IS NULL OR NOT JSON_CONTAINS(tools, '"clinical_kb_search"'));
