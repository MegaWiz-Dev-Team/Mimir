-- Sprint 36 follow-up — Per-model rerank gating (B-16 calibrated)
--
-- Phase 2 A/B revealed cross_encoder_rerank (BAAI/bge-reranker-v2-m3, general-domain)
-- helps some models but hurts others on medical Q&A. Findings:
--   ✅ gemini-3.1-flash-lite-preview: +4.0pp (44.4 → 48.4)
--   ❌ mlx-community/gemma-4-26b-a4b-it-4bit: -9.1pp (47.8 → 38.7)
--   ❌ gemini-2.5-flash: -6.9pp (43.1 → 36.2)
--
-- Hypothesis: rerank trims context to top-K most query-relevant. Larger reasoning
-- models (gemma-4-26b, 2.5-flash) exploit *peripherally* relevant facts to
-- synthesize answers — rerank cuts those.
--
-- This migration writes per-model `rerank_recommended` into `ai_models.metadata`
-- JSON. The chat path (chat.rs) reads this and decides per-request, with the
-- RERANKER_ENABLED env var as override (1=force on, 0=force off, unset=per-model).
--
-- Default for unflagged models is FALSE (conservative — keep current behavior).
-- Add models to the "true" list as we validate them.

-- Models verified to BENEFIT from rerank (Sprint 36 Phase 2):
UPDATE ai_models
   SET metadata = JSON_SET(
        COALESCE(metadata, JSON_OBJECT()),
        '$.rerank_recommended', TRUE,
        '$.rerank_evidence', JSON_OBJECT(
            'sprint', 36,
            'lift_pp', 4.0,
            'baseline_run_id', 'f56a591e',
            'rerank_run_id',   'fe1b4e9b'
        )
   )
 WHERE model_id IN (
    'gemini-3.1-flash-lite-preview'
 );

-- Models verified to be HURT by rerank (Sprint 36 Phase 2):
UPDATE ai_models
   SET metadata = JSON_SET(
        COALESCE(metadata, JSON_OBJECT()),
        '$.rerank_recommended', FALSE,
        '$.rerank_evidence', JSON_OBJECT(
            'sprint', 36,
            'lift_pp', -9.1,
            'baseline_run_id', '195e8912',
            'rerank_run_id',   '43b60ce3',
            'note', 'Rerank trims peripheral context this model needs for synthesis'
        )
   )
 WHERE model_id IN (
    'mlx-community/gemma-4-26b-a4b-it-4bit'
 );

UPDATE ai_models
   SET metadata = JSON_SET(
        COALESCE(metadata, JSON_OBJECT()),
        '$.rerank_recommended', FALSE,
        '$.rerank_evidence', JSON_OBJECT(
            'sprint', 36,
            'lift_pp', -6.9,
            'baseline_run_id', 'cfef47bf',
            'rerank_run_id',   '8e94f576'
        )
   )
 WHERE model_id IN (
    'gemini-2.5-flash'
 );

-- Verify
SELECT model_id,
       JSON_EXTRACT(metadata, '$.rerank_recommended') AS rerank_rec,
       JSON_EXTRACT(metadata, '$.rerank_evidence.lift_pp') AS lift_pp
  FROM ai_models
 WHERE JSON_EXTRACT(metadata, '$.rerank_recommended') IS NOT NULL;
