use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ContentFingerprint {
    pub id: i64,
    pub content_hash: String,
    pub source_id: i64,
    pub chunk_id: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
}
