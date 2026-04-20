use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct Config {
    // Server
    pub port: u16,

    // Database
    pub mariadb_url: String,
    pub qdrant_url: String,
    pub redis_url: String,

    // S3 / RustFS
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_region: String,

    // LLM
    pub ollama_url: String,
    pub local_model: String,
    pub embed_model: String,
    pub gemini_base_url: String,
    pub gemini_api_key: Option<String>,
    pub gemini_model: String,
    pub heimdall_api_url: String,
    pub heimdall_api_key: Option<String>,
    pub heimdall_model: String,

    // Auth
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok(); // Load .env file if it exists

        info!("Loading configuration from environment...");

        let config = Self {
            // Server
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),

            // Database
            mariadb_url: env::var("MARIADB_URL").unwrap_or_else(|_| {
                "mysql://mimir:mimir_password@localhost:3306/mimir".to_string()
            }),
            qdrant_url: env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6333".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),

            // S3 / RustFS
            s3_endpoint: env::var("S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            s3_bucket: env::var("S3_BUCKET").unwrap_or_else(|_| "mimir-tenant-uploads".to_string()),
            s3_access_key: env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
            s3_secret_key: env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
            s3_region: env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),

            // LLM
            ollama_url: env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            local_model: env::var("LOCAL_MODEL").unwrap_or_else(|_| "llama3.2".to_string()),
            embed_model: env::var("EMBED_MODEL").unwrap_or_else(|_| "BAAI/bge-m3".to_string()),
            gemini_base_url: env::var("GEMINI_BASE_URL").unwrap_or_else(|_| {
                "https://generativelanguage.googleapis.com/v1beta/openai/".to_string()
            }),
            gemini_api_key: env::var("GEMINI_API_KEY").ok(),
            gemini_model: env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.5-flash".to_string()),

            // Heimdall (Self-hosted LLM Gateway)
            heimdall_api_url: env::var("HEIMDALL_API_URL").unwrap_or_else(|_| {
                "https://stroppy-nonsensorial-lakita.ngrok-free.dev/v1".to_string()
            }),
            heimdall_api_key: env::var("HEIMDALL_API_KEY").ok(),
            heimdall_model: env::var("HEIMDALL_MODEL")
                .unwrap_or_else(|_| "mlx-community/Qwen3.5-35B-A3B-4bit".to_string()),

            // Auth
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "dev_secret_key".to_string()),
        };

        info!("Configuration loaded successfully.");
        config
    }
}

// ============================================================================
// Q/A Configuration
// ============================================================================

/// Configuration for Q/A generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QAConfig {
    /// Default number of Q/A pairs if no rule matches
    pub default_count: usize,
    /// Rules based on content size
    pub rules: Vec<SizeRule>,
    /// Rules based on file name patterns
    pub file_patterns: FilePatternConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeRule {
    /// Optional comment for documentation
    pub comment: Option<String>,
    /// Minimum content size (in characters) for this rule to apply
    pub min_size: usize,
    /// Maximum content size (in characters) for this rule to apply (null = no limit)
    pub max_size: Option<usize>,
    /// Number of Q/A pairs to generate
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePatternConfig {
    pub comment: Option<String>,
    pub patterns: Vec<PatternRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRule {
    /// Glob pattern to match file name (e.g., "*boss*", "*quest*")
    pub pattern: String,
    /// Number of Q/A pairs to generate
    pub count: usize,
    /// Optional reason for this override
    pub reason: Option<String>,
}

impl Default for QAConfig {
    fn default() -> Self {
        Self {
            default_count: 3,
            rules: vec![
                SizeRule {
                    comment: Some("Small files (< 2000 chars) - fewer Q/A pairs".to_string()),
                    min_size: 0,
                    max_size: Some(2000),
                    count: 2,
                },
                SizeRule {
                    comment: Some(
                        "Medium files (2000-10000 chars) - moderate Q/A pairs".to_string(),
                    ),
                    min_size: 2000,
                    max_size: Some(10000),
                    count: 3,
                },
                SizeRule {
                    comment: Some("Large files (> 10000 chars) - more Q/A pairs".to_string()),
                    min_size: 10000,
                    max_size: None,
                    count: 5,
                },
            ],
            file_patterns: FilePatternConfig {
                comment: Some("Override rules based on file name patterns (glob)".to_string()),
                patterns: vec![],
            },
        }
    }
}

impl QAConfig {
    /// Load configuration from a JSON file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: QAConfig = serde_json::from_str(&content)?;
        info!("📋 Loaded QA config from {}", path);
        Ok(config)
    }

    /// Load configuration from file, or return default if file doesn't exist
    pub fn from_file_or_default(path: &str) -> Self {
        match Self::from_file(path) {
            Ok(config) => config,
            Err(e) => {
                warn!(
                    "⚠️ Failed to load QA config from {}: {}. Using defaults.",
                    path, e
                );
                Self::default()
            }
        }
    }

    /// Load configuration from a generic JSON Value (e.g., from DB)
    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// Determine Q/A count based on file name and content size
    /// Priority: file pattern > size rule > default
    pub fn get_qa_count(&self, file_name: &str, content_size: usize) -> usize {
        // 1. Check file patterns first (highest priority)
        for pattern_rule in &self.file_patterns.patterns {
            if self.matches_pattern(file_name, &pattern_rule.pattern) {
                info!(
                    "🎯 File pattern '{}' matched for '{}': {} Q/A pairs",
                    pattern_rule.pattern, file_name, pattern_rule.count
                );
                return pattern_rule.count;
            }
        }

        // 2. Check size rules
        for rule in &self.rules {
            let min_ok = content_size >= rule.min_size;
            let max_ok = rule.max_size.map_or(true, |max| content_size < max);

            if min_ok && max_ok {
                info!(
                    "📏 Size rule matched for '{}' ({} chars): {} Q/A pairs",
                    file_name, content_size, rule.count
                );
                return rule.count;
            }
        }

        // 3. Fall back to default
        info!(
            "📋 Using default count for '{}': {} Q/A pairs",
            file_name, self.default_count
        );
        self.default_count
    }

    /// Simple glob pattern matching (supports * wildcard only)
    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with('*') && pattern.ends_with('*') {
            // *something* - contains
            let inner = &pattern[1..pattern.len() - 1];
            text.contains(inner)
        } else if pattern.starts_with('*') {
            // *something - ends with
            let suffix = &pattern[1..];
            text.ends_with(suffix)
        } else if pattern.ends_with('*') {
            // something* - starts with
            let prefix = &pattern[..pattern.len() - 1];
            text.starts_with(prefix)
        } else {
            // exact match
            text == pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = QAConfig::default();
        assert_eq!(config.default_count, 3);
        assert_eq!(config.rules.len(), 3);
    }

    #[test]
    fn test_size_rules() {
        let config = QAConfig::default();

        // Small file
        assert_eq!(config.get_qa_count("test.md", 500), 2);

        // Medium file
        assert_eq!(config.get_qa_count("test.md", 5000), 3);

        // Large file
        assert_eq!(config.get_qa_count("test.md", 15000), 5);
    }

    #[test]
    fn test_pattern_matching() {
        let config = QAConfig::default();

        // Test contains pattern
        assert!(config.matches_pattern("boss_monster.md", "*boss*"));
        assert!(config.matches_pattern("the_boss_fight.md", "*boss*"));
        assert!(!config.matches_pattern("monster.md", "*boss*"));

        // Test starts with
        assert!(config.matches_pattern("quest_guide.md", "quest*"));
        assert!(!config.matches_pattern("my_quest.md", "quest*"));

        // Test ends with
        assert!(config.matches_pattern("item_sword.md", "*sword.md"));
        assert!(!config.matches_pattern("sword_item.md", "*sword.md"));
    }
}
