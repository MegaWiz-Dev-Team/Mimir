use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::collections::HashMap;

/// A routing rule for agent-to-agent dispatch (cross-tenant)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct A2aRoutingRule {
    pub id: String,
    pub source_tenant_id: String,
    pub source_agent_id: String,
    pub target_tenant_id: String,
    pub target_agent_id: String,
    pub condition_json: Option<Json<serde_json::Value>>,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Request to create or update an A2A routing rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateA2aRoutingRuleRequest {
    pub source_tenant_id: String,
    pub source_agent_id: String,
    pub target_tenant_id: String,
    pub target_agent_id: String,
    #[serde(default)]
    pub condition_json: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A2A dispatch audit entry (what happened during the cross-tenant call)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct A2aDispatchAudit {
    pub id: String,
    // Phase 4: Chain tracking
    pub chain_id: Option<String>,
    pub parent_dispatch_id: Option<String>,
    pub chain_step: i8,
    pub timestamp: Option<DateTime<Utc>>,
    pub source_tenant_id: String,
    pub source_agent_id: String,
    pub source_session_id: Option<String>,
    pub target_tenant_id: String,
    pub target_agent_id: String,
    pub target_session_id: Option<String>,
    pub message_summary: Option<String>,
    pub pii_redaction_applied: bool,
    pub pii_fields_redacted: Option<Json<HashMap<String, i32>>>,
    pub status: String, // pending, delivered, failed
    pub error_message: Option<String>,
}

/// Request to dispatch a message from one agent to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aDispatchRequest {
    pub source_agent_id: String,
    pub target_agent_id: String,
    pub message: String,
    pub context: Option<serde_json::Value>,
    #[serde(default)]
    pub require_pii_redaction: bool,
    // Phase 4: Chain tracking
    #[serde(default)]
    pub chain_id: Option<String>,
    #[serde(default)]
    pub parent_dispatch_id: Option<String>,
}

/// Response from an A2A dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aDispatchResponse {
    pub dispatch_id: String,
    pub status: String,
    pub message: String,
    pub redaction_applied: bool,
    pub redacted_fields: Option<HashMap<String, i32>>,
    // Phase 4: Chain tracking
    pub chain_id: String,
    pub chain_step: i8,
}

/// PII redaction log entry (what was redacted)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct A2aRedactionLog {
    pub id: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub a2a_dispatch_id: String,
    pub original_text: String,
    pub redacted_text: String,
    pub pii_type: String,
    pub confidence_score: Option<f32>,
}

/// Phase 4: Chain registry - tracks multi-hop dispatch chains
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct A2aChainRegistry {
    pub id: String,
    pub initiated_by_tenant: String,
    pub initiated_by_agent: String,
    pub current_step: i8,
    pub max_steps: i8,
    pub status: String, // in_progress, complete, failed
    pub visited_agents: Option<Json<Vec<String>>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Phase 4: Chain status response - contains registry info + all dispatches in chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aChainStatus {
    pub chain_id: String,
    pub status: String,
    pub current_step: i8,
    pub max_steps: i8,
    pub initiated_by_tenant: String,
    pub initiated_by_agent: String,
    pub dispatches: Vec<A2aDispatchAudit>,
}
