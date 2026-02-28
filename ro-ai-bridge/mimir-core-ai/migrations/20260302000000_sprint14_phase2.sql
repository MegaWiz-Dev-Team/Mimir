-- ============================================================================
-- Sprint 14 — Phase 2 Schema Additions
--
-- Issue #150: Scheduled Re-sync — cron scheduling columns on data_sources
-- Issue #151: OCR Integration — ocr_metadata on chunks
-- Issue #153: Feedback System — feedback_reports table
-- ============================================================================

-- ─── Issue #150: Cron scheduling columns ─────────────────────────────────────
ALTER TABLE data_sources
  ADD COLUMN refresh_interval_hours INT NULL COMMENT 'Auto-refresh every N hours (null = disabled)',
  ADD COLUMN last_refreshed_at TIMESTAMP NULL,
  ADD COLUMN next_refresh_at TIMESTAMP NULL,
  ADD COLUMN refresh_status ENUM('idle', 'running', 'failed') DEFAULT 'idle';

-- ─── Issue #151: OCR metadata on chunks ──────────────────────────────────────
ALTER TABLE chunks
  ADD COLUMN ocr_metadata JSON NULL COMMENT '{"engine":"tesseract","lang":"tha+eng","confidence":0.92,"dpi":300}';

-- ─── Issue #153: Feedback & Bug Report ───────────────────────────────────────
CREATE TABLE IF NOT EXISTS feedback_reports (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    user_id VARCHAR(36) NULL,
    report_type ENUM('bug', 'feedback', 'feature_request') NOT NULL,
    title VARCHAR(200) NOT NULL,
    description TEXT,
    page_url VARCHAR(500),
    browser_info JSON COMMENT '{"userAgent":"...","viewport":"1920x1080"}',
    screenshot_url VARCHAR(500) COMMENT 'Optional screenshot stored in S3',
    priority ENUM('low', 'medium', 'high', 'critical') DEFAULT 'medium',
    status ENUM('open', 'in_progress', 'resolved', 'closed') DEFAULT 'open',
    resolution TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    INDEX idx_feedback_tenant (tenant_id),
    INDEX idx_feedback_status (status),
    INDEX idx_feedback_type (report_type)
);
