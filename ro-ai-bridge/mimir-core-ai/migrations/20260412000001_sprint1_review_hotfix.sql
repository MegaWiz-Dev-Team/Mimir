-- Sprint 1 Review Hotfix: Widen difficulty VARCHAR(10) → VARCHAR(20) to fit "intermediate"
-- and change DEFAULT 0 → DEFAULT NULL for telemetry columns (semantic correctness: NULL = no data, 0 = measured zero)

-- C3: Fix difficulty column width
ALTER TABLE rag_eval_datasets MODIFY COLUMN difficulty VARCHAR(20);

-- M1: Fix DEFAULT semantics for telemetry columns (NULL = "not measured" vs 0 = "measured zero")
ALTER TABLE rag_eval_runs MODIFY COLUMN total_prompt_tokens INT DEFAULT NULL;
ALTER TABLE rag_eval_runs MODIFY COLUMN total_completion_tokens INT DEFAULT NULL;
ALTER TABLE rag_eval_runs MODIFY COLUMN total_thinking_tokens INT DEFAULT NULL;

ALTER TABLE rag_eval_queries MODIFY COLUMN prompt_tokens INT DEFAULT NULL;
ALTER TABLE rag_eval_queries MODIFY COLUMN completion_tokens INT DEFAULT NULL;
ALTER TABLE rag_eval_queries MODIFY COLUMN thinking_tokens INT DEFAULT NULL;
ALTER TABLE rag_eval_queries MODIFY COLUMN ttft_ms INT DEFAULT NULL;

-- M3/T1.10: Add difficulty & question_type columns to per-query table for Difficulty Badge support
ALTER TABLE rag_eval_queries ADD COLUMN IF NOT EXISTS difficulty VARCHAR(20) DEFAULT NULL;
ALTER TABLE rag_eval_queries ADD COLUMN IF NOT EXISTS question_type VARCHAR(20) DEFAULT NULL;
