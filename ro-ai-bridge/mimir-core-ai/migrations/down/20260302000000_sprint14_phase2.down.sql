-- Rollback: 20260302000000_sprint14_phase2.sql

DROP TABLE IF EXISTS feedback_reports;
ALTER TABLE chunks DROP COLUMN ocr_metadata;
ALTER TABLE data_sources DROP COLUMN refresh_status;
ALTER TABLE data_sources DROP COLUMN next_refresh_at;
ALTER TABLE data_sources DROP COLUMN last_refreshed_at;
ALTER TABLE data_sources DROP COLUMN refresh_interval_hours;
