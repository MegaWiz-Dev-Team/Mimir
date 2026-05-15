-- 🔄 Asgard Tenant Recovery Script
-- Recreates asgard_medical tenant and core agents from scratch
-- Run this if tenant data is lost

-- ─────────────────────────────────────────────────────────────────
-- STEP 1: Create/Recreate Tenants
-- ─────────────────────────────────────────────────────────────────

-- Check if asgard_medical exists; if not, create it
INSERT INTO tenants (id, name, domain, created_at)
SELECT 'asgard_medical', 'MegaCare Hospital', 'medical.megacare.com', NOW()
WHERE NOT EXISTS (SELECT 1 FROM tenants WHERE id = 'asgard_medical');

INSERT INTO tenants (id, name, domain, created_at)
SELECT 'asgard_insurance', 'MegaCare Insurance', 'insurance.megacare.com', NOW()
WHERE NOT EXISTS (SELECT 1 FROM tenants WHERE id = 'asgard_insurance');

-- ─────────────────────────────────────────────────────────────────
-- STEP 2: Clear Old Agent Configs (if needed)
-- ─────────────────────────────────────────────────────────────────
-- WARNING: This will delete existing agents for asgard_medical
-- DELETE FROM agent_configs WHERE tenant_id = 'asgard_medical';

-- ─────────────────────────────────────────────────────────────────
-- STEP 3: Create Base Eir Agent
-- ─────────────────────────────────────────────────────────────────

INSERT INTO agent_configs (
    tenant_id, name, display_name, model_id, provider,
    temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description
)
VALUES (
    'asgard_medical',
    'eir',
    'Eir — Generic Medical Agent',
    'mlx-community/Qwen3.5-9B-MLX-4bit',
    'heimdall',
    0.30,
    16,
    4096,
    'You are Eir, the medical AI assistant for Asgard. You are part of a multi-agent medical platform designed to provide evidence-based clinical support. Your role: answer medical questions with accuracy, cite sources, and defer to domain specialists when needed.\n\nGuidance:\n- Use RAG to ground answers in the knowledge base\n- Cite specific sources (document title, page, date)\n- Flag uncertainty: "I am not confident in this answer"\n- Defer to specialty agents: "For cardiology/sleep/ENT/pediatrics, consider consulting the specialist agent"\n- Always frame as clinical decision support, not diagnosis\n- CRITICAL: Never recommend stopping medications without explicit physician guidance',
    1,
    0,
    0,
    JSON_ARRAY('vector_search', 'graph_search', 'memvid_search'),
    '/avatars/eir.png',
    'Eir — Medical AI Assistant (Generic)'
) ON DUPLICATE KEY UPDATE updated_at = NOW();

-- ─────────────────────────────────────────────────────────────────
-- STEP 4: Create Specialty Agents
-- ─────────────────────────────────────────────────────────────────

INSERT INTO agent_configs (
    tenant_id, name, display_name, model_id, provider,
    temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description
)
VALUES
(
    'asgard_medical', 'eir-cardio', 'Eir Cardiology Specialist',
    'mlx-community/gemma-4-26b-a4b-it-4bit', 'heimdall',
    0.30, 16, 4096,
    'You are Eir Cardiology — a cardiac specialist. Frame answers around: hemodynamics → mechanism → guideline-based therapy → red flags requiring urgent intervention.\n\nDomain focus: cardiovascular disease (CAD, HF, arrhythmia, HTN), cardiac pharmacology, ECG interpretation, cardiac imaging.\n\nBase guidance: You are Eir, the medical AI assistant for Asgard...',
    1, 0, 0,
    JSON_ARRAY('vector_search', 'graph_search', 'memvid_search'),
    '/avatars/eir.png',
    'Eir Cardiology Specialist'
) ON DUPLICATE KEY UPDATE updated_at = NOW();

INSERT INTO agent_configs (
    tenant_id, name, display_name, model_id, provider,
    temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description
)
VALUES
(
    'asgard_medical', 'eir-sleep', 'Eir Sleep Medicine Specialist',
    'gemini-3.1-flash-lite-preview', 'google',
    0.30, 16, 4096,
    'You are Eir Sleep Medicine — a sleep specialist. Frame answers around: sleep stage / breathing event → diagnostic criteria (AHI, ESS) → treatment ladder (CPAP, behavioral, pharma) → daytime impact.\n\nDomain focus: OSA/CSA, insomnia, narcolepsy, parasomnias, CPAP titration, sleep apnea management.\n\nBase guidance: You are Eir, the medical AI assistant for Asgard...',
    1, 0, 0,
    JSON_ARRAY('vector_search', 'graph_search', 'memvid_search'),
    '/avatars/eir.png',
    'Eir Sleep Medicine Specialist'
) ON DUPLICATE KEY UPDATE updated_at = NOW();

INSERT INTO agent_configs (
    tenant_id, name, display_name, model_id, provider,
    temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description
)
VALUES
(
    'asgard_medical', 'eir-ent', 'Eir ENT Specialist',
    'gemini-3.1-flash-lite-preview', 'google',
    0.30, 16, 4096,
    'You are Eir ENT — an otorhinolaryngology specialist. Frame answers around: anatomy → infection vs allergy vs structural → step-up therapy → surgical indications.\n\nDomain focus: rhinitis, sinusitis, otitis (acute/chronic), tonsil/adenoid, OSA-anatomy, hearing loss.\n\nBase guidance: You are Eir, the medical AI assistant for Asgard...',
    1, 0, 0,
    JSON_ARRAY('vector_search', 'graph_search', 'memvid_search'),
    '/avatars/eir.png',
    'Eir ENT Specialist'
) ON DUPLICATE KEY UPDATE updated_at = NOW();

INSERT INTO agent_configs (
    tenant_id, name, display_name, model_id, provider,
    temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description
)
VALUES
(
    'asgard_medical', 'eir-pediatrics', 'Eir Pediatrics Specialist',
    'gemini-3.1-flash-lite-preview', 'google',
    0.30, 16, 4096,
    'You are Eir Pediatrics — a pediatric specialist. Frame answers around: age-band normals → weight-based dosing → red-flag symptoms → caregiver counseling. ALWAYS verify weight before dosing recommendations.\n\nDomain focus: pediatric dosing (mg/kg), growth/development, vaccines, peds-specific red flags, developmental milestones.\n\nBase guidance: You are Eir, the medical AI assistant for Asgard...',
    1, 0, 0,
    JSON_ARRAY('vector_search', 'graph_search', 'memvid_search'),
    '/avatars/eir.png',
    'Eir Pediatrics Specialist'
) ON DUPLICATE KEY UPDATE updated_at = NOW();

-- ─────────────────────────────────────────────────────────────────
-- STEP 5: Create Router Agent
-- ─────────────────────────────────────────────────────────────────

INSERT INTO agent_configs (
    tenant_id, name, display_name,
    model_id, provider, temperature, top_k, max_tokens, system_prompt,
    use_rag, use_knowledge_graph, use_pageindex, tools, avatar_url, description
)
VALUES (
    'asgard_medical',
    'eir-router',
    'Eir Router — Specialty Dispatcher',
    'gemini-3.1-flash-lite-preview',
    'google',
    0.0, 0, 256,
    'You are a medical question router. Given a clinical question, output ONLY a single JSON object with exactly two keys: {"specialty": "...", "confidence": ...}. Specialty must be one of: cardio, sleep, ent, pediatrics, generic. Confidence is 0.0-1.0. If confidence < 0.5, use "generic".',
    0, 0, 0,
    JSON_ARRAY(),
    '/avatars/eir-router.png',
    'Eir Router — Specialty Dispatcher'
) ON DUPLICATE KEY UPDATE updated_at = NOW();

-- ─────────────────────────────────────────────────────────────────
-- STEP 6: Verify Recovered Agents
-- ─────────────────────────────────────────────────────────────────

SELECT
    ac.id,
    ac.name,
    ac.display_name,
    ac.model_id,
    ac.provider,
    ac.created_at
FROM agent_configs ac
WHERE ac.tenant_id = 'asgard_medical'
ORDER BY ac.name;

-- Summary
SELECT
    COUNT(*) as total_agents
FROM agent_configs
WHERE tenant_id = 'asgard_medical';
