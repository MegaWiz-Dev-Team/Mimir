use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use tracing::{info, warn};

use crate::services::vault;

// ═══════════════════════════════════════════════════════════════════════════════
// Vault Secret Injection (Issue #157)
// ═══════════════════════════════════════════════════════════════════════════════

/// Secret env var names that should be resolved from Vault when available.
pub const VAULT_MANAGED_SECRETS: &[&str] = &[
    "GITHUB_TOKEN",
    "HEIMDALL_API_KEY",
    "JWT_SECRET",
    "NCBI_API_KEY",
    "NEO4J_PASSWORD",
    "S3_ACCESS_KEY",
    "S3_SECRET_KEY",
    "YGGDRASIL_CLIENT_ID",
    "YGGDRASIL_CLIENT_SECRET",
    "YGGDRASIL_ISSUER",
    "YGGDRASIL_REDIRECT_URI",
];

/// Inject Vault secrets into process environment variables.
///
/// Call this **once** at startup, **before** `Config::from_env()`.
/// For each key in `VAULT_MANAGED_SECRETS`, tries Vault first;
/// if found, sets it via `std::env::set_var` so all subsequent
/// `env::var()` calls throughout the codebase pick it up.
///
/// If Vault is not configured (`VAULT_ADDR` not set), this is a no-op.
pub async fn inject_vault_secrets() {
    if !vault::is_vault_enabled() {
        info!(
            event = "vault_skip",
            "Vault not configured — using env vars directly for secrets"
        );
        return;
    }

    let config = match vault::parse_vault_config() {
        Ok(c) => c,
        Err(e) => {
            warn!(event = "vault_config_error", error = %e, "Could not parse Vault config — falling back to env vars");
            return;
        }
    };

    info!(
        event = "vault_inject_start",
        "Resolving secrets from Vault..."
    );
    let mut injected = 0u32;

    for key in VAULT_MANAGED_SECRETS {
        match vault::resolve_secret(key, Some(&config)).await {
            Ok((value, source)) => {
                if source == "vault" {
                    // SAFETY: set_var is safe here because we're single-threaded at startup
                    unsafe {
                        env::set_var(key, &value);
                    }
                    info!(event = "vault_injected", key = %key, "✅ Injected from Vault");
                    injected += 1;
                }
                // source == "env" means it was already in env, no need to set
            }
            Err(e) => {
                warn!(event = "vault_resolve_fail", key = %key, error = %e, "Could not resolve — will use env var if present");
            }
        }
    }

    info!(
        event = "vault_inject_done",
        injected = injected,
        total = VAULT_MANAGED_SECRETS.len(),
        "Vault secret injection complete"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// HS256 JWT secret — refuse public values
// ═══════════════════════════════════════════════════════════════════════════════

/// Opt-in for a local dev box or a test run: lets the server start on the old
/// public default (or on a template placeholder). Never set it on a deployment.
pub const ALLOW_INSECURE_DEV_JWT_ENV: &str = "MIMIR_ALLOW_INSECURE_DEV_JWT";

/// The old built-in default. This repository is public, so anyone can sign an
/// HS256 token with it. Used only behind `MIMIR_ALLOW_INSECURE_DEV_JWT=1`.
pub const INSECURE_DEV_JWT_SECRET: &str = "dev_secret_key";

/// JWT_SECRET values published in this repository's code, templates and docs.
/// A server running on any of them accepts tokens anyone can forge.
pub const PUBLIC_JWT_SECRETS: &[&str] = &[
    INSECURE_DEV_JWT_SECRET,
    "JWT_REDACTED",
    "change_me_to_a_secure_random_string",
    "change-me-to-a-random-string-at-least-32-chars",
    "your_jwt_secret_here",
];

/// Why a JWT_SECRET was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JwtSecretError {
    /// JWT_SECRET is unset or blank.
    Missing,
    /// JWT_SECRET is the old default or a placeholder published in this repo.
    PublicValue,
}

impl std::fmt::Display for JwtSecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let problem = match self {
            JwtSecretError::Missing => "JWT_SECRET is not set (or is empty)",
            JwtSecretError::PublicValue => {
                "JWT_SECRET is a public value ('dev_secret_key' or a placeholder from this repository)"
            }
        };
        write!(
            f,
            "{problem}. Refusing to start: anyone could forge HS256 tokens for this server. \
             Set JWT_SECRET to a private random value (for example `openssl rand -hex 32`; \
             in the cluster: secret asgard/asgard-secrets, key JWT_SECRET). \
             On a local dev box only, {ALLOW_INSECURE_DEV_JWT_ENV}=1 allows the insecure default."
        )
    }
}

impl std::error::Error for JwtSecretError {}

/// True when `secret` is one of [`PUBLIC_JWT_SECRETS`].
pub fn is_public_jwt_secret(secret: &str) -> bool {
    PUBLIC_JWT_SECRETS.contains(&secret.trim())
}

/// True when `MIMIR_ALLOW_INSECURE_DEV_JWT` is `1`, `true` or `yes`.
pub fn allow_insecure_dev_jwt() -> bool {
    env::var(ALLOW_INSECURE_DEV_JWT_ENV)
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

/// Decide the HS256 secret from a JWT_SECRET value and the dev opt-in.
/// Pure (reads no env) so the rule is unit-tested directly.
pub fn resolve_jwt_secret(
    jwt_secret: Option<&str>,
    allow_insecure_dev: bool,
) -> Result<String, JwtSecretError> {
    match jwt_secret.filter(|v| !v.trim().is_empty()) {
        None if allow_insecure_dev => Ok(INSECURE_DEV_JWT_SECRET.to_string()),
        None => Err(JwtSecretError::Missing),
        Some(v) if is_public_jwt_secret(v) && !allow_insecure_dev => {
            Err(JwtSecretError::PublicValue)
        }
        Some(v) => Ok(v.to_string()),
    }
}

/// The HS256 secret from `JWT_SECRET`, refusing a missing or public value
/// unless `MIMIR_ALLOW_INSECURE_DEV_JWT=1`. Every binary reads it through here.
pub fn jwt_secret_from_env() -> Result<String, JwtSecretError> {
    resolve_jwt_secret(
        env::var("JWT_SECRET").ok().as_deref(),
        allow_insecure_dev_jwt(),
    )
}

/// Startup warning for the opt-in case: the process runs on a public secret.
pub fn warn_if_insecure_dev_jwt(secret: &str, binary: &str) {
    if is_public_jwt_secret(secret) {
        warn!(
            event = "insecure_jwt_secret_default",
            binary = binary,
            "JWT_SECRET is a public value, allowed only because {ALLOW_INSECURE_DEV_JWT_ENV}=1 — \
             anyone can forge tokens for this process; never run it like this outside a dev box"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════

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
    pub heimdall_api_url: String,
    pub heimdall_api_key: Option<String>,
    pub heimdall_model: String,

    // Auth
    pub jwt_secret: String,
}

impl Config {
    /// Fails when JWT_SECRET is missing or public (see [`jwt_secret_from_env`]).
    pub fn from_env() -> Result<Self, JwtSecretError> {
        dotenv().ok(); // Load .env file if it exists

        info!("Loading configuration from environment...");

        let config = Self {
            // Server
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),

            // Database
            mariadb_url: env::var("MARIADB_URL").unwrap_or_else(|_| {
                "mysql://mimir:REDACTED-PW@localhost:3306/mimir".to_string()
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

            // LLM (All traffic routed through Heimdall Gateway)
            heimdall_api_url: env::var("HEIMDALL_API_URL")
                .unwrap_or_else(|_| "http://localhost:3000/v1".to_string()),
            heimdall_api_key: env::var("HEIMDALL_API_KEY").ok(),
            heimdall_model: env::var("HEIMDALL_MODEL")
                .unwrap_or_else(|_| "mlx-community/Qwen3.5-35B-A3B-4bit".to_string()),

            // Auth
            jwt_secret: jwt_secret_from_env()?,
        };

        info!("Configuration loaded successfully.");
        Ok(config)
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
            let inner = &pattern[1..pattern.len() - 1];
            text.contains(inner)
        } else if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            text.ends_with(suffix)
        } else if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            text.starts_with(prefix)
        } else {
            text == pattern
        }
    }
}
