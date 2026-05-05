-- Sprint 40 — Multi-Benchmark Foundation (B-36h)
--
-- Add `scoring_fn` enum to eval_benchmark_datasets so the eval pipeline + UI can
-- pick the right scoring formula per dataset. Different benchmarks score differently:
--   - HealthBench-Pro (Likert):    healthbench_likert  → HBp% normalized
--   - HealthBench paper-original:  paper_rubric_pct    → % rubric criteria met
--   - MedQA / MedMCQA / MedXpertQA: mcq_accuracy        → exact-match acc %
--   - PubMedQA:                    binary_yes_no        → Y/N/Maybe acc %
--
-- The UI uses this column to label the score column header (HBp% vs Acc%) and
-- to compute per-benchmark Rank/Champion correctly (no cross-rubric mixing).
--
-- Default for existing rows is 'healthbench_likert' (current Mimir behavior).

ALTER TABLE eval_benchmark_datasets
    ADD COLUMN scoring_fn VARCHAR(32) NOT NULL DEFAULT 'healthbench_likert'
    AFTER source;

-- Backfill existing rows (HealthBench-Pro and the older custom OSA set)
UPDATE eval_benchmark_datasets
   SET scoring_fn = 'healthbench_likert'
 WHERE source IN ('healthbench_professional', 'custom');

-- Index for fast filtering in /evaluations UI
CREATE INDEX idx_eval_benchmark_datasets_scoring_fn
    ON eval_benchmark_datasets (scoring_fn);
