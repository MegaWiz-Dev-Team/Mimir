use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::env;
use tracing::info;

use crate::services::bm25::SparseVector;

#[derive(Clone)]
pub struct QdrantService {
    client: Client,
    base_url: String,
}

impl QdrantService {
    pub fn new() -> Self {
        let base_url = env::var("QDRANT_URL").unwrap_or_else(|_| {
            let host = env::var("QDRANT_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = env::var("QDRANT_PORT").unwrap_or_else(|_| "6333".to_string());
            if port.starts_with("tcp://") {
                // Handle Kubernetes automatic service port injection
                format!("http://{}:6333", host)
            } else {
                format!("http://{}:{}", host, port)
            }
        });
        
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn init_collection(&self, collection_name: &str, vector_size: u64) -> Result<()> {
        info!("🔍 Checking Qdrant collection: {}", collection_name);
        
        // Check if exists
        let url = format!("{}/collections/{}", self.base_url, collection_name);
        let resp = self.client.get(&url).send().await?;

        if resp.status().is_success() {
            let info: serde_json::Value = resp.json().await?;
            let current_size = info.pointer("/result/config/params/vectors/dense/size")
                .or_else(|| info.pointer("/result/config/params/vectors/size"))
                .and_then(|v| v.as_u64());

            if let Some(size) = current_size {
                if size != vector_size {
                    info!("⚠️ Dimension mismatch in {}: found {}d, need {}d. Recreating collection automatically...", collection_name, size, vector_size);
                    self.delete_collection(collection_name).await?;
                    return self.create_collection(collection_name, vector_size).await;
                }
            }

            info!("✅ Collection {} already exists and matches dimensions ({}d)", collection_name, vector_size);
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
        info!("🏗️ Creating Qdrant collection: {} (hybrid: dense={}d + sparse bm25)", collection_name, vector_size);
        let create_url = format!("{}/collections/{}", self.base_url, collection_name);
        let body = json!({
            "vectors": {
                "dense": {
                    "size": vector_size,
                    "distance": "Cosine"
                }
            },
            "sparse_vectors": {
                "bm25": {
                    "modifier": "idf"
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

        info!("✅ Collection {} created successfully (hybrid mode)", collection_name);
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

    /// Dense-only search (legacy, used by wiki_qa)
    pub async fn search(&self, collection_name: &str, vector: Vec<f32>, limit: usize, tenant_id: &str, show_expired: bool) -> Result<serde_json::Value> {
        let url = format!("{}/collections/{}/points/search", self.base_url, collection_name);
        
        let mut must_conditions = vec![
            json!({ "key": "tenant_id", "match": { "value": tenant_id } })
        ];
        
        if !show_expired {
            must_conditions.push(json!({ "key": "is_active", "match": { "value": true } }));
        }

        let body = json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true,
            "filter": {
                "must": must_conditions
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

    /// Hybrid search using dense + sparse vectors with Reciprocal Rank Fusion (RRF).
    /// Uses Qdrant's /points/query endpoint with prefetch + fusion.
    pub async fn search_hybrid(
        &self,
        collection_name: &str,
        dense_vector: Vec<f32>,
        sparse_vector: &SparseVector,
        limit: usize,
        tenant_id: &str,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/collections/{}/points/query", self.base_url, collection_name);
        
        let filter = json!({
            "must": [
                { "key": "tenant_id", "match": { "value": tenant_id } },
                { "key": "is_active", "match": { "value": true } }
            ]
        });

        let body = json!({
            "prefetch": [
                {
                    "query": dense_vector,
                    "using": "dense",
                    "limit": limit * 3,
                    "filter": filter,
                },
                {
                    "query": {
                        "indices": sparse_vector.indices,
                        "values": sparse_vector.values,
                    },
                    "using": "bm25",
                    "limit": limit * 3,
                    "filter": filter,
                }
            ],
            "query": { "fusion": "rrf" },
            "limit": limit,
            "with_payload": true,
        });

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let error = resp.text().await?;
            return Err(anyhow::anyhow!("Hybrid search failed: {}", error));
        }

        let res = resp.json().await?;
        Ok(res)
    }

    pub async fn delete_point(&self, collection_name: &str, point_id: u64) -> Result<()> {
        let url = format!("{}/collections/{}/points/delete", self.base_url, collection_name);
        let body = json!({
            "points": [point_id]
        });

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let error = resp.text().await?;
            return Err(anyhow::anyhow!("Failed to delete point: {}", error));
        }

        Ok(())
    }
}
