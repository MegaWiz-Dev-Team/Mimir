-- Rollback: 202602210000_add_tenant_id.sql
-- Removes tenant_id columns from all tables

DROP INDEX idx_eval_summary_tenant ON eval_summary;
ALTER TABLE eval_summary DROP COLUMN tenant_id;

DROP INDEX idx_eval_scores_tenant ON eval_scores;
ALTER TABLE eval_scores DROP COLUMN tenant_id;

DROP INDEX idx_eval_runs_tenant ON eval_runs;
ALTER TABLE eval_runs DROP COLUMN tenant_id;

DROP INDEX idx_evaluation_reports_tenant ON evaluation_reports;
ALTER TABLE evaluation_reports DROP COLUMN tenant_id;

DROP INDEX idx_pipeline_steps_tenant ON pipeline_steps;
ALTER TABLE pipeline_steps DROP COLUMN tenant_id;

DROP INDEX idx_pipeline_runs_tenant ON pipeline_runs;
ALTER TABLE pipeline_runs DROP COLUMN tenant_id;

DROP INDEX idx_qa_results_tenant ON qa_results;
ALTER TABLE qa_results DROP COLUMN tenant_id;
