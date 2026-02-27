-- ============================================================================
-- Sprint 9 Schema Migration
-- Issue #100: DB Migration — Sprint 9 schema additions
--
-- New tables:
--   chunks              (Issue #95 — Chunking Service)
--   crawled_pages       (Issue #96 — Link Discovery)
--   content_fingerprints (Issue #97 — Cross-source Dedup)
--
-- Schema changes:
--   data_sources.source_type  ENUM → VARCHAR(50)
-- ============================================================================

-- ─── 1. chunks ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS chunks (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    source_id BIGINT NOT NULL,
    chunk_index INT NOT NULL,
    content TEXT NOT NULL,
    token_count INT DEFAULT 0,
    metadata_json JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_id) REFERENCES data_sources(id) ON DELETE CASCADE,
    INDEX idx_chunks_source (source_id)
);

-- ─── 2. crawled_pages ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS crawled_pages (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    source_id BIGINT NOT NULL,
    url VARCHAR(2048) NOT NULL,
    status ENUM('pending', 'crawled', 'failed') DEFAULT 'pending',
    content_hash VARCHAR(64),
    last_crawled_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_id) REFERENCES data_sources(id) ON DELETE CASCADE,
    INDEX idx_crawled_source (source_id),
    INDEX idx_crawled_hash (content_hash)
);

-- ─── 3. content_fingerprints ───────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS content_fingerprints (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    content_hash VARCHAR(64) NOT NULL,
    source_id BIGINT NOT NULL,
    chunk_id BIGINT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_id) REFERENCES data_sources(id) ON DELETE CASCADE,
    FOREIGN KEY (chunk_id) REFERENCES chunks(id) ON DELETE SET NULL,
    INDEX idx_fp_hash (content_hash),
    INDEX idx_fp_source (source_id)
);

-- ─── 4. Extend source_type from ENUM to VARCHAR(50) ───────────────────────────
-- This allows adding new source types without schema changes.
-- Existing values ('web', 'tabular', 'document', 'mcp') are preserved.
ALTER TABLE data_sources MODIFY COLUMN source_type VARCHAR(50) NOT NULL;
