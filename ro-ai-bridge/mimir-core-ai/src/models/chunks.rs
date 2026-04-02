use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Chunk {
    pub id: i64,
    pub source_id: i64,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: Option<i32>,
    pub metadata_json: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}
