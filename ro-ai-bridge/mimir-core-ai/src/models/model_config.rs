use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ModelConfig {
    pub model_id: String,
    pub provider: String,
    pub model_type: String,
    pub is_active: bool,
    pub capabilities: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ModelConfig {
    /// Helper to check if model supports specific capability
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities
            .as_ref()
            .and_then(|c| c.get(cap))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}
