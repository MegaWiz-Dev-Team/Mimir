-- ============================================================================
-- Sprint 12 — LLM Usage Logging (Issue #3)
--
-- New table:
--   llm_usage_logs — Track token usage, latency, and cost for every LLM call
-- ============================================================================

CREATE TABLE IF NOT EXISTS llm_usage_logs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    model_id VARCHAR(100) NOT NULL,
    provider VARCHAR(50) NOT NULL,
    endpoint VARCHAR(255),
    caller VARCHAR(100) COMMENT 'Feature/agent that triggered the call (e.g. extract_with_ai, agent_chat, qa_generation, sync_source)',
    input_tokens INT DEFAULT 0,
    output_tokens INT DEFAULT 0,
    total_tokens INT DEFAULT 0,
    latency_ms INT DEFAULT 0,
    status ENUM('success', 'error', 'timeout') DEFAULT 'success',
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    INDEX idx_llm_usage_tenant (tenant_id),
    INDEX idx_llm_usage_model (model_id),
    INDEX idx_llm_usage_created (created_at)
);
