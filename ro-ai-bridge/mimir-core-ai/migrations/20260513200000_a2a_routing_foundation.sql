-- ============================================================================
-- Phase 2: Agent-to-Agent (A2A) Cross-Tenant Routing Foundation
-- Enables medical_review_agent (asgard_medical) → underwriting_agent (asgard_insurance)
-- ============================================================================

-- A2A routing rules: define which agents can dispatch to which agents across tenants
CREATE TABLE IF NOT EXISTS a2a_routing_rules (
    id VARCHAR(36) PRIMARY KEY,
    source_tenant_id VARCHAR(100) NOT NULL,
    source_agent_id VARCHAR(100) NOT NULL,
    target_tenant_id VARCHAR(100) NOT NULL,
    target_agent_id VARCHAR(100) NOT NULL,

    -- Route condition: optional JSON filter (e.g., {"message_type":"medical_analysis_complete"})
    condition_json JSON,

    -- Whether this rule is active
    enabled BOOLEAN DEFAULT 1,

    -- Metadata
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

    -- Indexes for fast lookup
    INDEX idx_a2a_source (source_tenant_id, source_agent_id),
    INDEX idx_a2a_target (target_tenant_id, target_agent_id),
    INDEX idx_a2a_enabled (enabled),

    -- Uniqueness: one routing rule per source→target pair (per condition)
    UNIQUE KEY unique_a2a_route (source_tenant_id, source_agent_id, target_tenant_id, target_agent_id)
);

-- ─── Seed Default Cross-Tenant Routes ─────────────────────────────────────

-- Route 1: Medical Review → Insurance Underwriting
INSERT IGNORE INTO a2a_routing_rules
    (id, source_tenant_id, source_agent_id, target_tenant_id, target_agent_id,
     condition_json, description)
VALUES
    ('route_med2ins_001',
     'asgard_medical', 'medical_review_agent',
     'asgard_insurance', 'underwriting_agent',
     '{"message_type":"medical_analysis_complete"}',
     'Route clinical findings from medical review to insurance underwriting');

-- ─── A2A Message Audit Trail ─────────────────────────────────────────────

-- Log all cross-tenant message dispatches for compliance + debugging
CREATE TABLE IF NOT EXISTS a2a_dispatch_audit (
    id VARCHAR(36) PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    -- Source side
    source_tenant_id VARCHAR(100) NOT NULL,
    source_agent_id VARCHAR(100) NOT NULL,
    source_session_id VARCHAR(255),

    -- Target side
    target_tenant_id VARCHAR(100) NOT NULL,
    target_agent_id VARCHAR(100) NOT NULL,
    target_session_id VARCHAR(255),

    -- Message payload (before redaction)
    message_summary VARCHAR(500),

    -- PII redaction applied
    pii_redaction_applied BOOLEAN DEFAULT 0,
    pii_fields_redacted JSON,  -- e.g., {"thai_id": 5, "phone": 2}

    -- Outcome
    status ENUM('pending', 'delivered', 'failed') DEFAULT 'pending',
    error_message TEXT,

    -- Indexes
    INDEX idx_a2a_audit_source (source_tenant_id, timestamp),
    INDEX idx_a2a_audit_target (target_tenant_id, timestamp),
    INDEX idx_a2a_audit_status (status)
);

-- ─── A2A Message Redaction Log (Skuggi integration) ──────────────────────

-- Track what PII was redacted in each A2A dispatch (separate from audit for compliance)
CREATE TABLE IF NOT EXISTS a2a_redaction_log (
    id VARCHAR(36) PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    a2a_dispatch_id VARCHAR(36) NOT NULL,

    -- What was redacted
    original_text TEXT,
    redacted_text TEXT,

    -- Type of PII detected
    pii_type VARCHAR(50),  -- thai_national_id, phone_number, email, medical_cert, etc.

    -- Confidence score (from regex)
    confidence_score FLOAT,

    FOREIGN KEY (a2a_dispatch_id) REFERENCES a2a_dispatch_audit(id) ON DELETE CASCADE,
    INDEX idx_redaction_pii_type (pii_type),
    INDEX idx_redaction_dispatch (a2a_dispatch_id)
);
