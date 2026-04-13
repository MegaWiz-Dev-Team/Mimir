-- T2.3: Baseline Pinning
ALTER TABLE rag_eval_runs ADD COLUMN IF NOT EXISTS is_baseline BOOLEAN DEFAULT FALSE;

-- T2.4: Regression Detection
ALTER TABLE rag_eval_runs ADD COLUMN IF NOT EXISTS regression_detected BOOLEAN DEFAULT FALSE;
