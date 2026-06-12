-- Owned feedback store: ONE place for agent-answer feedback across all tenants.
--
-- Why: dashboard thumbs were written to agent_conversations.feedback (a column),
-- while RL read a separate non-existent table. This table is the single
-- source-of-truth that the feedback API writes to and that RL / fine-tune /
-- Laminar-bridge consume. Tenant-scoped via tenant_id.
--
-- Derivation: thumbs_up -> quality_score 1.000, thumbs_down -> 0.000.
-- feedback_domain stays NULL until tagged (RL groups by it; NULL rows are ignored
-- by RL, not errors). trace_id is reserved for Phase 2 Laminar span linkage.

CREATE TABLE IF NOT EXISTS agent_feedback (
  id              BIGINT AUTO_INCREMENT PRIMARY KEY,
  tenant_id       VARCHAR(64)  NOT NULL,
  conversation_id BIGINT       NULL,
  session_id      VARCHAR(128) NULL,
  agent_id        BIGINT       NULL,
  feedback        VARCHAR(16)  NOT NULL,
  quality_score   DECIMAL(4,3) NOT NULL,
  feedback_domain VARCHAR(128) NULL,
  reason          TEXT         NULL,
  reviewer        VARCHAR(128) NULL,
  source          VARCHAR(32)  NOT NULL DEFAULT 'dashboard',
  trace_id        VARCHAR(64)  NULL,
  created_at      TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  KEY idx_tenant_agent_created (tenant_id, agent_id, created_at),
  KEY idx_conversation (conversation_id),
  KEY idx_session (session_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_uca1400_ai_ci;
