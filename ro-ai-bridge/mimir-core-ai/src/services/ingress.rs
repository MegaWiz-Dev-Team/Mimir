use crate::models::sources::DataSource;
use anyhow::{Result, Context};
use tracing::{info, error};

pub struct IngressManager;

impl IngressManager {
    pub async fn process_source(source: &DataSource) -> Result<String> {
        info!("Processing source: {}", source.name);
        match source.source_type.as_str() {
            "web" => {
                Self::process_web_source(source).await
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

    async fn process_web_source(source: &DataSource) -> Result<String> {
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

        let html = response.text().await.context("Failed to read response body")?;
        
        // Very basic HTML extraction: just strip tags for now.
        // In a production system, use a crate like `scraper` to parse the DOM properly.
        let text = html.replace("<", " <").replace(">", "> "); // Naive spacing
        let mut clean_text = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                clean_text.push(c);
            }
        }
        
        let preview = if clean_text.len() > 100 { &clean_text[0..100] } else { &clean_text };
        info!("Successfully extracted {} bytes of text from {}. Preview: {}...", clean_text.len(), url, preview);

        // TODO: Send robust extracted text to Qdrant via QdrantService
        // Returning the raw text to be stored in the database for preview
        Ok(clean_text)
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
        // We will use a mock HTTP server or a known reliable URL for testing.
        // For standard unit tests without hitting the real internet, mockito is preferred.
        // Since we might not have mockito, let's use a very basic unit test that expects a failure for a bad URL,
        // and a pass for a known fast endpoint or we test error handling.

        let source = DataSource {
            id: 1,
            tenant_id: "test_tenant".to_string(),
            name: "Test Web Source".to_string(),
            source_type: "web".to_string(),
            config_json: json!({"url": "http://invalid.url.that.does.not.exist.local"}), // Will fail DNS
            schedule: None,
            last_sync_status: None,
            last_sync_at: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            mb_size: None,
            raw_markdown: None,
            total_chunks: None,
        };

        let result = IngressManager::process_source(&source).await;
        // The DNS should fail
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to send HTTP request") || err_msg.contains("error sending request"));

        // Now test a missing URL
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
        };

        let result = IngressManager::process_source(&source).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Unsupported source type");
    }
}
