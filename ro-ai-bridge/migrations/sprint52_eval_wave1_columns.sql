-- Sprint 52 hotfix — add Wave 1 eval columns expected by ro-ai-bridge::routes::eval
--
-- The dashboard /evaluations page issues SELECTs against eval_runs / eval_scores
-- that include columns introduced in the Wave 1 refactor (parent_run_id,
-- is_champion, retrieval_trace, …) but no migration ever landed for them
-- on the deployed schema. Discovered 2026-05-20 while wiring the POC
-- Friday demo baseline into Mimir — list_runs returned [] because sqlx
-- query_as failed silently on the missing columns.
--
-- This migration is idempotent-friendly via IF NOT EXISTS where MariaDB
-- supports it; for plain ALTER ADD it's not strictly idempotent, so run
-- once.

ALTER TABLE eval_runs
    ADD COLUMN parent_run_id           VARCHAR(36)  DEFAULT NULL,
    ADD COLUMN baseline_run_id         VARCHAR(36)  DEFAULT NULL,
    ADD COLUMN hypothesis              TEXT         DEFAULT NULL,
    ADD COLUMN variable_under_test     VARCHAR(255) DEFAULT NULL,
    ADD COLUMN expected_change         TEXT         DEFAULT NULL,
    ADD COLUMN is_champion             TINYINT(1)   DEFAULT 0,
    ADD COLUMN total_cost_usd          DECIMAL(10,5) DEFAULT NULL,
    ADD COLUMN total_prompt_tokens     BIGINT       DEFAULT NULL,
    ADD COLUMN total_completion_tokens BIGINT       DEFAULT NULL,
    ADD COLUMN total_thinking_tokens   BIGINT       DEFAULT NULL;

-- sqlx in the deployed binary decodes `config` as Option<String>; LONGTEXT
-- is reported as BLOB at the protocol level which mismatches. TEXT works.
ALTER TABLE eval_runs MODIFY config TEXT;

ALTER TABLE eval_scores
    ADD COLUMN retrieval_trace      TEXT         DEFAULT NULL,
    ADD COLUMN benchmark_item_id    VARCHAR(64)  DEFAULT NULL,
    ADD COLUMN replicate_index      INT          DEFAULT NULL,
    ADD COLUMN retrieval_params     TEXT         DEFAULT NULL,
    ADD COLUMN retrieval_chunks     TEXT         DEFAULT NULL,
    ADD COLUMN step_timings         TEXT         DEFAULT NULL,
    ADD COLUMN tool_calls           TEXT         DEFAULT NULL;
