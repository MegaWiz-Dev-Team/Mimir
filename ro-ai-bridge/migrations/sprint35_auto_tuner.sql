-- Sprint 35: Auto-Tuner Job Tracking
-- Supports the autonomous RAG parameter optimization loop

CREATE TABLE IF NOT EXISTS rag_auto_tuner_jobs (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    target_metric VARCHAR(50) NOT NULL DEFAULT 'ndcg',
    iterations INT NOT NULL DEFAULT 3,
    current_iteration INT NOT NULL DEFAULT 0,
    status VARCHAR(50) NOT NULL DEFAULT 'started',
    base_run_id VARCHAR(36) NULL,
    best_run_id VARCHAR(36) NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL
);

CREATE INDEX idx_rag_auto_tuner_tenant ON rag_auto_tuner_jobs(tenant_id);
CREATE INDEX idx_rag_auto_tuner_status ON rag_auto_tuner_jobs(status);
