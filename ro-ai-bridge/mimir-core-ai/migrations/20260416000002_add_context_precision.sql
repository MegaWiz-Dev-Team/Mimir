-- Add missing context_precision columns to eval tables
ALTER TABLE rag_eval_runs ADD COLUMN IF NOT EXISTS avg_context_precision DOUBLE DEFAULT NULL;
ALTER TABLE rag_eval_queries ADD COLUMN IF NOT EXISTS context_precision DOUBLE DEFAULT NULL;
