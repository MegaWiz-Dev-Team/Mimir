-- ============================================================================
-- Sprint 13 — Agent Studio & LLM Performance Intelligence
--
-- New tables:
--   agent_configs         — Agent configuration (no-code builder)
--   agent_conversations   — Conversation logging (Playground + Agent Studio)
--   evaluation_reports    — Evaluation batch results for model comparison
--   llm_budget_configs    — Per-model daily token budget settings
-- ============================================================================

-- ─── Agent Configurations ──────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS agent_configs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    display_name VARCHAR(200),
    description TEXT,
    system_prompt TEXT NOT NULL,
    model_id VARCHAR(100) NOT NULL,
    provider VARCHAR(50) NOT NULL DEFAULT 'ollama',
    temperature DECIMAL(3,2) DEFAULT 0.70,
    max_tokens INT DEFAULT 2048,
    top_k INT DEFAULT 5,
    use_rag BOOLEAN DEFAULT TRUE,
    use_knowledge_graph BOOLEAN DEFAULT FALSE,
    tools JSON COMMENT 'Array of enabled tool names',
    personality_traits JSON COMMENT 'Array of trait strings',
    greeting TEXT,
    avatar_url VARCHAR(500),
    template_id VARCHAR(50) COMMENT 'Template used to create this agent',
    is_published BOOLEAN DEFAULT FALSE,
    api_key VARCHAR(100) COMMENT 'Generated API key for external access',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    UNIQUE KEY unique_agent_name (tenant_id, name)
);

-- ─── Agent Conversations ───────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS agent_conversations (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    agent_config_id BIGINT,
    session_id VARCHAR(100) NOT NULL,
    user_id BIGINT COMMENT 'Nullable — users table TBD in future sprint',
    role ENUM('user', 'assistant', 'system') NOT NULL,
    content TEXT NOT NULL,
    model_id VARCHAR(100),
    latency_ms INT,
    input_tokens INT DEFAULT 0,
    output_tokens INT DEFAULT 0,
    confidence_score DECIMAL(5,2),
    sources JSON COMMENT 'Source citations used',
    tools_used JSON COMMENT 'Tools invoked in this turn',
    feedback ENUM('thumbs_up', 'thumbs_down') NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    FOREIGN KEY (agent_config_id) REFERENCES agent_configs(id) ON DELETE SET NULL,
    INDEX idx_user (user_id),
    INDEX idx_session (session_id),
    INDEX idx_agent_conv (agent_config_id, created_at)
);

-- ─── Evaluation Reports ────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS evaluation_reports (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    agent_config_id BIGINT,
    model_id VARCHAR(100) NOT NULL,
    question TEXT NOT NULL,
    expected_answer TEXT,
    actual_answer TEXT,
    accuracy INT DEFAULT 0 COMMENT '1-5 score',
    completeness INT DEFAULT 0 COMMENT '1-5 score',
    relevance INT DEFAULT 0 COMMENT '1-5 score',
    reasoning TEXT,
    latency_ms INT DEFAULT 0,
    batch_id VARCHAR(100) COMMENT 'Groups evaluations from same run',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    FOREIGN KEY (agent_config_id) REFERENCES agent_configs(id) ON DELETE SET NULL,
    INDEX idx_eval_batch (batch_id),
    INDEX idx_eval_model (model_id, created_at)
);

-- ─── LLM Budget Configs ────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS llm_budget_configs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    model_id VARCHAR(100) NOT NULL,
    daily_token_limit BIGINT DEFAULT 0 COMMENT '0 = unlimited',
    alert_threshold_pct INT DEFAULT 80 COMMENT 'Percentage at which to alert',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    UNIQUE KEY unique_budget (tenant_id, model_id)
);
