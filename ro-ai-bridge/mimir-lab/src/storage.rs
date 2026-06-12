//! MinIO / S3 blob storage for dataset originals + Parquet versions.
//!
//! Uses `rust-s3` against a MinIO endpoint (path-style). Config from env,
//! matching the rest of Mimir (`S3_ENDPOINT` / `S3_BUCKET` / `S3_ACCESS_KEY` /
//! `S3_SECRET_KEY` / `S3_REGION`; MinIO defaults `minioadmin`). Per ADR-024 the
//! DuckDB/Parquet catalog holds working copies; MinIO holds the durable blobs.

use crate::error::{LabError, Result};
use s3::creds::Credentials;
use s3::{Bucket, Region};

pub struct Storage {
    bucket: Box<Bucket>,
}

/// Read MinIO/S3 settings from the environment.
pub struct StorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

impl StorageConfig {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            endpoint: get("S3_ENDPOINT", "http://localhost:9000"),
            bucket: get("S3_BUCKET", "asgard-analytics"),
            region: get("S3_REGION", "us-east-1"),
            access_key: get("S3_ACCESS_KEY", "minioadmin"),
            secret_key: get("S3_SECRET_KEY", "minioadmin"),
        }
    }
}

impl Storage {
    pub fn from_env() -> Result<Self> {
        Self::new(StorageConfig::from_env())
    }

    pub fn new(cfg: StorageConfig) -> Result<Self> {
        let region = Region::Custom {
            region: cfg.region,
            endpoint: cfg.endpoint,
        };
        let creds = Credentials::new(Some(&cfg.access_key), Some(&cfg.secret_key), None, None, None)
            .map_err(|e| LabError::Storage(e.to_string()))?;
        // MinIO requires path-style addressing.
        let bucket = Bucket::new(&cfg.bucket, region, creds)
            .map_err(|e| LabError::Storage(e.to_string()))?
            .with_path_style();
        Ok(Self { bucket })
    }

    /// Upload bytes to `key`. Returns the `s3://bucket/key` URI.
    pub async fn put(&self, key: &str, bytes: &[u8]) -> Result<String> {
        self.bucket
            .put_object(key, bytes)
            .await
            .map_err(|e| LabError::Storage(e.to_string()))?;
        Ok(format!("s3://{}/{}", self.bucket.name(), key))
    }

    /// Download the bytes stored at `key`.
    pub async fn get(&self, key: &str) -> Result<Vec<u8>> {
        let resp = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| LabError::Storage(e.to_string()))?;
        Ok(resp.bytes().to_vec())
    }

    /// Delete the object at `key`.
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| LabError::Storage(e.to_string()))?;
        Ok(())
    }
}
