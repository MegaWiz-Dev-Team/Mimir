-- Reverse of 20260915120000_gemini_3_replaces_retired_2_5: only the catalog flags are restored.
-- The slot rewrite is deliberately not reversed — after 2027-03-31 a Gemini 2.5 id no longer
-- answers, so pointing configuration back at it would be a regression, not a rollback.
UPDATE ai_models SET is_active = TRUE WHERE model_id LIKE 'gemini-2.5-%';
