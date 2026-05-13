-- ============================================================================
-- Phase 4: A2A Multi-Agent Chain Tracking
-- Enables chaining: medical_review_agent → underwriting_agent → compliance_agent
-- Tracks chain_id, parent_dispatch_id, chain_step for multi-hop workflows
-- Enforces max depth (5 hops) and cycle detection
-- ============================================================================

-- ─── Extend a2a_dispatch_audit table ─────────────────────────────────────────

ALTER TABLE a2a_dispatch_audit
    ADD COLUMN chain_id           VARCHAR(36)  NULL             AFTER id,
    ADD COLUMN parent_dispatch_id VARCHAR(36)  NULL             AFTER chain_id,
    ADD COLUMN chain_step         TINYINT      NOT NULL DEFAULT 1 AFTER parent_dispatch_id,
    ADD INDEX  idx_a2a_chain (chain_id),
    ADD INDEX  idx_a2a_parent (parent_dispatch_id);

-- ─── New table: a2a_chain_registry ──────────────────────────────────────────

-- Tracks the lifecycle and validation state of a chain
-- chain_id → multiple dispatch audit rows
CREATE TABLE IF NOT EXISTS a2a_chain_registry (
    id                  VARCHAR(36)  PRIMARY KEY,
    initiated_by_tenant VARCHAR(100) NOT NULL,
    initiated_by_agent  VARCHAR(100) NOT NULL,
    current_step        TINYINT      NOT NULL DEFAULT 1,
    max_steps           TINYINT      NOT NULL DEFAULT 5,
    status              ENUM('in_progress', 'complete', 'failed') DEFAULT 'in_progress',
    -- JSON array of "tenant:agent" strings, used for cycle detection
    visited_agents      JSON,
    created_at          TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at          TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

    INDEX idx_chain_status (status),
    INDEX idx_chain_tenant (initiated_by_tenant)
);

-- ─── Seed: No default chains (created at runtime) ──────────────────────────
