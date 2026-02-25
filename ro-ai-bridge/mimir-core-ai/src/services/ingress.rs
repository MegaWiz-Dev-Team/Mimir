use crate::models::sources::DataSource;
use anyhow::Result;
use tracing::info;

pub struct IngressManager;

impl IngressManager {
    pub async fn process_source(source: &DataSource) -> Result<String> {
        info!("Processing source: {}", source.name);
        match source.source_type.as_str() {
            "web" => {
                // In a real scenario, this would call ScraperService
                Ok(format!("Scraped content from {}", source.name))
            },
            "tabular" => {
                // Call TableParser
                Ok(format!("Parsed tabular data for {}", source.name))
            },
            "document" => {
                // Call VisionParser
                Ok(format!("Extracted text from document {}", source.name))
            },
            "mcp" => {
                // Call McpClient
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
            config_json: json!({"url": "https://example.com"}),
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Scraped content from Test Web Source");
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
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unsupported source type");
    }
}
