-- Sprint 51 — Eval Champion Tracking & Cost
--
-- Adds champion + cost tracking to eval_runs for competitive benchmarking
-- and cost attribution across multiple evaluation runs.
--
-- New columns:
--   - is_champion BOOLEAN — marks the best-performing run for a given agent/model combo
--   - total_cost_usd DECIMAL — actual cloud API spend for the entire run

ALTER TABLE eval_runs
    ADD COLUMN is_champion BOOLEAN NOT NULL DEFAULT 0
        COMMENT 'Marks the best run for this agent/model combo (used in UI leaderboard)',
    ADD COLUMN total_cost_usd DECIMAL(10, 4) DEFAULT NULL
        COMMENT 'Total USD cost for this eval run (based on actual API calls)',
    ADD INDEX idx_is_champion (is_champion, started_at DESC);
