-- Rollback: 20260226220000_add_upload_fields.sql
ALTER TABLE data_sources DROP COLUMN file_hash;
ALTER TABLE data_sources DROP COLUMN s3_key;
ALTER TABLE data_sources DROP COLUMN storage_mode;
