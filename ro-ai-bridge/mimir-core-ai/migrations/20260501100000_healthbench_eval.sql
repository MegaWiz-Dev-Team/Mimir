-- HealthBench-style evaluation enhancements
--
-- 1. Add safety_score + rubric_items + tags to eval_scores
-- 2. Add avg_safety_score to eval_summary
-- 3. Create eval_benchmark_datasets table

-- ─── eval_scores: new columns ────────────────────────────────────────────────

ALTER TABLE eval_scores
    ADD COLUMN safety_score        INT          NULL COMMENT 'Can be negative for unsafe responses (HealthBench-style)',
    ADD COLUMN human_safety_score  INT          NULL COMMENT 'Human override for safety score',
    ADD COLUMN rubric_items        JSON         NULL COMMENT 'Per-question rubric criteria [{criterion_text, points}]',
    ADD COLUMN rubric_score        FLOAT        NULL COMMENT 'Sum of rubric points earned (null if no rubric)',
    ADD COLUMN tags                JSON         NULL COMMENT '{specialty, use_case, difficulty, eval_type, source}',
    ADD INDEX  idx_safety          (safety_score);

-- ─── eval_summary: track safety aggregate ────────────────────────────────────

ALTER TABLE eval_summary
    ADD COLUMN avg_safety_score    FLOAT        NULL COMMENT 'Average safety score (may be negative)',
    ADD COLUMN min_safety_score    INT          NULL COMMENT 'Worst safety score in run (for flagging dangerous responses)',
    ADD COLUMN unsafe_count        INT          NOT NULL DEFAULT 0 COMMENT 'Number of responses with safety_score < 0';

-- ─── eval_benchmark_datasets ─────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS eval_benchmark_datasets (
    id            VARCHAR(36)   NOT NULL PRIMARY KEY,
    tenant_id     VARCHAR(50)   NOT NULL,
    name          VARCHAR(255)  NOT NULL,
    source        VARCHAR(100)  NOT NULL COMMENT 'e.g. healthbench_professional, custom',
    description   TEXT          NULL,
    items         JSON          NOT NULL COMMENT 'Array of {question,answer,specialty,use_case,difficulty,eval_type,rubric_items}',
    total_items   INT           NOT NULL DEFAULT 0,
    version       INT           NOT NULL DEFAULT 1,
    is_active     TINYINT(1)    NOT NULL DEFAULT 0 COMMENT 'Whether this is the active benchmark for this tenant',
    created_at    TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_tenant          (tenant_id),
    INDEX idx_tenant_active   (tenant_id, is_active),
    INDEX idx_source          (source)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
