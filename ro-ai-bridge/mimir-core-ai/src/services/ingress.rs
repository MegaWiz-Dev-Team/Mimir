use crate::models::sources::DataSource;
use crate::services::extraction;
use crate::services::sql_import;
use anyhow::{Context, Result};
use tracing::{error, info, warn};

/// Retry an async operation with exponential backoff.
///
/// - `max_retries`: number of retry attempts (3 = 1 initial + 3 retries)
/// - `base_delay_ms`: initial delay in milliseconds (doubles each retry)
pub async fn retry_with_backoff<F, Fut, T>(
    operation_name: &str,
    max_retries: u32,
    base_delay_ms: u64,
    f: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_err = None;
    for attempt in 0..=max_retries {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt < max_retries {
                    let delay = base_delay_ms * 2u64.pow(attempt);
                    warn!(
                        "{} failed (attempt {}/{}), retrying in {}ms: {}",
                        operation_name,
                        attempt + 1,
                        max_retries + 1,
                        delay,
                        e
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                } else {
                    error!(
                        "{} failed after {} attempts: {}",
                        operation_name,
                        max_retries + 1,
                        e
                    );
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap())
}

pub struct IngressManager;

impl IngressManager {
    /// Phase 3: Extract content from raw data downloaded from RustFS.
    ///
    /// Routes to the appropriate extraction function based on `source_type`
    /// and file extension (from `s3_key`). Returns Markdown content for `raw_markdown`.
    ///
    /// When `storage_mode = "sql"` and `source_type = "tabular"`, returns a
    /// summary string. Use `process_extraction_sql()` for the full SQL import result.
    pub fn process_extraction(source: &DataSource, data: &[u8]) -> Result<String> {
        let s3_key = source.s3_key.as_deref().unwrap_or("unknown");
        let storage_mode = source.storage_mode.as_deref().unwrap_or("markdown");

        info!(
            "Phase 3: Extracting source_id={}, type={}, s3_key={}, storage_mode={}, size={} bytes",
            source.id,
            source.source_type,
            s3_key,
            storage_mode,
            data.len()
        );

        // SQL mode branch for tabular data
        if storage_mode == "sql" && source.source_type == "tabular" {
            let result =
                sql_import::process_tabular_for_sql(s3_key, data, &source.tenant_id, source.id)?;
            return Ok(format!(
                "SQL Import: table={}, {} rows, {} batches",
                result.table_name,
                result.total_rows,
                result.insert_batches.len()
            ));
        }

        extraction::extract(&source.source_type, s3_key, data)
    }

    /// Phase 3 (SQL Mode): Extract and prepare SQL import for tabular data.
    ///
    /// Returns `SqlImportResult` with DDL and INSERT statements for execution.
    pub fn process_extraction_sql(
        source: &DataSource,
        data: &[u8],
    ) -> Result<sql_import::SqlImportResult> {
        let s3_key = source.s3_key.as_deref().unwrap_or("unknown");
        info!(
            "Phase 3 (SQL Mode): source_id={}, s3_key={}, size={} bytes",
            source.id,
            s3_key,
            data.len()
        );
        sql_import::process_tabular_for_sql(s3_key, data, &source.tenant_id, source.id)
    }

    /// Phase 2: Fetch raw content for Web/MCP sources.
    ///
    /// - Web: downloads HTML via reqwest
    /// - MCP: fetches JSON from MCP server (placeholder — requires McpClient integration)
    ///
    /// Returns raw bytes to be saved to RustFS.
    pub async fn process_fetch(source: &DataSource) -> Result<Vec<u8>> {
        info!(
            "Phase 2: Fetching source_id={}, type={}",
            source.id, source.source_type
        );

        match source.source_type.as_str() {
            "web" => Self::fetch_web(source).await,
            "mcp" => Self::fetch_mcp(source).await,
            _ => Err(anyhow::anyhow!(
                "Phase 2 fetch not applicable for source_type: {}",
                source.source_type
            )),
        }
    }

    /// Fetch raw HTML from a web URL.
    async fn fetch_web(source: &DataSource) -> Result<Vec<u8>> {
        let url = source
            .config_json
            .get("url")
            .and_then(|v| v.as_str())
            .context("Missing or invalid 'url' in config_json for web source")?;

        info!("Fetching HTML from URL: {}", url);

        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .context("Failed to send HTTP request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read response body")?;
        info!("Fetched {} bytes from {}", bytes.len(), url);

        Ok(bytes.to_vec())
    }

    /// Fetch data from an MCP server.
    ///
    /// Reads the `mcp_url` from `config_json`, connects to the MCP server,
    /// fetches all resources, and returns them as JSON bytes.
    async fn fetch_mcp(source: &DataSource) -> Result<Vec<u8>> {
        let mcp_url = source
            .config_json
            .get("mcp_url")
            .and_then(|v| v.as_str())
            .context("Missing 'mcp_url' in config_json for MCP source")?;

        info!("Connecting to MCP server: {}", mcp_url);

        let mut mcp_client = crate::services::mcp_client::McpClient::new(mcp_url);
        mcp_client
            .connect()
            .await
            .context("Failed to connect to MCP server")?;

        let resources = mcp_client
            .fetch_resources()
            .await
            .context("Failed to fetch MCP resources")?;

        // Serialize all resources to JSON bytes
        let json_bytes = serde_json::to_vec_pretty(&resources)
            .context("Failed to serialize MCP resources to JSON")?;

        info!(
            "Fetched {} MCP resources ({} bytes)",
            resources.len(),
            json_bytes.len()
        );
        Ok(json_bytes)
    }

    /// Process a source by fetching its data and running real extraction.
    ///
    /// - `web`: fetch HTML → extract to Markdown
    /// - `mcp`: fetch JSON → extract to Markdown
    /// - `document` / `tabular`: requires pre-downloaded data — use `process_source_with_data()`
    ///   or provide S3 bucket to download from.
    pub async fn process_source(source: &DataSource) -> Result<String> {
        info!(
            "Processing source: {} (type={})",
            source.name, source.source_type
        );
        match source.source_type.as_str() {
            "web" => {
                let data = Self::fetch_web(source).await?;
                extraction::extract_html_to_markdown(&data)
            }
            "mcp" => {
                let data = Self::fetch_mcp(source).await?;
                extraction::extract_mcp_json_to_markdown(&data)
            }
            "file" | "document" | "tabular" => {
                // For file-based sources, data must be provided via process_source_with_data()
                Err(anyhow::anyhow!(
                    "Source type '{}' requires file data — use process_source_with_data() or sync_source with S3 download",
                    source.source_type
                ))
            }
            _ => Err(anyhow::anyhow!(
                "Unsupported source type: {}",
                source.source_type
            )),
        }
    }

    /// Process a source with pre-downloaded file data (for upload & sync flows).
    ///
    /// Downloads are handled by the caller (sync_source downloads from S3,
    /// upload_file already has bytes in memory). This method runs the real
    /// extraction pipeline and returns Markdown content.
    pub fn process_source_with_data(source: &DataSource, data: &[u8]) -> Result<String> {
        if data.is_empty() {
            return Err(anyhow::anyhow!(
                "Empty file data for source '{}' (id={})",
                source.name,
                source.id
            ));
        }

        info!(
            "Processing source with data: {} (id={}, type={}, {} bytes)",
            source.name,
            source.id,
            source.source_type,
            data.len()
        );

        Self::process_extraction(source, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sources::DataSource;
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn test_ingress_process_web_source() {
        let source = DataSource {
            id: 1,
            tenant_id: "test_tenant".to_string(),
            name: "Test Web Source".to_string(),
            source_type: "web".to_string(),
            config_json: json!({"url": "http://invalid.url.that.does.not.exist.local"}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: None,
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Failed to send HTTP request")
                || err_msg.contains("error sending request")
        );

        let bad_config_source = DataSource {
            id: 2,
            tenant_id: "test_tenant".to_string(),
            name: "Bad Config Source".to_string(),
            source_type: "web".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: None,
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };
        let result2 = IngressManager::process_source(&bad_config_source).await;
        assert!(result2.is_err());
        assert_eq!(
            result2.unwrap_err().to_string(),
            "Missing or invalid 'url' in config_json for web source"
        );
    }

    #[tokio::test]
    async fn test_ingress_process_unsupported_source() {
        let source = DataSource {
            id: 2,
            tenant_id: "test_tenant".to_string(),
            name: "Unknown Source".to_string(),
            source_type: "unknown".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: None,
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unsupported source type: unknown"
        );
    }

    #[test]
    fn test_process_extraction_csv() {
        let source = DataSource {
            id: 10,
            tenant_id: "test".to_string(),
            name: "CSV Source".to_string(),
            source_type: "tabular".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: Some("PENDING".to_string()),
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: Some("markdown".to_string()),
            s3_key: Some("test/10/data.csv".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let csv_data = b"Name,Score\nAlice,95\n";
        let result = IngressManager::process_extraction(&source, csv_data);
        assert!(result.is_ok());
        let md = result.unwrap();
        assert!(md.contains("| Name | Score |"));
        assert!(md.contains("| Alice | 95 |"));
    }

    #[tokio::test]
    async fn test_fetch_unsupported_type() {
        let source = DataSource {
            id: 3,
            tenant_id: "test".to_string(),
            name: "Doc Source".to_string(),
            source_type: "document".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: None,
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let result = IngressManager::process_fetch(&source).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not applicable"));
    }

    #[test]
    fn test_process_extraction_csv_sql_mode() {
        let source = DataSource {
            id: 20,
            tenant_id: "test_tenant".to_string(),
            name: "SQL CSV Source".to_string(),
            source_type: "tabular".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: Some("PENDING".to_string()),
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: Some("sql".to_string()),
            s3_key: Some("test/20/data.csv".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let csv_data = b"Name,Score\nAlice,95\nBob,88\n";
        let result = IngressManager::process_extraction(&source, csv_data);
        assert!(
            result.is_ok(),
            "SQL mode extraction should succeed: {:?}",
            result.err()
        );
        let summary = result.unwrap();
        assert!(
            summary.contains("SQL Import:"),
            "Should return SQL import summary"
        );
        assert!(
            summary.contains("tenant_test_tenant_src_20"),
            "Should contain table name"
        );
        assert!(summary.contains("2 rows"), "Should report row count");
    }

    #[test]
    fn test_process_extraction_sql_returns_ddl() {
        let source = DataSource {
            id: 30,
            tenant_id: "t1".to_string(),
            name: "SQL Import Test".to_string(),
            source_type: "tabular".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: Some("sql".to_string()),
            s3_key: Some("test/30/data.csv".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let csv_data = b"Name,Age,City\nAlice,30,Bangkok\nBob,25,Tokyo\n";
        let result = IngressManager::process_extraction_sql(&source, csv_data);
        assert!(
            result.is_ok(),
            "process_extraction_sql should succeed: {:?}",
            result.err()
        );

        let import = result.unwrap();
        assert_eq!(import.table_name, "tenant_t1_src_30");
        assert!(
            import
                .create_table_ddl
                .contains("CREATE TABLE IF NOT EXISTS tenant_t1_src_30")
        );
        assert!(import.create_table_ddl.contains("name VARCHAR(255)"));
        assert!(import.create_table_ddl.contains("age DECIMAL"));
        assert_eq!(import.total_rows, 2);
    }

    // ─── New tests for process_source_with_data ────────────────────────────────

    #[test]
    fn test_process_source_with_data_document_txt() {
        let source = DataSource {
            id: 40,
            tenant_id: "test".to_string(),
            name: "TXT Document".to_string(),
            source_type: "document".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: Some("markdown".to_string()),
            s3_key: Some("test/40/readme.txt".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let txt_data = b"Hello, this is a test document for pipeline wiring.";
        let result = IngressManager::process_source_with_data(&source, txt_data);
        assert!(
            result.is_ok(),
            "TXT extraction should succeed: {:?}",
            result.err()
        );
        let content = result.unwrap();
        assert!(content.contains("Hello, this is a test document"));
    }

    #[test]
    fn test_process_source_with_data_tabular_csv() {
        let source = DataSource {
            id: 41,
            tenant_id: "test".to_string(),
            name: "CSV Tabular".to_string(),
            source_type: "tabular".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: Some("markdown".to_string()),
            s3_key: Some("test/41/data.csv".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let csv_data = b"Name,Age,City\nAlice,30,Bangkok\nBob,25,Tokyo\n";
        let result = IngressManager::process_source_with_data(&source, csv_data);
        assert!(
            result.is_ok(),
            "CSV extraction should succeed: {:?}",
            result.err()
        );
        let md = result.unwrap();
        assert!(md.contains("| Name | Age | City |"));
        assert!(md.contains("| Alice | 30 | Bangkok |"));
    }

    #[test]
    fn test_process_source_with_data_empty_file() {
        let source = DataSource {
            id: 42,
            tenant_id: "test".to_string(),
            name: "Empty File".to_string(),
            source_type: "document".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: Some("test/42/empty.txt".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let result = IngressManager::process_source_with_data(&source, b"");
        assert!(result.is_err(), "Empty file should return error");
        assert!(result.unwrap_err().to_string().contains("Empty file data"));
    }

    #[tokio::test]
    async fn test_process_source_document_without_data() {
        let source = DataSource {
            id: 43,
            tenant_id: "test".to_string(),
            name: "Doc Without Data".to_string(),
            source_type: "document".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: None,
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires file data")
        );
    }

    // ─── Issue #122: "file" source_type must work via process_source_with_data ──

    #[test]
    fn test_process_source_with_data_file_type_csv() {
        let source = DataSource {
            id: 122,
            tenant_id: "test".to_string(),
            name: "File Type CSV".to_string(),
            source_type: "file".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: Some("markdown".to_string()),
            s3_key: Some("test/122/data.csv".to_string()),
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let csv_data = b"Name,Score\nAlice,95\nBob,88\n";
        let result = IngressManager::process_source_with_data(&source, csv_data);
        assert!(
            result.is_ok(),
            "File type CSV should extract via process_source_with_data: {:?}",
            result.err()
        );
        let md = result.unwrap();
        assert!(
            md.contains("| Name | Score |"),
            "Should contain markdown table"
        );
    }

    #[tokio::test]
    async fn test_process_source_file_type_requires_data() {
        let source = DataSource {
            id: 123,
            tenant_id: "test".to_string(),
            name: "File Without Data".to_string(),
            source_type: "file".to_string(),
            config_json: json!({}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
            storage_mode: None,
            s3_key: None,
            file_hash: None,
            refresh_interval_hours: None,
            last_refreshed_at: None,
            next_refresh_at: None,
            refresh_status: None,
            pageindex_tree: None,
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err(), "File type without data should error");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires file data")
        );
    }
}
