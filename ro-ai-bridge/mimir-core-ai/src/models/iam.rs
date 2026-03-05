use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserWithRole {
    pub id: String,
    pub username: String,
    pub tenant_id: Option<String>,
    pub role: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: Option<String>,
    pub tenant_id: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub tenant_id: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserPasswordRequest {
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTenantRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub domain: Option<String>,
    pub is_dedicated_vector_db: bool,
    pub admin_email: String,
    pub admin_password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {

    pub token: String,
    pub tenant_id: String,
}

/// A single provider+model slot for a specific purpose (chat, rag, pipeline, etc.)
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct LlmSlot {
    pub provider: String,
    pub model: String,
}

/// Per-purpose LLM/Embedding configuration — stored as JSON in tenant_configs.llm_config
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LlmConfig {
    /// Model for Chat / NPC agents (Tier 1 & 2)
    pub chat: Option<LlmSlot>,
    /// Model for RAG (Oracle Agent) queries
    pub rag: Option<LlmSlot>,
    /// Model for QA pipeline generation
    pub pipeline_generator: Option<LlmSlot>,
    /// Model for QA pipeline evaluation (ACU extraction, coverage)
    pub pipeline_evaluator: Option<LlmSlot>,
    /// Model for LLM-as-Judge scoring
    pub judge: Option<LlmSlot>,
    /// Embedding model for vector search
    pub embedding: Option<LlmSlot>,
    /// Heimdall gateway URL (stored via Vault in production)
    pub heimdall_url: Option<String>,
    /// Heimdall API key (stored via Vault in production)
    pub heimdall_api_key: Option<String>,
}

impl LlmConfig {
    /// Resolve a slot by name with 3-tier fallback:
    /// 1. Specific slot value (e.g., llm_config.chat)
    /// 2. Provided defaults (default_provider + default_model from TenantConfig)
    /// 3. Hardcoded fallback (ollama + llama3.2, or ollama + nomic-embed-text for embedding)
    pub fn resolve_slot(&self, slot_name: &str, default_provider: Option<&str>, default_model: Option<&str>) -> LlmSlot {
        let slot = match slot_name {
            "chat" => self.chat.as_ref(),
            "rag" => self.rag.as_ref(),
            "pipeline_generator" => self.pipeline_generator.as_ref(),
            "pipeline_evaluator" => self.pipeline_evaluator.as_ref(),
            "judge" => self.judge.as_ref(),
            "embedding" => self.embedding.as_ref(),
            _ => None,
        };

        // Tier 1: Specific slot value
        if let Some(s) = slot {
            if !s.provider.is_empty() && !s.model.is_empty() {
                return s.clone();
            }
        }

        // Tier 2: TenantConfig defaults
        if let (Some(p), Some(m)) = (default_provider, default_model) {
            if !p.is_empty() && !m.is_empty() {
                return LlmSlot { provider: p.to_string(), model: m.to_string() };
            }
        }

        // Tier 3: Hardcoded fallback
        if slot_name == "embedding" {
            LlmSlot { provider: "ollama".to_string(), model: "nomic-embed-text".to_string() }
        } else {
            LlmSlot { provider: "ollama".to_string(), model: "llama3.2".to_string() }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub default_provider: String,
    pub default_model: String,
    pub provider_api_keys: Option<sqlx::types::Json<serde_json::Value>>,
    pub qa_rules: Option<sqlx::types::Json<serde_json::Value>>,
    pub system_prompt: Option<String>,
    pub max_daily_tokens: i64,
    pub is_dedicated_vector_db: bool,
    pub max_crawl_pages: i32,
    pub search_settings: Option<sqlx::types::Json<serde_json::Value>>,
    pub llm_config: Option<sqlx::types::Json<LlmConfig>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateTenantConfigRequest {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub provider_api_keys: Option<sqlx::types::Json<serde_json::Value>>,
    pub qa_rules: Option<sqlx::types::Json<serde_json::Value>>,
    pub system_prompt: Option<String>,
    pub max_daily_tokens: Option<i64>,
    pub is_dedicated_vector_db: Option<bool>,
    pub max_crawl_pages: Option<i32>,
    pub search_settings: Option<sqlx::types::Json<serde_json::Value>>,
    pub llm_config: Option<sqlx::types::Json<LlmConfig>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_slot_serialization() {
        let slot = LlmSlot { provider: "gemini".into(), model: "gemini-2.5-flash".into() };
        let json = serde_json::to_string(&slot).unwrap();
        assert!(json.contains("gemini-2.5-flash"));
        let parsed: LlmSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, slot);
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert!(config.chat.is_none());
        assert!(config.embedding.is_none());
        assert!(config.heimdall_url.is_none());
    }

    #[test]
    fn test_resolve_slot_tier1_specific_value() {
        let config = LlmConfig {
            chat: Some(LlmSlot { provider: "heimdall".into(), model: "Qwen3.5-35B".into() }),
            ..Default::default()
        };
        let slot = config.resolve_slot("chat", Some("ollama"), Some("llama3.2"));
        assert_eq!(slot.provider, "heimdall");
        assert_eq!(slot.model, "Qwen3.5-35B");
    }

    #[test]
    fn test_resolve_slot_tier2_tenant_default() {
        let config = LlmConfig::default(); // No specific chat slot
        let slot = config.resolve_slot("chat", Some("gemini"), Some("gemini-2.5-flash"));
        assert_eq!(slot.provider, "gemini");
        assert_eq!(slot.model, "gemini-2.5-flash");
    }

    #[test]
    fn test_resolve_slot_tier3_hardcoded_fallback() {
        let config = LlmConfig::default();
        let slot = config.resolve_slot("chat", None, None);
        assert_eq!(slot.provider, "ollama");
        assert_eq!(slot.model, "llama3.2");
    }

    #[test]
    fn test_resolve_slot_embedding_fallback() {
        let config = LlmConfig::default();
        let slot = config.resolve_slot("embedding", None, None);
        assert_eq!(slot.provider, "ollama");
        assert_eq!(slot.model, "nomic-embed-text");
    }

    #[test]
    fn test_resolve_slot_unknown_name_uses_default() {
        let config = LlmConfig::default();
        let slot = config.resolve_slot("unknown_slot", Some("ollama"), Some("llama3.2"));
        assert_eq!(slot.provider, "ollama");
        assert_eq!(slot.model, "llama3.2");
    }

    #[test]
    fn test_resolve_slot_empty_provider_falls_through() {
        let config = LlmConfig {
            chat: Some(LlmSlot { provider: "".into(), model: "".into() }),
            ..Default::default()
        };
        // Empty slot should fall through to tier 2
        let slot = config.resolve_slot("chat", Some("gemini"), Some("gemini-2.5-flash"));
        assert_eq!(slot.provider, "gemini");
    }

    #[test]
    fn test_llm_config_full_serialization() {
        let config = LlmConfig {
            chat: Some(LlmSlot { provider: "ollama".into(), model: "llama3.2".into() }),
            rag: Some(LlmSlot { provider: "heimdall".into(), model: "Qwen3.5-35B-A3B-4bit".into() }),
            pipeline_generator: Some(LlmSlot { provider: "gemini".into(), model: "gemini-2.5-flash".into() }),
            pipeline_evaluator: Some(LlmSlot { provider: "gemini".into(), model: "gemini-2.5-flash".into() }),
            judge: Some(LlmSlot { provider: "gemini".into(), model: "gemini-2.5-flash".into() }),
            embedding: Some(LlmSlot { provider: "ollama".into(), model: "nomic-embed-text".into() }),
            heimdall_url: Some("https://example.ngrok.dev/v1".into()),
            heimdall_api_key: Some("test-key".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: LlmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.chat.unwrap().model, "llama3.2");
        assert_eq!(parsed.rag.unwrap().provider, "heimdall");
        assert_eq!(parsed.heimdall_url.unwrap(), "https://example.ngrok.dev/v1");
    }
}

