-- ============================================================================
-- Gemini 2.5 → Gemini 3 (Google retirement notice, 2026-09-15)
--
-- Vertex AI / Gemini API withdraw Gemini 2.5 from public access on 2026-10-20 and shut it down on
-- 2027-01-28 (2.5 Flash-Lite) / 2027-03-31 (2.5 Flash, 2.5 Pro). Google's named successors:
--   gemini-2.5-flash-lite → gemini-3.1-flash-lite
--   gemini-2.5-flash      → gemini-3.5-flash-lite   (same price: $0.30 / $2.50 per 1M tokens)
--   gemini-2.5-pro        → gemini-3.5-flash
-- Rewrites every stored *configuration* slot; history rows (usage logs, conversations, eval
-- judge_model) keep the model that actually ran.
-- ============================================================================

-- 1. Catalog: the successors exist and are active; the retired ids stay (history) but inactive.
INSERT INTO ai_models (model_id, provider, model_type, is_active, capabilities) VALUES
('gemini-3.1-flash-lite', 'google', 'llm', TRUE, '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3.5-flash-lite', 'google', 'llm', TRUE, '{"tools":true, "vision":true, "reasoning":true}'),
('gemini-3.5-flash',      'google', 'llm', TRUE, '{"tools":true, "vision":true, "reasoning":true}')
ON DUPLICATE KEY UPDATE is_active = TRUE, capabilities = VALUES(capabilities);

-- 2. Personas (FK → ai_models, so the successors above must exist first).
UPDATE ai_npc_persona
SET model_id = CASE
        WHEN model_id LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN model_id LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END
WHERE model_id LIKE 'gemini-2.5-%';

-- 3. Agent Studio agents.
UPDATE agent_configs
SET model_id = CASE
        WHEN model_id LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN model_id LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END
WHERE model_id LIKE 'gemini-2.5-%';

-- 4. Tenant default model.
UPDATE tenant_configs
SET default_model = CASE
        WHEN default_model LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN default_model LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END
WHERE default_model LIKE 'gemini-2.5-%';

-- 5. Tenant per-purpose LLM slots (llm_config JSON).

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.chat.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.chat.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.chat.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.chat.model')) LIKE 'gemini-2.5-%';

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.rag.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.rag.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.rag.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.rag.model')) LIKE 'gemini-2.5-%';

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.pipeline_generator.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_generator.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_generator.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_generator.model')) LIKE 'gemini-2.5-%';

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.pipeline_extractor.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_extractor.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_extractor.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_extractor.model')) LIKE 'gemini-2.5-%';

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.pipeline_evaluator.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_evaluator.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_evaluator.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.pipeline_evaluator.model')) LIKE 'gemini-2.5-%';

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.judge.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.judge.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.judge.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.judge.model')) LIKE 'gemini-2.5-%';

UPDATE tenant_configs
SET llm_config = JSON_SET(llm_config, '$.graph_analyzer.model', CASE
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.graph_analyzer.model')) LIKE 'gemini-2.5-flash-lite%' THEN 'gemini-3.1-flash-lite'
        WHEN JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.graph_analyzer.model')) LIKE 'gemini-2.5-flash%'      THEN 'gemini-3.5-flash-lite'
        ELSE 'gemini-3.5-flash'
    END)
WHERE JSON_UNQUOTE(JSON_EXTRACT(llm_config, '$.graph_analyzer.model')) LIKE 'gemini-2.5-%';


-- 6. Retired ids can no longer be picked.
UPDATE ai_models SET is_active = FALSE WHERE model_id LIKE 'gemini-2.5-%';

