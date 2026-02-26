use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DataSource {
    pub id: i64,
    pub tenant_id: String,
    pub name: String,
    pub source_type: String, // "web", "tabular", "document", "mcp"
    pub config_json: serde_json::Value,
    pub schedule: Option<String>,
    pub last_sync_status: Option<String>,
    pub raw_markdown: Option<String>,
    pub mb_size: Option<f64>,
    pub total_chunks: Option<i32>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub storage_mode: Option<String>,   // "markdown" | "sql"
    pub s3_key: Option<String>,         // RustFS object path
    pub file_hash: Option<String>,      // SHA-256 for dedup
}

#[derive(Debug, Deserialize)]
pub struct CreateDataSourceRequest {
    pub name: String,
    pub source_type: String,
    pub config_json: serde_json::Value,
    pub schedule: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDataSourceRequest {
    pub name: Option<String>,
    pub config_json: Option<serde_json::Value>,
    pub schedule: Option<String>,
}
