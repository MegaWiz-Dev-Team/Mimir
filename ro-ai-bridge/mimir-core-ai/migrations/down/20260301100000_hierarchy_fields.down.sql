-- Rollback: 20260301100000_hierarchy_fields.sql
ALTER TABLE crawled_pages DROP COLUMN parent_url;
ALTER TABLE crawled_pages DROP COLUMN depth;
ALTER TABLE crawled_pages DROP COLUMN title;
