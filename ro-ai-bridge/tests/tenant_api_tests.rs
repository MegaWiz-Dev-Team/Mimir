/// TDD Integration Tests for Mimir Tenant API
///
/// Tests the full CRUD lifecycle plus document ingestion and query.
/// Requires a running MariaDB instance (uses env DATABASE_URL).
///
/// ISO 29110 Reference: SI-04 Test Plan
/// Run: DATABASE_URL="mysql://..." cargo test --test tenant_api_tests

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    fn base_url() -> String {
        std::env::var("MIMIR_TEST_URL").unwrap_or_else(|_| "http://localhost:3002".to_string())
    }

    fn client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::new()
    }

    // ── Health ─────────────────────────────────────────

    #[test]
    fn t01_health_check() {
        let resp = client().get(format!("{}/health", base_url())).send().unwrap();
        assert_eq!(resp.status(), 200, "GET /health should return 200");
        let body: Value = resp.json().unwrap();
        assert_eq!(body["status"], "ok");
    }

    #[test]
    fn t02_healthz_alias() {
        let resp = client().get(format!("{}/healthz", base_url())).send().unwrap();
        assert_eq!(resp.status(), 200, "GET /healthz should return 200 (K8s probe)");
    }

    // ── Tenant CRUD ────────────────────────────────────

    #[test]
    fn t10_list_tenants() {
        let resp = client().get(format!("{}/api/v1/tenants", base_url())).send().unwrap();
        assert_eq!(resp.status(), 200);
        let body: Vec<Value> = resp.json().unwrap();
        assert!(!body.is_empty(), "Should have at least default_tenant");
        // Verify default_tenant exists
        assert!(body.iter().any(|t| t["id"] == "default_tenant"));
    }

    #[test]
    fn t11_create_tenant() {
        let resp = client()
            .post(format!("{}/api/v1/tenants", base_url()))
            .json(&json!({
                "id": "test_tdd_agent",
                "name": "TDD Test Agent",
                "service_type": "test-agent",
                "description": "Created by TDD test"
            }))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 201, "POST /api/v1/tenants should return 201 Created");

        let body: Value = resp.json().unwrap();
        assert_eq!(body["id"], "test_tdd_agent");
        assert_eq!(body["name"], "TDD Test Agent");
        assert_eq!(body["domain"], "test_tdd_agent.asgard.local");
        assert_eq!(body["service_type"], "test-agent");
    }

    #[test]
    fn t12_create_long_domain_tenant() {
        // Regression test for #251: domain VARCHAR(20) was too short
        let resp = client()
            .post(format!("{}/api/v1/tenants", base_url()))
            .json(&json!({
                "id": "heimdall_llm_gateway",
                "name": "Heimdall LLM Gateway Service",
                "service_type": "llm-gateway"
            }))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 201, "Long domain name should work (fixes #251)");
        let body: Value = resp.json().unwrap();
        assert_eq!(body["domain"], "heimdall_llm_gateway.asgard.local");
    }

    #[test]
    fn t13_get_tenant() {
        let resp = client()
            .get(format!("{}/api/v1/tenants/test_tdd_agent", base_url()))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().unwrap();
        assert_eq!(body["id"], "test_tdd_agent");
        assert_eq!(body["service_type"], "test-agent");
    }

    #[test]
    fn t14_get_nonexistent_tenant() {
        let resp = client()
            .get(format!("{}/api/v1/tenants/does_not_exist", base_url()))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 404, "Should return 404 for unknown tenant");
    }

    #[test]
    fn t15_create_duplicate_tenant() {
        let resp = client()
            .post(format!("{}/api/v1/tenants", base_url()))
            .json(&json!({"id": "muninn", "name": "Duplicate Muninn"}))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 409, "Duplicate tenant should return 409 Conflict");
    }

    // ── Document Ingestion ─────────────────────────────

    #[test]
    fn t20_ingest_document() {
        let resp = client()
            .post(format!("{}/api/v1/tenants/test_tdd_agent/ingest", base_url()))
            .json(&json!({
                "title": "TDD Agent README",
                "content": "# TDD Agent\n## Overview\nTest-driven agent\n## API\n### POST /test\n### GET /results",
                "source": "readme"
            }))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 200, "Ingest should return 200");
        let body: Value = resp.json().unwrap();
        assert!(body["document_id"].as_i64().unwrap() > 0);
        assert_eq!(body["status"], "indexed");
        assert!(body["tree_node_count"].as_i64().unwrap() >= 1, "Should have at least 1 tree node");
    }

    #[test]
    fn t21_list_documents() {
        let resp = client()
            .get(format!("{}/api/v1/tenants/test_tdd_agent/ingest/documents", base_url()))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: Vec<Value> = resp.json().unwrap();
        assert!(!body.is_empty(), "Should have at least 1 ingested document");
        assert_eq!(body[0]["title"], "TDD Agent README");
    }

    // ── Tenant Query ───────────────────────────────────

    #[test]
    fn t30_query_tenant() {
        let resp = client()
            .post(format!("{}/api/v1/tenants/test_tdd_agent/query", base_url()))
            .json(&json!({"question": "What API endpoints does TDD Agent have?"}))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().unwrap();
        assert!(body["answer"].as_str().is_some(), "Should have answer");
        assert!(body["sources"].is_array(), "Should have sources array");
    }

    #[test]
    fn t31_query_nonexistent_tenant() {
        let resp = client()
            .post(format!("{}/api/v1/tenants/ghost_tenant/query", base_url()))
            .json(&json!({"question": "anything"}))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 404, "Query unknown tenant → 404");
    }

    // ── Cleanup ────────────────────────────────────────

    #[test]
    fn t90_delete_tenant() {
        let resp = client()
            .delete(format!("{}/api/v1/tenants/test_tdd_agent", base_url()))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: Value = resp.json().unwrap();
        assert_eq!(body["deleted"], "test_tdd_agent");
    }

    #[test]
    fn t91_delete_long_domain_tenant() {
        let resp = client()
            .delete(format!("{}/api/v1/tenants/heimdall_llm_gateway", base_url()))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[test]
    fn t92_delete_nonexistent_tenant() {
        let resp = client()
            .delete(format!("{}/api/v1/tenants/does_not_exist", base_url()))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 404, "Delete unknown → 404");
    }
}
