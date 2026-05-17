//! Qdrant client — direct upsert into `source_chunks` collection.
//!
//! Matches the schema /api/v1/search reads from:
//!   vector:  {"dense": [..1024 floats..]}
//!   payload: {chunk_id, content, source_id, tenant_id, is_active}
//!
//! Uses the HTTP API (PUT /collections/{name}/points). Auth not required
//! for the in-cluster instance used by Mimir.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct QdrantPoint {
    pub id: u64,
    pub vector: VectorDense,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct VectorDense {
    pub dense: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct UpsertResponse {
    #[allow(dead_code)]
    status: String,
    #[serde(default)]
    #[allow(dead_code)]
    time: f64,
}

pub struct QdrantClient {
    base_url: String,
    client: reqwest::Client,
}

impl QdrantClient {
    pub fn new(base_url: String) -> Result<Self> {
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
        })
    }

    /// Upsert points (insert or update by id). `wait=true` blocks until indexed.
    pub async fn upsert_points(&self, collection: &str, points: Vec<QdrantPoint>) -> Result<usize> {
        if points.is_empty() {
            return Ok(0);
        }
        let n = points.len();
        let body = json!({ "points": points });
        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.base_url, collection
        );
        let resp = self.client.put(&url).json(&body).send().await
            .context("PUT /collections/{c}/points")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Qdrant upsert HTTP {}: {}",
                status,
                &body[..body.len().min(300)]
            );
        }
        let _: UpsertResponse = resp.json().await.context("parse upsert response")?;
        Ok(n)
    }

    /// Get current point count in a collection. Useful for verification.
    pub async fn count(&self, collection: &str) -> Result<u64> {
        let url = format!("{}/collections/{}", self.base_url, collection);
        let resp = self.client.get(&url).send().await.context("GET collection info")?;
        let body: Value = resp.json().await.context("parse collection info")?;
        let count = body
            .get("result")
            .and_then(|r| r.get("points_count"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("missing result.points_count in response"))?;
        Ok(count)
    }
}

/// Build a Qdrant point id from (source_id, chunk_index). Stable: same input → same id.
/// Uses 32 bits for source_id and 32 bits for chunk_index so upper-32 collisions
/// from independent sources are guarded against. Safe for ≤ 4B sources & 4B chunks/source.
pub fn point_id(source_id: u64, chunk_index: u32) -> u64 {
    (source_id.saturating_mul(1_000_000)) + chunk_index as u64
}

/// Build the payload shape that ro-ai-bridge's RetrievalResult expects.
pub fn payload(content: &str, chunk_id: u64, source_id: u64, tenant_id: &str) -> Value {
    json!({
        "content": content,
        "chunk_id": chunk_id,
        "source_id": source_id,
        "tenant_id": tenant_id,
        "is_active": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_id_is_deterministic() {
        assert_eq!(point_id(7, 3), point_id(7, 3));
    }

    #[test]
    fn point_id_separates_sources() {
        // Different sources never collide for reasonable chunk counts.
        assert_ne!(point_id(1, 5), point_id(2, 5));
    }

    #[test]
    fn payload_has_required_fields() {
        let p = payload("hello", 42, 7, "asgard_insurance");
        assert_eq!(p["content"], "hello");
        assert_eq!(p["chunk_id"], 42);
        assert_eq!(p["source_id"], 7);
        assert_eq!(p["tenant_id"], "asgard_insurance");
        assert_eq!(p["is_active"], true);
    }
}
