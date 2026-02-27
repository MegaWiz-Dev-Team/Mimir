-- ============================================================================
-- Sprint 12 — Web Hierarchy Fields (Issue #1)
--
-- Extends crawled_pages table for hierarchy discovery:
--   title      — page title from <title> tag
--   depth      — BFS depth from root URL
--   parent_url — URL of the parent page
-- ============================================================================

ALTER TABLE crawled_pages ADD COLUMN title VARCHAR(500) DEFAULT NULL;
ALTER TABLE crawled_pages ADD COLUMN depth INT DEFAULT 0;
ALTER TABLE crawled_pages ADD COLUMN parent_url VARCHAR(2048) DEFAULT NULL;
