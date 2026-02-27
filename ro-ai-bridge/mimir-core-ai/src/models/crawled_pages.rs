use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct CrawledPage {
    pub id: i64,
    pub source_id: i64,
    pub url: String,
    pub status: Option<String>,     // "pending", "crawled", "failed"
    pub content_hash: Option<String>,
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}
