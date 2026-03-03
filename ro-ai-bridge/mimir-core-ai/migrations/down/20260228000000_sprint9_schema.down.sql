-- Rollback: 20260228000000_sprint9_schema.sql
-- Revert source_type back to ENUM and drop Sprint 9 tables

ALTER TABLE data_sources MODIFY COLUMN source_type ENUM('web', 'tabular', 'document', 'mcp') NOT NULL;
DROP TABLE IF EXISTS content_fingerprints;
DROP TABLE IF EXISTS crawled_pages;
DROP TABLE IF EXISTS chunks;
