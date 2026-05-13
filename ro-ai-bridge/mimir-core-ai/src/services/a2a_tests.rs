#[cfg(test)]
mod tests {
    use super::super::*;
    use sqlx::mysql::MySqlPoolOptions;
    use std::time::Duration;

    async fn setup_test_db() -> MySqlPool {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:3306/test_mimir".to_string());

        MySqlPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&database_url)
            .await
            .expect("Failed to connect to test DB")
    }

    async fn cleanup_test_data(db: &MySqlPool) {
        sqlx::query("DELETE FROM a2a_redaction_log").execute(db).await.ok();
        sqlx::query("DELETE FROM a2a_dispatch_audit").execute(db).await.ok();
        sqlx::query("DELETE FROM a2a_routing_rules").execute(db).await.ok();
    }

    // ─── P0: A2aService CRUD Tests ───────────────────────────────────────

    #[tokio::test]
    async fn test_create_routing_rule_success() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "asgard_medical".to_string(),
            source_agent_id: "medical_review_agent".to_string(),
            target_tenant_id: "asgard_insurance".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            condition_json: None,
            description: Some("Medical to Insurance".to_string()),
        };

        let result = service.create_routing_rule(req).await;
        assert!(result.is_ok(), "Should create routing rule");

        let rule = result.unwrap();
        assert_eq!(rule.source_tenant_id, "asgard_medical");
        assert_eq!(rule.target_agent_id, "underwriting_agent");
        assert!(rule.enabled);

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_create_routing_rule_duplicate_fails() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "asgard_medical".to_string(),
            source_agent_id: "medical_review_agent".to_string(),
            target_tenant_id: "asgard_insurance".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            condition_json: None,
            description: None,
        };

        // First insert should succeed
        let first = service.create_routing_rule(req.clone()).await;
        assert!(first.is_ok());

        // Second insert of same route should fail (UNIQUE constraint)
        let second = service.create_routing_rule(req).await;
        assert!(second.is_err(), "Should reject duplicate routing rule");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_find_routing_rule_exists() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        // Create a rule first
        let req = CreateA2aRoutingRuleRequest {
            source_tenant_id: "asgard_medical".to_string(),
            source_agent_id: "medical_review_agent".to_string(),
            target_tenant_id: "asgard_insurance".to_string(),
            target_agent_id: "underwriting_agent".to_string(),
            condition_json: None,
            description: None,
        };
        service.create_routing_rule(req).await.unwrap();

        // Now find it
        let result = service
            .find_routing_rule(
                "asgard_medical",
                "medical_review_agent",
                "asgard_insurance",
                "underwriting_agent",
            )
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some(), "Should find the routing rule");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_find_routing_rule_not_found() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let result = service
            .find_routing_rule(
                "asgard_medical",
                "nonexistent_agent",
                "asgard_insurance",
                "underwriting_agent",
            )
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none(), "Should return None for nonexistent rule");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_find_routing_rule_disabled_ignored() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;

        // Insert a disabled rule directly
        sqlx::query(
            "INSERT INTO a2a_routing_rules
             (id, source_tenant_id, source_agent_id, target_tenant_id, target_agent_id, enabled)
             VALUES (?, ?, ?, ?, ?, 0)"
        )
        .bind("test_rule_123")
        .bind("asgard_medical")
        .bind("medical_review_agent")
        .bind("asgard_insurance")
        .bind("underwriting_agent")
        .execute(&db)
        .await
        .unwrap();

        let service = A2aService::new(db.clone());
        let result = service
            .find_routing_rule(
                "asgard_medical",
                "medical_review_agent",
                "asgard_insurance",
                "underwriting_agent",
            )
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none(), "Should ignore disabled rules");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_list_routing_rules_cross_tenant() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        // Create rules for both directions
        service
            .create_routing_rule(CreateA2aRoutingRuleRequest {
                source_tenant_id: "asgard_medical".to_string(),
                source_agent_id: "medical_review_agent".to_string(),
                target_tenant_id: "asgard_insurance".to_string(),
                target_agent_id: "underwriting_agent".to_string(),
                condition_json: None,
                description: None,
            })
            .await
            .unwrap();

        service
            .create_routing_rule(CreateA2aRoutingRuleRequest {
                source_tenant_id: "asgard_insurance".to_string(),
                source_agent_id: "underwriting_agent".to_string(),
                target_tenant_id: "asgard_medical".to_string(),
                target_agent_id: "claims_processor".to_string(),
                condition_json: None,
                description: None,
            })
            .await
            .unwrap();

        // List for medical tenant should see both (as source and target)
        let rules = service.list_routing_rules("asgard_medical").await.unwrap();
        assert_eq!(rules.len(), 2, "Should see rules where tenant is source or target");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_delete_routing_rule_success() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let rule = service
            .create_routing_rule(CreateA2aRoutingRuleRequest {
                source_tenant_id: "asgard_medical".to_string(),
                source_agent_id: "medical_review_agent".to_string(),
                target_tenant_id: "asgard_insurance".to_string(),
                target_agent_id: "underwriting_agent".to_string(),
                condition_json: None,
                description: None,
            })
            .await
            .unwrap();

        let delete_result = service.delete_routing_rule(&rule.id).await;
        assert!(delete_result.is_ok(), "Should delete routing rule");

        // Verify it's gone
        let find_result = service
            .find_routing_rule(
                "asgard_medical",
                "medical_review_agent",
                "asgard_insurance",
                "underwriting_agent",
            )
            .await
            .unwrap();

        assert!(find_result.is_none(), "Deleted rule should not be found");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_delete_routing_rule_not_found() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let result = service.delete_routing_rule("nonexistent_id_123").await;
        assert!(result.is_err(), "Should fail to delete nonexistent rule");

        cleanup_test_data(&db).await;
    }

    // ─── P1: Dispatch & Redaction Logging ────────────────────────────────

    #[tokio::test]
    async fn test_log_redaction_creates_entry() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let dispatch_id = uuid::Uuid::new_v4().to_string();

        let result = service
            .log_redaction(
                &dispatch_id,
                "123-4567-89012",
                "[REDACTED_THAI_ID]",
                "thai_national_id",
                0.99,
            )
            .await;

        assert!(result.is_ok(), "Should log redaction");

        cleanup_test_data(&db).await;
    }

    #[tokio::test]
    async fn test_get_redaction_summary_aggregates() {
        let db = setup_test_db().await;
        cleanup_test_data(&db).await;
        let service = A2aService::new(db.clone());

        let dispatch_id = uuid::Uuid::new_v4().to_string();

        // Log multiple redactions of same type
        service
            .log_redaction(&dispatch_id, "123-4567-89012", "[REDACTED]", "thai_national_id", 0.99)
            .await
            .unwrap();
        service
            .log_redaction(&dispatch_id, "098-7654-32109", "[REDACTED]", "thai_national_id", 0.98)
            .await
            .unwrap();
        service
            .log_redaction(&dispatch_id, "08-XXXX-XXXX", "[REDACTED]", "phone_number", 0.95)
            .await
            .unwrap();

        let summary = service.get_redaction_summary(&dispatch_id).await.unwrap();

        assert_eq!(summary.get("thai_national_id"), Some(&2), "Should count 2 Thai IDs");
        assert_eq!(summary.get("phone_number"), Some(&1), "Should count 1 phone");

        cleanup_test_data(&db).await;
    }
}
