use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DataSource {
    pub id: i64,
    pub tenant_id: String,
    pub name: String,
    pub source_type: String, // "web", "tabular", "document", "mcp", "image", "database"
    pub config_json: serde_json::Value,
    pub schedule: Option<String>,
    pub last_sync_status: Option<String>,
    pub raw_markdown: Option<String>,
    pub mb_size: Option<f64>,
    pub total_chunks: Option<i32>,
    pub pageindex_tree: Option<serde_json::Value>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub storage_mode: Option<String>,   // "markdown" | "sql"
    pub s3_key: Option<String>,         // RustFS object path
    pub file_hash: Option<String>,      // SHA-256 for dedup
    // Sprint 14: Cron scheduling fields
    pub refresh_interval_hours: Option<i32>,
    pub last_refreshed_at: Option<DateTime<Utc>>,
    pub next_refresh_at: Option<DateTime<Utc>>,
    pub refresh_status: Option<String>, // "idle" | "running" | "failed"
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

/// Sprint 14: Request to set/update cron refresh schedule
#[derive(Debug, Deserialize)]
pub struct SetScheduleRequest {
    /// Refresh interval in hours (0 = disable)
    pub refresh_interval_hours: Option<i32>,
}
