-- Sprint 50 B-50g — Add `ocr_extract` to Eir agent tool allowlists.
--
-- Sprint plan originally named target agents `eir-medtech`, `eir-pharmacy`,
-- `eir-internal-medicine` (from the 2026-Q1 spec). Those names never landed;
-- the actual specialty roster from sprint38 is:
--   - eir              (generic clinical)
--   - eir-cardio       (cardiology)
--   - eir-sleep        (sleep medicine — Asgard's strength)
--   - eir-ent          (otorhinolaryngology)
--   - eir-pediatrics   (peds)
--   - eir-router       (classifier — does NOT execute medical work, no OCR needed)
--
-- We add `ocr_extract` to the 5 clinical agents and skip the router. The tool
-- itself is defined in Hermodr (`src/services/syn.rs`) and routes to Syn's
-- 4-tier OCR (PaddleOCR + Typhoon-OCR locally, Gemini Flash/Pro on opt-in).
--
-- Idempotent: only appends if `ocr_extract` isn't already in the list. Safe
-- to run multiple times (e.g. on a re-seeded dev DB).
--
-- Why per-agent and not "all clinical agents"? Each clinical agent's prompt
-- already knows its specialty framing; the tool list controls what the agent
-- is *allowed* to call. Routing decisions (which engine, cloud vs local,
-- budget enforcement) happen downstream in Syn + Mimir's cost guard (B-50m).
--
-- Runtime note (2026-05-11): Bifrost's overseer currently mounts only
-- `vector_search`/`graph_search`/`tree_search` Rig tools from the allowlist
-- (see `Bifrost/src/swarm_engine/overseer.rs` line 118-130 — `_agent_tools`
-- is reserved for future per-tool config). B-50d (transparent OCR path) is
-- the wire-up that turns this declarative intent into runtime capability.
-- This migration ships the contract first so B-50d has something to gate on.

UPDATE agent_configs
   SET tools = JSON_ARRAY_APPEND(COALESCE(tools, JSON_ARRAY()), '$', 'ocr_extract'),
       updated_at = NOW()
 WHERE tenant_id = 'asgard_medical'
   AND name IN ('eir', 'eir-cardio', 'eir-sleep', 'eir-ent', 'eir-pediatrics')
   AND (tools IS NULL OR NOT JSON_CONTAINS(tools, '"ocr_extract"'));
