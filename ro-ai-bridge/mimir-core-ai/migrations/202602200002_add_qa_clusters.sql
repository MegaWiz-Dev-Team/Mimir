-- Phase 7: Backend Data Quality Control
-- Add qa_clusters table for Resolution UI

CREATE TABLE IF NOT EXISTS qa_clusters (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL DEFAULT 'default_tenant',
    topic VARCHAR(255) NOT NULL,
    reasoning TEXT,
    cluster_type VARCHAR(20) NOT NULL COMMENT 'CONFLICT or DUPLICATE',
    golden_answer TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING' COMMENT 'PENDING, RESOLVED_A, RESOLVED_B, MERGED, MANUAL_OVERRIDE',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_tenant (tenant_id),
    INDEX idx_status (status)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS qa_cluster_items (
    cluster_id VARCHAR(36) NOT NULL,
    qa_id BIGINT NOT NULL,
    source_label VARCHAR(10) NOT NULL COMMENT 'A, B, C, etc.',
    FOREIGN KEY (cluster_id) REFERENCES qa_clusters(id) ON DELETE CASCADE,
    FOREIGN KEY (qa_id) REFERENCES qa_results(id) ON DELETE CASCADE,
    PRIMARY KEY (cluster_id, qa_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
