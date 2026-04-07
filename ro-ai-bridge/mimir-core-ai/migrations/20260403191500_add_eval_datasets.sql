-- ============================================================================
-- Add RAG Evaluation Datasets
-- 
-- Adds a new table to store benchmark test suites (eval sets) so users
-- can reuse consistent questions/expected answers.
-- Also tracks which dataset was used in particular benchmark runs.
-- ============================================================================

CREATE TABLE IF NOT EXISTS rag_eval_datasets (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    eval_set JSON NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_auth (tenant_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

ALTER TABLE rag_eval_runs 
ADD COLUMN IF NOT EXISTS dataset_id VARCHAR(36) NULL,
ADD COLUMN IF NOT EXISTS dataset_name VARCHAR(255) NULL;
