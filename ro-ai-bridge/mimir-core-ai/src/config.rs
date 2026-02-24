use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use dotenvy::dotenv;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct Config {
    pub ollama_url: String,
    pub qdrant_url: String,
    pub mariadb_url: String,
    pub redis_url: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok(); // Load .env file if it exists

        info!("Loading configuration from environment...");

        let config = Self {
            ollama_url: env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            qdrant_url: env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6333".to_string()),
            mariadb_url: env::var("MARIADB_URL")
                .unwrap_or_else(|_| "mysql://mimir:REDACTED-PW@localhost:3306/ro_landverse".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),
        };

        info!("Configuration loaded successfully.");
        config
    }
}

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
    /// Glob pattern to match file name
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
                    comment: Some("Medium files (2000-10000 chars) - moderate Q/A pairs".to_string()),
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
                warn!("⚠️ Failed to load QA config from {}: {}. Using defaults.", path, e);
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
        for pattern_rule in &self.file_patterns.patterns {
            if self.matches_pattern(file_name, &pattern_rule.pattern) {
                return pattern_rule.count;
            }
        }

        for rule in &self.rules {
            let min_ok = content_size >= rule.min_size;
            let max_ok = rule.max_size.map_or(true, |max| content_size < max);
            
            if min_ok && max_ok {
                return rule.count;
            }
        }

        self.default_count
    }

    /// Simple glob pattern matching (supports * wildcard only)
    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with('*') && pattern.ends_with('*') {
            let inner = &pattern[1..pattern.len()-1];
            text.contains(inner)
        } else if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            text.ends_with(suffix)
        } else if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len()-1];
            text.starts_with(prefix)
        } else {
            text == pattern
        }
    }
}
