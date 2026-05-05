-- Sprint 38 — Specialty Router Foundation (B-27 + B-29 partial)
--
-- Architecture: per-tenant specialty agents.
-- Each tenant can opt into N specialty agents (Cardio, Sleep, ENT, Peds, ...);
-- a router endpoint classifies the question → routes to the right specialist.
-- Specialists inherit from the canonical Eir prompt + add specialty-specific
-- knowledge framing.
--
-- This migration:
--   1. Adds `specialty` column to agent_configs (NULL = generic/router)
--   2. Adds `is_router` boolean for the dispatch agent
--   3. Adds `routes_to_specialties` JSON list (which specialties this tenant supports)
--   4. Spawns 5 specialist Eir clones for asgard_medical tenant as POC
--
-- Specialty taxonomy (extensible — JSON not enum, lets tenants add custom):
--   - cardio          (cardiology, vascular)
--   - sleep           (OSA, insomnia, narcolepsy — Asgard's strength)
--   - ent             (rhinitis, otitis, ENT)
--   - pediatrics      (peds-specific dosing, growth, vaccines)
--   - generic         (catch-all fallback; current Eir behavior)

ALTER TABLE agent_configs
    ADD COLUMN specialty VARCHAR(40) NULL AFTER name,
    ADD COLUMN is_router TINYINT(1) NOT NULL DEFAULT 0 AFTER specialty,
    ADD COLUMN routes_to_specialties JSON NULL AFTER is_router,
    ADD INDEX idx_agent_configs_specialty (tenant_id, specialty);

-- Mark existing Eir as the generic (current behavior unchanged for back-compat).
UPDATE agent_configs
   SET specialty = 'generic'
 WHERE name = 'eir' AND tenant_id = 'asgard_medical';

-- ── Spawn 4 specialist clones for asgard_medical ─────────────────────────────
-- Each clone copies Eir's CoT prompt + tools, adds a specialty preamble that
-- focuses retrieval and reasoning on the relevant clinical domain. Model choice
-- per cross-benchmark evidence (gemma for RAG synth, flash-lite for fast clinical):

INSERT INTO agent_configs (
    tenant_id, name, specialty, is_router, model_id, provider,
    temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description,
    created_at, updated_at
)
SELECT
    'asgard_medical' AS tenant_id,
    CONCAT('eir-', sp.specialty) AS name,
    sp.specialty,
    0 AS is_router,
    sp.model_id, sp.provider,
    0.30 AS temperature, 16 AS top_k, 4096 AS max_tokens,
    CONCAT(
        '## Specialty: ', sp.display_name, '\n\n',
        'You are a ', sp.display_name, ' specialist clone of Eir. When the user asks about your specialty:\n',
        sp.preamble,
        '\n\n---\n\n',
        (SELECT system_prompt FROM agent_configs WHERE name='eir' AND tenant_id='asgard_medical' LIMIT 1)
    ) AS system_prompt,
    1 AS use_rag, 0 AS use_knowledge_graph, 0 AS use_pageindex,
    (SELECT tools FROM agent_configs WHERE name='eir' AND tenant_id='asgard_medical' LIMIT 1) AS tools,
    '/avatars/eir.png' AS avatar_url,
    CONCAT('Eir ', sp.display_name, ' specialist — focused on ', sp.scope) AS description,
    NOW(), NOW()
FROM (
    SELECT 'cardio' AS specialty, 'Cardiology' AS display_name,
           'mlx-community/gemma-4-26b-a4b-it-4bit' AS model_id, 'heimdall' AS provider,
           'cardiovascular disease (CAD, HF, arrhythmia, HTN), cardiac pharmacology, ECG interpretation' AS scope,
           'Frame answers around: hemodynamics → mechanism → guideline-based therapy → red flags requiring urgent intervention.' AS preamble
    UNION ALL SELECT 'sleep', 'Sleep Medicine',
           'gemini-3.1-flash-lite-preview', 'google',
           'OSA/CSA, insomnia, narcolepsy, parasomnias, CPAP titration',
           'Frame answers around: sleep stage / breathing event → diagnostic criteria (AHI, ESS) → treatment ladder (CPAP, behavioral, pharma) → daytime impact.'
    UNION ALL SELECT 'ent', 'ENT (Otorhinolaryngology)',
           'gemini-3.1-flash-lite-preview', 'google',
           'rhinitis, sinusitis, otitis (acute/chronic), tonsil/adenoid, OSA-anatomy',
           'Frame answers around: anatomy → infection vs allergy vs structural → step-up therapy → surgical indications.'
    UNION ALL SELECT 'pediatrics', 'Pediatrics',
           'gemini-3.1-flash-lite-preview', 'google',
           'pediatric dosing (mg/kg), growth/development, vaccines, peds-specific red flags',
           'Frame answers around: age-band normals → weight-based dosing → red-flag symptoms → caregiver counseling. ALWAYS verify weight before dosing recommendations.'
) AS sp;

-- ── Spawn router agent for asgard_medical ────────────────────────────────────
-- Uses cheap flash-lite to classify question → specialty in one call.
-- Falls through to 'generic' if confidence is low or specialty not enabled.

INSERT INTO agent_configs (
    tenant_id, name, specialty, is_router, routes_to_specialties,
    model_id, provider, temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description,
    created_at, updated_at
)
VALUES (
    'asgard_medical',
    'eir-router',
    'router',
    1,
    JSON_ARRAY('cardio', 'sleep', 'ent', 'pediatrics', 'generic'),
    'gemini-3.1-flash-lite-preview', 'google',
    0.0, 0, 256,
    'You are a medical question router. Given a clinical question, output ONLY a single JSON object:\n\n'
    '  {"specialty": "<one of: cardio | sleep | ent | pediatrics | generic>", "confidence": 0.0-1.0, "reasoning": "<one sentence>"}\n\n'
    'Rules:\n'
    '- "cardio" for heart/vascular/blood pressure/arrhythmia\n'
    '- "sleep" for sleep disorders, OSA, insomnia, CPAP\n'
    '- "ent" for ear/nose/throat/sinus/rhinitis\n'
    '- "pediatrics" for child-specific (mention of age <18, peds dosing, growth)\n'
    '- "generic" for everything else, OR when multiple specialties apply equally\n'
    '- Output JSON only, no prose.',
    0, 0, 0,
    '[]',
    '/avatars/eir.png',
    'Specialty router — classifies clinical questions and dispatches to the right Eir specialist',
    NOW(), NOW()
);

-- Verify
SELECT id, name, specialty, is_router, model_id,
       JSON_LENGTH(routes_to_specialties) AS n_routes
  FROM agent_configs
 WHERE tenant_id = 'asgard_medical'
 ORDER BY is_router DESC, specialty;
