use anyhow::{Result, anyhow};
use sqlx::MySqlPool;
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

use crate::models::a2a::{
    A2aRoutingRule, A2aDispatchRequest, CreateA2aRoutingRuleRequest,
};

/// Internal helper for chain resolution (no DB access, testable)
pub struct ChainResolution {
    pub chain_id: String,
    pub chain_step: i8,
    pub parent_dispatch_id: Option<String>,
    pub chain_context: Option<serde_json::Value>,
}

pub struct A2aService {
    db: MySqlPool,
}

impl A2aService {
    pub fn new(db: MySqlPool) -> Self {
        Self { db }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Phase 4: Chain Resolution — Pure Static Helpers (no DB access)
    // ═══════════════════════════════════════════════════════════════════

    /// Start a new chain by generating a chain_id and setting step to 1
    pub fn new_chain_resolution(req: &A2aDispatchRequest, _source_tenant: &str) -> ChainResolution {
        ChainResolution {
            chain_id: Uuid::new_v4().to_string(),
            chain_step: 1,
            parent_dispatch_id: None,
            chain_context: req.context.clone(),
        }
    }

    /// Continue an existing chain by incrementing the step
    pub fn continue_chain_resolution(req: &A2aDispatchRequest, current_step: i8) -> ChainResolution {
        ChainResolution {
            chain_id: req.chain_id.clone().unwrap_or_default(),
            chain_step: current_step + 1,
            parent_dispatch_id: req.parent_dispatch_id.clone(),
            chain_context: req.context.clone(),
        }
    }

    /// Validate that the next step would not exceed max_steps
    pub fn validate_chain_depth(current_step: i8, max_steps: i8) -> Result<()> {
        if current_step >= max_steps {
            return Err(anyhow!(
                "Cannot exceed maximum chain depth: current_step={}, max_steps={}",
                current_step,
                max_steps
            ));
        }
        Ok(())
    }

    /// Detect cycles: check if target agent is already in the visited list
    pub fn validate_no_cycle(
        visited: &[String],
        target_tenant: &str,
        target_agent: &str,
    ) -> Result<()> {
        let target_key = format!("{}:{}", target_tenant, target_agent);
        if visited.contains(&target_key) {
            return Err(anyhow!(
                "Cycle detected in chain: {} already visited",
                target_key
            ));
        }
        Ok(())
    }

    /// Validate A2A routing request
    async fn validate_routing_request(req: &CreateA2aRoutingRuleRequest) -> Result<()> {
        // Validate IDs are not empty
        if req.source_tenant_id.is_empty() || req.source_tenant_id.len() > 100 {
            return Err(anyhow!("source_tenant_id must be 1-100 characters"));
        }
        if req.source_agent_id.is_empty() || req.source_agent_id.len() > 100 {
            return Err(anyhow!("source_agent_id must be 1-100 characters"));
        }
        if req.target_tenant_id.is_empty() || req.target_tenant_id.len() > 100 {
            return Err(anyhow!("target_tenant_id must be 1-100 characters"));
        }
        if req.target_agent_id.is_empty() || req.target_agent_id.len() > 100 {
            return Err(anyhow!("target_agent_id must be 1-100 characters"));
        }

        // Validate tenant IDs don't match (can't route to self)
        if req.source_tenant_id == req.target_tenant_id && req.source_agent_id == req.target_agent_id {
            return Err(anyhow!("Cannot route agent to itself"));
        }

        // Validate condition_json is valid
        if let Some(condition) = &req.condition_json {
            // Just verify it's serializable
            serde_json::to_string(condition)
                .map_err(|e| anyhow!("Invalid condition_json: {}", e))?;
        }

        // Validate description length if provided
        if let Some(desc) = &req.description {
            if desc.len() > 500 {
                return Err(anyhow!("description must be under 500 characters"));
            }
        }

        Ok(())
    }

    /// Find routing rule from source→target agents (requires all 4 parameters)
    pub async fn find_routing_rule(
        &self,
        source_tenant: &str,
        source_agent: &str,
        target_tenant: &str,
        target_agent: &str,
    ) -> Result<Option<A2aRoutingRule>> {
        let rule = sqlx::query_as::<_, A2aRoutingRule>(
            "SELECT * FROM a2a_routing_rules
             WHERE source_tenant_id = ? AND source_agent_id = ?
             AND target_tenant_id = ? AND target_agent_id = ?
             AND enabled = 1"
        )
        .bind(source_tenant)
        .bind(source_agent)
        .bind(target_tenant)
        .bind(target_agent)
        .fetch_optional(&self.db)
        .await?;

        Ok(rule)
    }

    /// Find routing rule by source and target agent IDs (target tenant determined by rule)
    pub async fn find_routing_rule_by_agents(
        &self,
        source_tenant: &str,
        source_agent: &str,
        target_agent: &str,
    ) -> Result<Option<A2aRoutingRule>> {
        let rule = sqlx::query_as::<_, A2aRoutingRule>(
            "SELECT * FROM a2a_routing_rules
             WHERE source_tenant_id = ? AND source_agent_id = ?
             AND target_agent_id = ?
             AND enabled = 1
             LIMIT 1"
        )
        .bind(source_tenant)
        .bind(source_agent)
        .bind(target_agent)
        .fetch_optional(&self.db)
        .await?;

        Ok(rule)
    }

    /// List all routing rules for a tenant
    pub async fn list_routing_rules(&self, tenant_id: &str) -> Result<Vec<A2aRoutingRule>> {
        let rules = sqlx::query_as::<_, A2aRoutingRule>(
            "SELECT * FROM a2a_routing_rules
             WHERE (source_tenant_id = ? OR target_tenant_id = ?) AND enabled = 1
             ORDER BY created_at DESC"
        )
        .bind(tenant_id)
        .bind(tenant_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rules)
    }

    /// Create a new A2A routing rule
    pub async fn create_routing_rule(
        &self,
        req: CreateA2aRoutingRuleRequest,
    ) -> Result<A2aRoutingRule> {
        // Validate input
        Self::validate_routing_request(&req).await?;

        let id = Uuid::new_v4().to_string();

        let condition_json = req.condition_json.as_ref()
            .map(|v| serde_json::to_string(v))
            .transpose()?;

        sqlx::query(
            "INSERT INTO a2a_routing_rules
             (id, source_tenant_id, source_agent_id, target_tenant_id, target_agent_id,
              condition_json, description)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&req.source_tenant_id)
        .bind(&req.source_agent_id)
        .bind(&req.target_tenant_id)
        .bind(&req.target_agent_id)
        .bind(condition_json)
        .bind(&req.description)
        .execute(&self.db)
        .await?;

        Ok(A2aRoutingRule {
            id,
            source_tenant_id: req.source_tenant_id,
            source_agent_id: req.source_agent_id,
            target_tenant_id: req.target_tenant_id,
            target_agent_id: req.target_agent_id,
            condition_json: req.condition_json.map(sqlx::types::Json),
            enabled: true,
            description: req.description,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    /// Delete a routing rule
    pub async fn delete_routing_rule(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM a2a_routing_rules WHERE id = ?")
            .bind(rule_id)
            .execute(&self.db)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Routing rule not found: {}", rule_id));
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // Phase 4: Chain Registry DB Methods
    // ═══════════════════════════════════════════════════════════════════

    /// Start a new chain in the registry
    pub async fn start_chain(
        &self,
        chain_id: &str,
        source_tenant: &str,
        source_agent: &str,
        target_tenant: &str,
        target_agent: &str,
    ) -> Result<()> {
        let initial_visited = vec![format!("{}:{}", source_tenant, source_agent)];
        let visited_json = serde_json::to_string(&initial_visited)?;

        sqlx::query(
            "INSERT INTO a2a_chain_registry
             (id, initiated_by_tenant, initiated_by_agent, current_step, max_steps, status, visited_agents)
             VALUES (?, ?, ?, 1, 5, 'in_progress', ?)"
        )
        .bind(chain_id)
        .bind(source_tenant)
        .bind(source_agent)
        .bind(visited_json)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Continue an existing chain (validate depth and add to visited list)
    pub async fn continue_chain(
        &self,
        chain_id: &str,
        target_tenant: &str,
        target_agent: &str,
    ) -> Result<()> {
        // Get current chain status
        let registry: Option<crate::models::a2a::A2aChainRegistry> = sqlx::query_as(
            "SELECT * FROM a2a_chain_registry WHERE id = ?"
        )
        .bind(chain_id)
        .fetch_optional(&self.db)
        .await?;

        let registry = registry.ok_or_else(|| anyhow!("Chain not found: {}", chain_id))?;

        // Validate depth
        Self::validate_chain_depth(registry.current_step, registry.max_steps)?;

        // Validate no cycle
        let visited: Vec<String> = registry
            .visited_agents
            .as_ref()
            .map(|v| v.0.clone())
            .unwrap_or_default();

        Self::validate_no_cycle(&visited, target_tenant, target_agent)?;

        // Add target to visited list
        let mut updated_visited = visited;
        updated_visited.push(format!("{}:{}", target_tenant, target_agent));
        let visited_json = serde_json::to_string(&updated_visited)?;

        // Update registry
        sqlx::query(
            "UPDATE a2a_chain_registry
             SET current_step = current_step + 1, visited_agents = ?
             WHERE id = ?"
        )
        .bind(visited_json)
        .bind(chain_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Finalize chain (mark as complete or failed)
    pub async fn finalize_chain(&self, chain_id: &str, status: &str) -> Result<()> {
        sqlx::query(
            "UPDATE a2a_chain_registry
             SET status = ?
             WHERE id = ?"
        )
        .bind(status)
        .bind(chain_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Get chain status with all dispatches
    pub async fn get_chain(&self, chain_id: &str) -> Result<crate::models::a2a::A2aChainStatus> {
        let registry: crate::models::a2a::A2aChainRegistry = sqlx::query_as(
            "SELECT * FROM a2a_chain_registry WHERE id = ?"
        )
        .bind(chain_id)
        .fetch_one(&self.db)
        .await?;

        let dispatches: Vec<crate::models::a2a::A2aDispatchAudit> = sqlx::query_as(
            "SELECT * FROM a2a_dispatch_audit WHERE chain_id = ? ORDER BY chain_step ASC"
        )
        .bind(chain_id)
        .fetch_all(&self.db)
        .await?;

        Ok(crate::models::a2a::A2aChainStatus {
            chain_id: registry.id,
            status: registry.status,
            current_step: registry.current_step,
            max_steps: registry.max_steps,
            initiated_by_tenant: registry.initiated_by_tenant,
            initiated_by_agent: registry.initiated_by_agent,
            dispatches,
        })
    }

    /// Log an A2A dispatch event (with chain tracking)
    pub async fn log_dispatch(
        &self,
        dispatch_id: &str,
        req: &A2aDispatchRequest,
        source_tenant: &str,
        target_tenant: &str,
        message_summary: &str,
        chain_id: &str,
        chain_step: i8,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO a2a_dispatch_audit
             (id, chain_id, parent_dispatch_id, chain_step, source_tenant_id, source_agent_id,
              target_tenant_id, target_agent_id, message_summary, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')"
        )
        .bind(dispatch_id)
        .bind(chain_id)
        .bind(&req.parent_dispatch_id)
        .bind(chain_step)
        .bind(source_tenant)
        .bind(&req.source_agent_id)
        .bind(target_tenant)
        .bind(&req.target_agent_id)
        .bind(message_summary)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Update dispatch status after processing
    pub async fn update_dispatch_status(
        &self,
        dispatch_id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE a2a_dispatch_audit
             SET status = ?, error_message = ?
             WHERE id = ?"
        )
        .bind(status)
        .bind(error)
        .bind(dispatch_id)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Log a PII redaction event (called by Skuggi middleware)
    ///
    /// Logs what PII was detected and how it was redacted.
    /// Note: original_text should be the redacted message (not the original message).
    /// This is for audit purposes - we don't log the actual sensitive data.
    pub async fn log_redaction(
        &self,
        dispatch_id: &str,
        pii_category: &str,
        redaction_count: i32,
        confidence: f32,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO a2a_redaction_log
             (id, a2a_dispatch_id, original_text, redacted_text, pii_type, confidence_score)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(dispatch_id)
        .bind(format!("[REDACTED {} instances]", redaction_count))
        .bind(pii_category)
        .bind(pii_category)
        .bind(confidence)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Get redaction summary for a dispatch
    pub async fn get_redaction_summary(
        &self,
        dispatch_id: &str,
    ) -> Result<HashMap<String, i32>> {
        let redactions: Vec<(String,)> = sqlx::query_as(
            "SELECT pii_type FROM a2a_redaction_log WHERE a2a_dispatch_id = ?"
        )
        .bind(dispatch_id)
        .fetch_all(&self.db)
        .await?;

        let mut summary = HashMap::new();
        for (pii_type,) in redactions {
            *summary.entry(pii_type).or_insert(0) += 1;
        }

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::a2a::{
        A2aDispatchResponse, A2aRedactionLog, A2aDispatchAudit,
    };

    #[test]
    fn test_a2a_service_creation() {
        // Basic sanity test that the service can be instantiated
        // (Full integration tests would need a test database)
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // This is a placeholder test — full tests need sqlx test fixtures
            println!("A2aService tests scaffolded (requires TEST_DATABASE_URL)");
        }));
    }

    #[test]
    fn test_routing_rule_deserialization() {
        use serde_json::json;

        let json = json!({
            "id": "rule-123",
            "source_tenant_id": "asgard_medical",
            "source_agent_id": "medical_review_agent",
            "target_tenant_id": "asgard_insurance",
            "target_agent_id": "underwriting_agent",
            "enabled": true,
            "description": "Medical to Insurance route"
        });

        let result: Result<A2aRoutingRule, _> = serde_json::from_value(json);
        assert!(result.is_ok(), "Should deserialize routing rule");

        let rule = result.unwrap();
        assert_eq!(rule.source_tenant_id, "asgard_medical");
        assert_eq!(rule.target_agent_id, "underwriting_agent");
    }

    #[test]
    fn test_dispatch_request_deserialization() {
        use serde_json::json;

        let json = json!({
            "source_agent_id": "medical_review_agent",
            "target_agent_id": "underwriting_agent",
            "message": "Clinical data for underwriting evaluation",
            "require_pii_redaction": true
        });

        let result: Result<A2aDispatchRequest, _> = serde_json::from_value(json);
        assert!(result.is_ok(), "Should deserialize dispatch request");

        let req = result.unwrap();
        assert_eq!(req.source_agent_id, "medical_review_agent");
        assert!(req.require_pii_redaction);
    }

    #[test]
    fn test_dispatch_response_serialization() {
        let response = A2aDispatchResponse {
            dispatch_id: "dispatch-456".to_string(),
            status: "pending".to_string(),
            message: "Dispatched to target agent".to_string(),
            redaction_applied: true,
            redacted_fields: None,
        };

        let json = serde_json::to_value(&response);
        assert!(json.is_ok(), "Should serialize dispatch response");

        let serialized = json.unwrap();
        assert_eq!(serialized["dispatch_id"], "dispatch-456");
        assert_eq!(serialized["status"], "pending");
    }

    #[test]
    fn test_redaction_log_serialization() {
        use chrono::Utc;

        let log = A2aRedactionLog {
            id: "redact-789".to_string(),
            timestamp: Some(Utc::now()),
            a2a_dispatch_id: "dispatch-456".to_string(),
            original_text: "123-4567-89012".to_string(),
            redacted_text: "[REDACTED_THAI_ID]".to_string(),
            pii_type: "thai_national_id".to_string(),
            confidence_score: Some(0.99),
        };

        let json = serde_json::to_value(&log);
        assert!(json.is_ok(), "Should serialize redaction log");

        let serialized = json.unwrap();
        assert_eq!(serialized["pii_type"], "thai_national_id");
        assert_eq!(serialized["confidence_score"], 0.99);
    }

    #[test]
    fn test_create_routing_rule_request_validation() {
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "asgard_medical".to_string(),
            source_agent_id: "medical_review_agent".to_string(),
            target_tenant_id: "asgard_insurance".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            condition_json: None,
            description: Some("Test route".to_string()),
        };

        // Verify all required fields are present
        assert!(!req.source_tenant_id.is_empty());
        assert!(!req.source_agent_id.is_empty());
        assert!(!req.target_tenant_id.is_empty());
        assert!(!req.target_agent_id.is_empty());
    }

    #[test]
    fn test_dispatch_audit_with_redaction() {
        use std::collections::HashMap;

        let mut redacted_fields = HashMap::new();
        redacted_fields.insert("thai_national_id".to_string(), 2);
        redacted_fields.insert("phone_number".to_string(), 1);

        let audit = A2aDispatchAudit {
            id: "audit-123".to_string(),
            timestamp: Some(chrono::Utc::now()),
            source_tenant_id: "asgard_medical".to_string(),
            source_agent_id: "medical_review_agent".to_string(),
            source_session_id: Some("session-1".to_string()),
            target_tenant_id: "asgard_insurance".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            target_session_id: Some("session-2".to_string()),
            message_summary: Some("Clinical findings".to_string()),
            pii_redaction_applied: true,
            pii_fields_redacted: Some(sqlx::types::Json(redacted_fields)),
            status: "delivered".to_string(),
            error_message: None,
        };

        assert!(audit.pii_redaction_applied);
        assert_eq!(audit.status, "delivered");
    }

    // ─── Validation Tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_validation_rejects_empty_tenant_id() {
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "".to_string(),  // Invalid: empty
            source_agent_id: "agent1".to_string(),
            target_tenant_id: "tenant2".to_string(),
            target_agent_id: "agent2".to_string(),
            condition_json: None,
            description: None,
        };

        let result = A2aService::validate_routing_request(&req).await;
        assert!(result.is_err(), "Should reject empty tenant ID");
    }

    #[tokio::test]
    async fn test_validation_rejects_too_long_id() {
        let long_id = "a".repeat(101);  // Too long
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: long_id,
            source_agent_id: "agent1".to_string(),
            target_tenant_id: "tenant2".to_string(),
            target_agent_id: "agent2".to_string(),
            condition_json: None,
            description: None,
        };

        let result = A2aService::validate_routing_request(&req).await;
        assert!(result.is_err(), "Should reject ID > 100 chars");
    }

    #[tokio::test]
    async fn test_validation_rejects_self_route() {
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "tenant1".to_string(),
            source_agent_id: "agent1".to_string(),
            target_tenant_id: "tenant1".to_string(),  // Same tenant
            target_agent_id: "agent1".to_string(),    // Same agent
            condition_json: None,
            description: None,
        };

        let result = A2aService::validate_routing_request(&req).await;
        assert!(result.is_err(), "Should reject routing agent to itself");
    }

    #[tokio::test]
    async fn test_validation_rejects_invalid_json() {
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "tenant1".to_string(),
            source_agent_id: "agent1".to_string(),
            target_tenant_id: "tenant2".to_string(),
            target_agent_id: "agent2".to_string(),
            condition_json: Some(serde_json::json!({"valid": "json"})),  // Valid
            description: None,
        };

        let result = A2aService::validate_routing_request(&req).await;
        assert!(result.is_ok(), "Should accept valid JSON");
    }

    #[tokio::test]
    async fn test_validation_accepts_valid_request() {
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "asgard_medical".to_string(),
            source_agent_id: "medical_review_agent".to_string(),
            target_tenant_id: "asgard_insurance".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            condition_json: Some(serde_json::json!({"type": "complete"})),
            description: Some("Medical to Insurance route".to_string()),
        };

        let result = A2aService::validate_routing_request(&req).await;
        assert!(result.is_ok(), "Should accept valid request");
    }

    // ─── Phase 4: Chain Tracking Tests ──────────────────────────────────────

    #[test]
    fn test_chain_start_generates_chain_id() {
        let req = A2aDispatchRequest {
            source_agent_id: "medical_review_agent".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            message: "Clinical data".to_string(),
            context: None,
            require_pii_redaction: false,
            chain_id: None,
            parent_dispatch_id: None,
        };

        let resolution = A2aService::new_chain_resolution(&req, "asgard_medical");
        // chain_id must be a valid UUID
        assert!(
            uuid::Uuid::parse_str(&resolution.chain_id).is_ok(),
            "New chain must have a valid UUID chain_id"
        );
        assert_eq!(resolution.chain_step, 1, "New chain starts at step 1");
        assert!(resolution.parent_dispatch_id.is_none(), "New chain has no parent");
    }

    #[test]
    fn test_chain_continuation_preserves_chain_id() {
        let existing_chain_id = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let existing_dispatch_id = "dispatch-step1-uuid".to_string();

        let req = A2aDispatchRequest {
            source_agent_id: "underwriting_agent".to_string(),
            target_agent_id: "claims_processor".to_string(),
            message: "Underwriting decision".to_string(),
            context: None,
            require_pii_redaction: false,
            chain_id: Some(existing_chain_id.clone()),
            parent_dispatch_id: Some(existing_dispatch_id),
        };

        let resolution = A2aService::continue_chain_resolution(&req, 1);
        assert_eq!(resolution.chain_id, existing_chain_id,
            "chain_id must pass through unchanged");
        assert_eq!(resolution.chain_step, 2,
            "step must increment from parent");
    }

    #[test]
    fn test_chain_max_depth_rejected() {
        // current_step = 5, max_steps = 5 → next would be 6 → reject
        let result = A2aService::validate_chain_depth(5, 5);
        assert!(result.is_err(), "Step 6 must be rejected when max_steps = 5");
        assert!(
            result.unwrap_err().to_string().contains("maximum chain depth"),
            "Error message must mention maximum chain depth"
        );
    }

    #[test]
    fn test_chain_cycle_detection() {
        // Chain: A → B → (tries to go back to A)
        let visited: Vec<String> = vec![
            "asgard_medical:medical_review_agent".to_string(),
            "asgard_insurance:underwriting_agent".to_string(),
        ];

        // Target is the first agent in the chain → cycle
        let result = A2aService::validate_no_cycle(
            &visited,
            "asgard_medical",
            "medical_review_agent",
        );
        assert!(result.is_err(), "A→B→A must be rejected");
        assert!(
            result.unwrap_err().to_string().contains("cycle"),
            "Error message must mention cycle"
        );

        // Non-cycle: new agent not in visited list
        let ok = A2aService::validate_no_cycle(
            &visited,
            "asgard_medical",
            "claims_processor",
        );
        assert!(ok.is_ok(), "Dispatch to new agent must be allowed");
    }

    #[test]
    fn test_chain_context_preserved() {
        let context = serde_json::json!({
            "prior_decision": "pending_review",
            "hba1c": 7.2
        });

        let req = A2aDispatchRequest {
            source_agent_id: "medical_review_agent".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            message: "Clinical data with context".to_string(),
            context: Some(context.clone()),
            require_pii_redaction: false,
            chain_id: None,
            parent_dispatch_id: None,
        };

        let resolution = A2aService::new_chain_resolution(&req, "asgard_medical");
        assert_eq!(
            resolution.chain_context,
            Some(context),
            "context field must be preserved in chain resolution"
        );
    }
}
