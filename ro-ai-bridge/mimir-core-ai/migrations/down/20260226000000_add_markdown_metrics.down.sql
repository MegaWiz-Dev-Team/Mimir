-- Rollback: 20260226000000_add_markdown_metrics.sql
ALTER TABLE data_sources DROP COLUMN total_chunks;
ALTER TABLE data_sources DROP COLUMN mb_size;
ALTER TABLE data_sources DROP COLUMN raw_markdown;
