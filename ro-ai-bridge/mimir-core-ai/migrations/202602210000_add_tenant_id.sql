-- Add tenant_id to all relevant tables

-- qa_results
ALTER TABLE qa_results ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_qa_results_tenant ON qa_results(tenant_id);

-- pipeline_runs
ALTER TABLE pipeline_runs ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_pipeline_runs_tenant ON pipeline_runs(tenant_id);

-- pipeline_steps
ALTER TABLE pipeline_steps ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_pipeline_steps_tenant ON pipeline_steps(tenant_id);

-- qa_clusters tenant_id was already added in the creation script.
-- evaluation_reports
ALTER TABLE evaluation_reports ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_evaluation_reports_tenant ON evaluation_reports(tenant_id);

-- eval_runs
ALTER TABLE eval_runs ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_eval_runs_tenant ON eval_runs(tenant_id);

-- eval_scores
ALTER TABLE eval_scores ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_eval_scores_tenant ON eval_scores(tenant_id);

-- eval_summary
ALTER TABLE eval_summary ADD COLUMN tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant';
CREATE INDEX idx_eval_summary_tenant ON eval_summary(tenant_id);
