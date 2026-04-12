-- ============================================================================
-- Add LLM Parameters to RAG Evaluation System
--
-- Adds LLM parameter tracking (search AND generation variables) to eval runs.
-- Backfills nulls with safe defaults.
-- Formalizes the auto-tuner jobs schema.
-- ============================================================================

-- 1. Add missing columns to `rag_eval_runs`
ALTER TABLE rag_eval_runs
  ADD COLUMN search_provider VARCHAR(50) AFTER judge_provider,
  ADD COLUMN search_model VARCHAR(100) AFTER search_provider,
  ADD COLUMN generation_provider VARCHAR(50) AFTER search_model,
  ADD COLUMN generation_model VARCHAR(100) AFTER generation_provider,
  ADD COLUMN generation_temperature DOUBLE DEFAULT 0.1 AFTER generation_model,
  ADD COLUMN generation_max_tokens INT DEFAULT 1024 AFTER generation_temperature;

-- 2. Backfill existing rows with 'default' so we don't break old comparisons
UPDATE rag_eval_runs 
SET search_provider = 'default',
    search_model = 'default',
    generation_provider = 'default',
    generation_model = 'default'
WHERE search_provider IS NULL;

-- 3. Formalize `rag_auto_tuner_jobs`
CREATE TABLE IF NOT EXISTS rag_auto_tuner_jobs (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    target_metric VARCHAR(50) DEFAULT 'ndcg',
    iterations INT DEFAULT 3,
    current_iteration INT DEFAULT 0,
    status VARCHAR(20) DEFAULT 'running',
    base_run_id VARCHAR(36),
    best_run_id VARCHAR(36),
    dataset_id VARCHAR(36),
    dataset_name VARCHAR(255),
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    INDEX idx_tenant (tenant_id),
    INDEX idx_dataset (dataset_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
