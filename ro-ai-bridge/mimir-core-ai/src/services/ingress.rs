use crate::models::sources::DataSource;
use crate::services::extraction;
use anyhow::{Result, Context};
use tracing::{info, error, warn};

pub struct IngressManager;

impl IngressManager {
    /// Phase 3: Extract content from raw data downloaded from RustFS.
    ///
    /// Routes to the appropriate extraction function based on `source_type`
    /// and file extension (from `s3_key`). Returns Markdown content for `raw_markdown`.
    pub fn process_extraction(source: &DataSource, data: &[u8]) -> Result<String> {
        let s3_key = source.s3_key.as_deref().unwrap_or("unknown");
        info!(
            "Phase 3: Extracting source_id={}, type={}, s3_key={}, size={} bytes",
            source.id, source.source_type, s3_key, data.len()
        );

        extraction::extract(&source.source_type, s3_key, data)
    }

    /// Phase 2: Fetch raw content for Web/MCP sources.
    ///
    /// - Web: downloads HTML via reqwest
    /// - MCP: fetches JSON from MCP server (placeholder — requires McpClient integration)
    ///
    /// Returns raw bytes to be saved to RustFS.
    pub async fn process_fetch(source: &DataSource) -> Result<Vec<u8>> {
        info!("Phase 2: Fetching source_id={}, type={}", source.id, source.source_type);

        match source.source_type.as_str() {
            "web" => Self::fetch_web(source).await,
            "mcp" => Self::fetch_mcp(source).await,
            _ => Err(anyhow::anyhow!(
                "Phase 2 fetch not applicable for source_type: {}", source.source_type
            )),
        }
    }

    /// Fetch raw HTML from a web URL.
    async fn fetch_web(source: &DataSource) -> Result<Vec<u8>> {
        let url = source.config_json.get("url")
            .and_then(|v| v.as_str())
            .context("Missing or invalid 'url' in config_json for web source")?;

        info!("Fetching HTML from URL: {}", url);

        let client = reqwest::Client::new();
        let response = client.get(url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .context("Failed to send HTTP request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
        }

        let bytes = response.bytes().await.context("Failed to read response body")?;
        info!("Fetched {} bytes from {}", bytes.len(), url);

        Ok(bytes.to_vec())
    }

    /// Fetch data from an MCP server.
    ///
    /// Reads the `mcp_url` from `config_json`, connects to the MCP server,
    /// fetches all resources, and returns them as JSON bytes.
    async fn fetch_mcp(source: &DataSource) -> Result<Vec<u8>> {
        let mcp_url = source.config_json.get("mcp_url")
            .and_then(|v| v.as_str())
            .context("Missing 'mcp_url' in config_json for MCP source")?;

        info!("Connecting to MCP server: {}", mcp_url);

        let mut mcp_client = crate::services::mcp_client::McpClient::new(mcp_url);
        mcp_client.connect().await.context("Failed to connect to MCP server")?;

        let resources = mcp_client.fetch_resources().await
            .context("Failed to fetch MCP resources")?;

        // Serialize all resources to JSON bytes
        let json_bytes = serde_json::to_vec_pretty(&resources)
            .context("Failed to serialize MCP resources to JSON")?;

        info!("Fetched {} MCP resources ({} bytes)", resources.len(), json_bytes.len());
        Ok(json_bytes)
    }

    /// Legacy entry point — process a source synchronously (backward compat).
    ///
    /// For `web` sources, performs a simple HTML→text extraction inline.
    /// For other types, returns a placeholder message.
    pub async fn process_source(source: &DataSource) -> Result<String> {
        info!("Processing source: {}", source.name);
        match source.source_type.as_str() {
            "web" => {
                let data = Self::fetch_web(source).await?;
                extraction::extract_html_to_markdown(&data)
            },
            "tabular" => {
                Ok(format!("Parsed tabular data for {}", source.name))
            },
            "document" => {
                Ok(format!("Extracted text from document {}", source.name))
            },
            "mcp" => {
                Ok(format!("Fetched data via MCP for {}", source.name))
            },
            _ => Err(anyhow::anyhow!("Unsupported source type")),
        }
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
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to send HTTP request") || err_msg.contains("error sending request"));

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
        };
        let result2 = IngressManager::process_source(&bad_config_source).await;
        assert!(result2.is_err());
        assert_eq!(result2.unwrap_err().to_string(), "Missing or invalid 'url' in config_json for web source");
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
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unsupported source type");
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
        };

        let result = IngressManager::process_fetch(&source).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not applicable"));
    }
}
