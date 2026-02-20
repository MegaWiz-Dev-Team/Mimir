use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::env;
use tracing::info;

#[derive(Clone)]
pub struct QdrantService {
    client: Client,
    base_url: String,
}

impl QdrantService {
    pub fn new() -> Self {
        let host = env::var("QDRANT_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("QDRANT_PORT").unwrap_or_else(|_| "6333".to_string());
        Self {
            client: Client::new(),
            base_url: format!("http://{}:{}", host, port),
        }
    }

    pub async fn init_collection(&self, collection_name: &str, vector_size: u64) -> Result<()> {
        info!("🔍 Checking Qdrant collection: {}", collection_name);
        
        // Check if exists
        let url = format!("{}/collections/{}", self.base_url, collection_name);
        let resp = self.client.get(&url).send().await?;

        if resp.status().is_success() {
            info!("✅ Collection {} already exists", collection_name);
            return Ok(());
        }

        self.create_collection(collection_name, vector_size).await
    }

    pub async fn delete_collection(&self, collection_name: &str) -> Result<()> {
        info!("🗑️ Deleting Qdrant collection: {}", collection_name);
        let url = format!("{}/collections/{}", self.base_url, collection_name);
        let resp = self.client.delete(&url).send().await?;
        
        if !resp.status().is_success() {
            // It's okay if it doesn't exist, but log error if other issues
            if resp.status() != 404 {
                let error = resp.text().await?;
                return Err(anyhow::anyhow!("Failed to delete collection: {}", error));
            }
        }
        Ok(())
    }

    async fn create_collection(&self, collection_name: &str, vector_size: u64) -> Result<()> {
        info!("🏗️ Creating Qdrant collection: {}", collection_name);
        let create_url = format!("{}/collections/{}", self.base_url, collection_name);
        let body = json!({
            "vectors": {
                "size": vector_size,
                "distance": "Cosine"
            },
            "sparse_vectors": {
                "text-sparse": {
                    "index": {
                        "full_scan_threshold": 1000
                    }
                }
            }
        });

        let create_resp = self.client.put(&create_url)
            .json(&body)
            .send()
            .await?;

        if !create_resp.status().is_success() {
            let error = create_resp.text().await?;
            return Err(anyhow::anyhow!("Failed to create collection: {}", error));
        }

        info!("✅ Collection {} created successfully", collection_name);
        Ok(())
    }

    pub async fn upsert_points(&self, collection_name: &str, points: serde_json::Value) -> Result<()> {
        let url = format!("{}/collections/{}/points", self.base_url, collection_name);
        let resp = self.client.put(&url)
            .json(&points)
            .send()
            .await?;

        if !resp.status().is_success() {
            let error = resp.text().await?;
            return Err(anyhow::anyhow!("Failed to upsert points: {}", error));
        }

        Ok(())
    }

    pub async fn get_collection_info(&self, collection_name: &str) -> Result<serde_json::Value> {
        let url = format!("{}/collections/{}", self.base_url, collection_name);
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            let error = resp.text().await?;
            return Err(anyhow::anyhow!("Failed to get collection info: {}", error));
        }

        let body = resp.json().await?;
        Ok(body)
    }

    pub async fn search(&self, collection_name: &str, vector: Vec<f32>, limit: usize, tenant_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/collections/{}/points/search", self.base_url, collection_name);
        let body = json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true,
            "filter": {
                "must": [
                    { "key": "tenant_id", "match": { "value": tenant_id } }
                ]
            }
        });

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let error = resp.text().await?;
            return Err(anyhow::anyhow!("Failed to search vectors: {}", error));
        }

        let res = resp.json().await?;
        Ok(res)
    }
}
