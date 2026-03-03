-- Rollback: 20260221213000_alter_pipeline_steps_error_message.sql
-- Revert MEDIUMTEXT back to TEXT
ALTER TABLE pipeline_steps MODIFY COLUMN error_message TEXT;
