ALTER TABLE data_sources 
ADD COLUMN storage_mode VARCHAR(10) DEFAULT 'markdown',
ADD COLUMN s3_key VARCHAR(500),
ADD COLUMN file_hash VARCHAR(64);
