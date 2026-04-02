//! E2E Service-Level Integration Tests (Issue #154)
//!
//! End-to-end validation of Sprint 14 service function chains.
//! No database or external services required — tests pure function pipelines.

#[cfg(test)]
mod tests {
    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S01: Vault — Config → Build Path → Parse Response flow
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_vault_config_to_secret_resolution() {
        use crate::services::vault::*;

        // Step 1: Construct config
        let config = VaultConfig {
            addr: "https://vault.prod.example.com".to_string(),
            token: "s.mySecretProductionToken".to_string(),
            mount: "secret".to_string(),
            path: "mimir/production".to_string(),
        };

        // Step 2: Build URL path
        let url = build_secret_path(&config);
        assert_eq!(
            url,
            "https://vault.prod.example.com/v1/secret/data/mimir/production"
        );

        // Step 3: Simulate Vault response and parse
        let vault_response = serde_json::json!({
            "data": {
                "data": {
                    "gemini_api_key": "AIza-production-key-xxx",
                    "jwt_secret": "prod-jwt-secret-yyy",
                    "s3_access_key": "AKIA-prod-zzz"
                },
                "metadata": { "version": 7 }
            }
        });

        let api_key = parse_vault_response(&vault_response, "gemini_api_key").unwrap();
        assert_eq!(api_key, "AIza-production-key-xxx");

        let jwt = parse_vault_response(&vault_response, "jwt_secret").unwrap();
        assert_eq!(jwt, "prod-jwt-secret-yyy");

        // Step 4: Verify version extraction
        let version = parse_vault_version(&vault_response);
        assert_eq!(version, Some(7));

        // Step 5: Verify masking for logs
        let masked = mask_secret(&api_key);
        assert_eq!(masked, "AIza***xx");

        // Step 6: Verify key mapping
        assert_eq!(map_config_to_vault_key("GEMINI_API_KEY"), "gemini_api_key");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S02: DB Connector — Parse → Validate → Schema → Markdown flow
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_db_connector_full_pipeline() {
        use crate::services::db_connector::*;

        // Step 1: Parse connection string
        let info = parse_connection_string(
            "mysql://analyst:secret@db.company.com:3306/analytics_db?ssl=true",
        )
        .unwrap();
        assert_eq!(info.db_type, DbType::Mysql);
        assert_eq!(info.host, "db.company.com");
        assert_eq!(info.port, Some(3306));
        assert_eq!(info.database, "analytics_db");
        assert_eq!(info.user, Some("analyst".to_string()));

        // Step 2: Validate safe query
        assert!(validate_query("SELECT id, name, revenue FROM orders WHERE year = 2024").is_ok());

        // Step 3: Reject dangerous query
        assert!(validate_query("SELECT 1; DROP TABLE orders").is_err());
        assert!(validate_query("DELETE FROM orders WHERE id = 1").is_err());

        // Step 4: Build schema queries
        let (tables_q, cols_q) = build_schema_query(&info.db_type);
        assert!(tables_q.contains("information_schema.TABLES"));
        assert!(cols_q.contains("{TABLE_NAME}"));

        // Step 5: Convert data to markdown
        let columns = vec![
            "Order".to_string(),
            "Customer".to_string(),
            "Amount".to_string(),
        ];
        let rows = vec![
            vec![
                "001".to_string(),
                "ACME Corp".to_string(),
                "5000".to_string(),
            ],
            vec!["002".to_string(), "Globex".to_string(), "3200".to_string()],
        ];
        let md = rows_to_markdown(&columns, &rows);
        assert!(md.contains("| Order | Customer | Amount |"));
        assert!(md.contains("| 001 | ACME Corp | 5000 |"));
        assert!(md.contains("| 002 | Globex | 3200 |"));

        // Step 6: Validate connection config
        let req = TestConnectionRequest {
            name: "Analytics DB".to_string(),
            db_type: DbType::Mysql,
            connection_string: "mysql://analyst:secret@db.company.com:3306/analytics_db"
                .to_string(),
        };
        assert!(validate_connection_config(&req).is_ok());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S03: Feedback — Request Build → GitHub Issue Body flow
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_feedback_to_github_issue_body() {
        use crate::services::feedback::*;

        // Step 1: Create a feedback request
        let req = CreateFeedbackRequest {
            report_type: "bug".to_string(),
            title: "OCR fails on Thai text".to_string(),
            description: Some(
                "When uploading a Thai document, OCR returns garbled text.".to_string(),
            ),
            priority: Some("high".to_string()),
            page_url: Some("/dashboard/sources/42".to_string()),
            browser_info: Some(serde_json::json!({"browser": "Chrome 120", "os": "macOS 14.3"})),
            client_logs: Some(serde_json::json!(["Error: OCR timeout after 30s"])),
        };

        // Step 2: Build GitHub issue body
        let system_logs =
            "Last 5 LLM calls:\n- gemini-2.5-flash: 1200 tokens\n- Failed source id=42: timeout";
        let body =
            build_github_issue_body(&req, Some(system_logs), 42, "tenant-abc", Some("user-123"));

        // Step 3: Verify issue body contains essential info
        assert!(body.contains("bug")); // report_type
        assert!(body.contains("High")); // priority badge
        assert!(body.contains("tenant-abc")); // tenant
        assert!(body.contains("user-123")); // user
        assert!(body.contains("Thai document")); // description content
        assert!(body.contains("System Logs")); // system logs section
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S04: Cron — State Creation and Tracking
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_cron_state_lifecycle() {
        use crate::services::cron::CronState;

        // Step 1: Create new cron state
        let state = CronState::new();

        // Step 2: Verify initial state
        // CronState tracks tick_count, last_tick_at, sources_refreshed
        // Just verify it can be created without panicking
        assert!(format!("{:?}", state).contains("CronState"));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S05: OCR — MIME Detection → Capability Check → Vision Request flow
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_ocr_pipeline_pure_functions() {
        use crate::services::ocr::*;

        // Step 1: Detect MIME types
        assert_eq!(detect_mime_type("invoice.png"), Some("image/png"));
        assert_eq!(detect_mime_type("scan.pdf"), Some("application/pdf"));
        assert_eq!(detect_mime_type("photo.jpg"), Some("image/jpeg"));
        assert_eq!(detect_mime_type("data.csv"), None);

        // Step 2: Check OCR capability
        assert!(is_ocr_capable("invoice.png"));
        assert!(is_ocr_capable("scan.pdf"));
        assert!(!is_ocr_capable("data.csv"));

        // Step 3: Build vision request
        let dummy_image = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG magic bytes
        let request = build_vision_request(
            &dummy_image,
            "image/jpeg",
            "gemini-2.5-flash",
            "Extract all text from this image",
        );

        // Verify request structure
        let messages = request.get("messages").unwrap().as_array().unwrap();
        assert!(!messages.is_empty());
        let model = request.get("model").unwrap().as_str().unwrap();
        assert_eq!(model, "gemini-2.5-flash");

        // Step 4: Test scanned PDF detection
        assert!(is_likely_scanned_pdf("ab", 100_000)); // tiny text + big file = scanned
        assert!(!is_likely_scanned_pdf(
            "This is a long document with lots of text content.",
            1_000
        ));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S06: Extraction — MIME Detection → Extract → Markdown flow
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_extraction_csv_to_markdown() {
        use crate::services::extraction::*;

        // Step 1: Load fixture
        let csv_data = include_bytes!("../../tests/fixtures/sample.csv");

        // Step 2: Extract via router (detects .csv → Markdown table)
        let result = extract("tabular", "uploads/tenant-1/sample.csv", csv_data);
        assert!(result.is_ok());

        let markdown = result.unwrap();
        // Step 3: Verify markdown table output
        assert!(markdown.contains("| id"));
        assert!(markdown.contains("Alice Johnson"));
        assert!(markdown.contains("Engineering"));
        assert!(markdown.contains("Marketing"));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S07: Extraction — HTML → Markdown flow
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_extraction_html_to_markdown() {
        use crate::services::extraction::*;

        // Step 1: Load fixture
        let html_data = include_bytes!("../../tests/fixtures/sample.html");

        // Step 2: Extract via router
        let result = extract("web", "pages/tenant-1/index.html", html_data);
        assert!(result.is_ok());

        let markdown = result.unwrap();
        // Step 3: Verify content preservation
        assert!(markdown.contains("Mimir Knowledge Base") || markdown.contains("Welcome"));
        assert!(markdown.contains("Document ingestion") || markdown.contains("ingestion"));
        assert!(markdown.contains("Vector search") || markdown.contains("Qdrant"));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // E2E-S08: Multi-service — DB Connector + Vault key mapping chain
    // ═══════════════════════════════════════════════════════════════════════════
    #[test]
    fn e2e_multi_service_integration() {
        use crate::services::db_connector::*;
        use crate::services::vault::*;

        // Scenario: Resolve DB connection string from Vault, then use it

        // Step 1: Simulate Vault response with connection info
        let vault_response = serde_json::json!({
            "data": {
                "data": {
                    "external_db_mysql": "mysql://reader:pass@analytics.internal:3306/warehouse",
                    "external_db_postgres": "postgres://bi:pass@reports.internal:5432/dashboard"
                },
                "metadata": { "version": 2 }
            }
        });

        // Step 2: Extract MySQL connection from Vault
        let mysql_conn = parse_vault_response(&vault_response, "external_db_mysql").unwrap();
        assert_eq!(
            mysql_conn,
            "mysql://reader:pass@analytics.internal:3306/warehouse"
        );

        // Step 3: Parse the extracted connection string with DB connector
        let info = parse_connection_string(&mysql_conn).unwrap();
        assert_eq!(info.db_type, DbType::Mysql);
        assert_eq!(info.host, "analytics.internal");
        assert_eq!(info.database, "warehouse");

        // Step 4: Validate the connection config
        let req = TestConnectionRequest {
            name: "Warehouse DB".to_string(),
            db_type: DbType::Mysql,
            connection_string: mysql_conn.clone(),
        };
        assert!(validate_connection_config(&req).is_ok());

        // Step 5: Also test PostgreSQL path
        let pg_conn = parse_vault_response(&vault_response, "external_db_postgres").unwrap();
        let pg_info = parse_connection_string(&pg_conn).unwrap();
        assert_eq!(pg_info.db_type, DbType::Postgres);
        assert_eq!(pg_info.host, "reports.internal");

        // Step 6: Verify masking for connection string logging
        let masked = mask_secret(&mysql_conn);
        assert!(masked.starts_with("mysq"));
        assert!(masked.contains("***"));
    }
}
