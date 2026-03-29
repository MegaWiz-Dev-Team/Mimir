-- Add missing columns to pipeline_runs table used by auto_pipeline.rs
ALTER TABLE pipeline_runs
ADD COLUMN source_id BIGINT DEFAULT NULL,
ADD COLUMN prompt_version VARCHAR(50) DEFAULT NULL,
ADD COLUMN run_label VARCHAR(100) DEFAULT NULL,
ADD COLUMN error_message TEXT DEFAULT NULL;

CREATE INDEX idx_pipeline_runs_source ON pipeline_runs(source_id);
