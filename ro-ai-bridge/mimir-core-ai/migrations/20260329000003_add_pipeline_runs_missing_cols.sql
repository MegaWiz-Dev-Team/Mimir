-- Add missing columns to pipeline_runs table used by auto_pipeline.rs
ALTER TABLE pipeline_runs
ADD COLUMN IF NOT EXISTS source_id BIGINT DEFAULT NULL,
ADD COLUMN IF NOT EXISTS prompt_version VARCHAR(50) DEFAULT NULL,
ADD COLUMN IF NOT EXISTS run_label VARCHAR(100) DEFAULT NULL,
ADD COLUMN IF NOT EXISTS error_message TEXT DEFAULT NULL;

CREATE INDEX IF NOT EXISTS idx_pipeline_runs_source ON pipeline_runs(source_id);
