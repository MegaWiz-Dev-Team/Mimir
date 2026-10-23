-- Per-dispatch RL self-evaluation log.
--
-- Why: Bifrost's RL feedback writer (src/rl_feedback.rs) INSERTs automated,
-- per-response self-eval scores into `agent_feedback_logs`, but no migration
-- ever created the table -> every dispatch logged a WARN ("Table
-- 'mimir.agent_feedback_logs' doesn't exist") and the scores were dropped.
--
-- This is a SEPARATE data stream from `agent_feedback` (20260606000000):
--   * agent_feedback       = coarse human/dashboard thumbs (quality derived 1.0/0.0)
--   * agent_feedback_logs  = fine-grained automated self-eval per dispatch
--     (quality/relevance/latency/confidence). Both feed RL; not duplicates.
-- Columns + types mirror Bifrost's DispatchFeedback struct and its INSERT/aggregation
-- queries (filter by agent_id+tenant_id+created_at; GROUP BY feedback_domain).

CREATE TABLE IF NOT EXISTS agent_feedback_logs (
  id                BIGINT AUTO_INCREMENT PRIMARY KEY,
  tenant_id         VARCHAR(64)  NOT NULL,
  agent_id          BIGINT       NOT NULL,
  session_id        VARCHAR(128) NULL,
  dispatch_id       VARCHAR(64)  NULL,
  quality_score     DECIMAL(4,3) NOT NULL,
  relevance_score   DECIMAL(4,3) NOT NULL,
  latency_score     DECIMAL(4,3) NOT NULL,
  confidence_score  DECIMAL(4,3) NOT NULL,
  user_satisfaction TINYINT(1)   NULL,
  follow_up_needed  TINYINT(1)   NULL,
  feedback_domain   VARCHAR(128) NULL,
  source_context    TEXT         NULL,
  created_at        TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  KEY idx_agent_tenant_created (agent_id, tenant_id, created_at),
  KEY idx_feedback_domain (feedback_domain)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_uca1400_ai_ci;
