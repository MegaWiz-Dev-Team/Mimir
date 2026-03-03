//! Vault Secrets Management Service (Issue #157)
//!
//! HashiCorp Vault KV v2 integration with env-var fallback.
//! Features: secret resolution, key rotation, status check.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Types & Config
// ═══════════════════════════════════════════════════════════════════════════════

/// Vault connection configuration
#[derive(Debug, Clone)]
pub struct VaultConfig {
    /// Vault server address (e.g. http://localhost:8200)
    pub addr: String,
    /// Vault authentication token
    pub token: String,
    /// KV v2 mount path (default: "secret")
    pub mount: String,
    /// Secret path within mount (default: "mimir")
    pub path: String,
}

/// Vault status response
#[derive(Debug, Serialize)]
pub struct VaultStatus {
    pub enabled: bool,
    pub addr: Option<String>,
    pub connected: bool,
    pub version: Option<String>,
    pub sealed: Option<bool>,
}

/// Rotate secret request
#[derive(Debug, Deserialize)]
pub struct RotateSecretRequest {
    pub key: String,
    pub new_value: String,
}

/// Rotate secret response
#[derive(Debug, Serialize)]
pub struct RotateSecretResponse {
    pub key: String,
    pub rotated: bool,
    pub vault_version: Option<u64>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pure Functions — TDD-testable (no I/O)
// ═══════════════════════════════════════════════════════════════════════════════

/// Check if Vault integration is enabled (VAULT_ADDR is set and non-empty)
pub fn is_vault_enabled() -> bool {
    std::env::var("VAULT_ADDR")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Parse Vault configuration from environment variables.
///
/// Required: `VAULT_ADDR`, `VAULT_TOKEN`
/// Optional: `VAULT_MOUNT` (default: "secret"), `VAULT_PATH` (default: "mimir")
pub fn parse_vault_config() -> Result<VaultConfig> {
    let addr = std::env::var("VAULT_ADDR")
        .map_err(|_| anyhow::anyhow!("VAULT_ADDR not set"))?;
    
    if addr.trim().is_empty() {
        bail!("VAULT_ADDR is empty");
    }

    let token = std::env::var("VAULT_TOKEN")
        .map_err(|_| anyhow::anyhow!("VAULT_TOKEN not set"))?;

    let mount = std::env::var("VAULT_MOUNT")
        .unwrap_or_else(|_| "secret".to_string());
    
    let path = std::env::var("VAULT_PATH")
        .unwrap_or_else(|_| "mimir".to_string());

    Ok(VaultConfig { addr, token, mount, path })
}

/// Build the Vault KV v2 read URL for a given config.
///
/// Format: `{addr}/v1/{mount}/data/{path}`
/// Strips trailing slashes from addr.
pub fn build_secret_path(config: &VaultConfig) -> String {
    let addr = config.addr.trim_end_matches('/');
    let mount = config.mount.trim_matches('/');
    let path = config.path.trim_matches('/');
    format!("{}/v1/{}/data/{}", addr, mount, path)
}

/// Build the Vault KV v2 write URL (for rotation).
///
/// Same format as read: `{addr}/v1/{mount}/data/{path}`
pub fn build_write_path(config: &VaultConfig) -> String {
    // In KV v2, read and write use the same /data/ endpoint
    build_secret_path(config)
}

/// Parse a Vault KV v2 response JSON and extract a specific key.
///
/// Vault KV v2 response format:
/// ```json
/// {
///   "data": {
///     "data": { "key1": "value1", "key2": "value2" },
///     "metadata": { "version": 1 }
///   }
/// }
/// ```
pub fn parse_vault_response(response_json: &Value, key: &str) -> Result<String> {
    let data = response_json
        .get("data")
        .and_then(|d| d.get("data"))
        .ok_or_else(|| anyhow::anyhow!("Invalid Vault response: missing data.data"))?;

    let value = data
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Secret key '{}' not found in Vault", key))?;

    Ok(value.to_string())
}

/// Extract metadata version from Vault KV v2 response.
pub fn parse_vault_version(response_json: &Value) -> Option<u64> {
    response_json
        .get("data")
        .and_then(|d| d.get("metadata"))
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_u64())
}

/// Map a Mimir env var name to a Vault secret key name.
///
/// Convention: env vars are UPPER_SNAKE_CASE, vault keys are lower_snake_case.
/// Example: `GEMINI_API_KEY` → `gemini_api_key`
pub fn map_config_to_vault_key(env_name: &str) -> String {
    env_name.to_lowercase()
}

/// Build the JSON payload for writing/rotating a secret in Vault KV v2.
///
/// Format: `{ "data": { "key": "value" } }`
pub fn build_rotation_payload(key: &str, value: &str) -> Value {
    json!({
        "data": {
            key: value
        }
    })
}

/// Mask a secret value for safe logging.
///
/// Shows first 4 chars + `***` + last 2 chars.
/// Short values (≤8 chars) show `****`.
pub fn mask_secret(value: &str) -> String {
    if value.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}***{}", &value[..4], &value[value.len()-2..])
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Async Operations — Vault HTTP API calls
// ═══════════════════════════════════════════════════════════════════════════════

/// Resolve a secret: try Vault first, fall back to env var.
///
/// Returns the secret value and the source ("vault" or "env").
pub async fn resolve_secret(env_name: &str, vault_config: Option<&VaultConfig>) -> Result<(String, &'static str)> {
    // Try Vault first if configured
    if let Some(config) = vault_config {
        let vault_key = map_config_to_vault_key(env_name);
        match fetch_from_vault(config, &vault_key).await {
            Ok(value) => {
                info!(
                    key = %env_name,
                    vault_key = %vault_key,
                    masked = %mask_secret(&value),
                    "Secret resolved from Vault"
                );
                return Ok((value, "vault"));
            }
            Err(e) => {
                warn!(
                    key = %env_name,
                    error = %e,
                    "Vault lookup failed, falling back to env var"
                );
            }
        }
    }

    // Fallback to env var
    match std::env::var(env_name) {
        Ok(value) => {
            info!(key = %env_name, source = "env", "Secret resolved from environment");
            Ok((value, "env"))
        }
        Err(_) => bail!("Secret '{}' not found in Vault or environment", env_name),
    }
}

/// Fetch a secret from Vault KV v2.
async fn fetch_from_vault(config: &VaultConfig, key: &str) -> Result<String> {
    let url = build_secret_path(config);
    
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("X-Vault-Token", &config.token)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Vault connection failed: {}", e))?;

    if !response.status().is_success() {
        bail!("Vault returned status {}", response.status());
    }

    let body: Value = response.json().await
        .map_err(|e| anyhow::anyhow!("Failed to parse Vault response: {}", e))?;

    parse_vault_response(&body, key)
}

/// Check Vault server health/status.
pub async fn check_vault_status(config: &VaultConfig) -> VaultStatus {
    let addr = config.addr.trim_end_matches('/');
    let url = format!("{}/v1/sys/health", addr);

    let client = reqwest::Client::new();
    match client
        .get(&url)
        .header("X-Vault-Token", &config.token)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let body: Value = resp.json().await.unwrap_or(json!({}));
            VaultStatus {
                enabled: true,
                addr: Some(config.addr.clone()),
                connected: true,
                version: body.get("version").and_then(|v| v.as_str()).map(String::from),
                sealed: body.get("sealed").and_then(|v| v.as_bool()),
            }
        }
        Err(e) => {
            warn!(error = %e, "Vault health check failed");
            VaultStatus {
                enabled: true,
                addr: Some(config.addr.clone()),
                connected: false,
                version: None,
                sealed: None,
            }
        }
    }
}

/// Rotate a secret in Vault KV v2 (read-modify-write).
pub async fn rotate_secret(config: &VaultConfig, key: &str, new_value: &str) -> Result<RotateSecretResponse> {
    let url = build_write_path(config);

    // First, read existing secrets so we don't overwrite others
    let client = reqwest::Client::new();
    let read_resp = client
        .get(&url)
        .header("X-Vault-Token", &config.token)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    let mut secrets: serde_json::Map<String, Value> = match read_resp {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(json!({}));
            body.get("data")
                .and_then(|d| d.get("data"))
                .and_then(|d| d.as_object())
                .cloned()
                .unwrap_or_default()
        }
        _ => serde_json::Map::new(),
    };

    // Update the specific key
    secrets.insert(key.to_string(), json!(new_value));

    // Write back
    let write_body = json!({ "data": secrets });
    let write_resp = client
        .post(&url)
        .header("X-Vault-Token", &config.token)
        .json(&write_body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Vault write failed: {}", e))?;

    if !write_resp.status().is_success() {
        bail!("Vault write returned status {}", write_resp.status());
    }

    let resp_body: Value = write_resp.json().await.unwrap_or(json!({}));
    let version = parse_vault_version(&resp_body);

    info!(
        key = %key,
        masked_value = %mask_secret(new_value),
        version = ?version,
        "Secret rotated in Vault"
    );

    Ok(RotateSecretResponse {
        key: key.to_string(),
        rotated: true,
        vault_version: version,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests — Pure function tests (no Vault server required)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> VaultConfig {
        VaultConfig {
            addr: "http://localhost:8200".to_string(),
            token: "test-token".to_string(),
            mount: "secret".to_string(),
            path: "mimir".to_string(),
        }
    }

    // ========================================
    // UT-017a: build_secret_path — correct URL
    // ========================================
    #[test]
    fn test_build_secret_path() {
        let config = test_config();
        let url = build_secret_path(&config);
        assert_eq!(url, "http://localhost:8200/v1/secret/data/mimir");
    }

    #[test]
    fn test_build_secret_path_custom_mount() {
        let config = VaultConfig {
            addr: "https://vault.example.com".to_string(),
            token: "t".to_string(),
            mount: "kv".to_string(),
            path: "prod/mimir".to_string(),
        };
        assert_eq!(
            build_secret_path(&config),
            "https://vault.example.com/v1/kv/data/prod/mimir"
        );
    }

    // ========================================
    // UT-017b: build_secret_path — strips trailing slashes
    // ========================================
    #[test]
    fn test_build_secret_path_strips_slashes() {
        let config = VaultConfig {
            addr: "http://vault:8200/".to_string(),
            token: "t".to_string(),
            mount: "/secret/".to_string(),
            path: "/mimir/".to_string(),
        };
        assert_eq!(
            build_secret_path(&config),
            "http://vault:8200/v1/secret/data/mimir"
        );
    }

    // ========================================
    // UT-017c: VaultConfig struct construction
    // ========================================
    #[test]
    fn test_vault_config_construction() {
        let config = VaultConfig {
            addr: "http://test:8200".to_string(),
            token: "test-token-123".to_string(),
            mount: "kv2".to_string(),
            path: "myapp".to_string(),
        };
        assert_eq!(config.addr, "http://test:8200");
        assert_eq!(config.token, "test-token-123");
        assert_eq!(config.mount, "kv2");
        assert_eq!(config.path, "myapp");
    }

    // ========================================
    // UT-017d: VaultConfig defaults convention
    // ========================================
    #[test]
    fn test_vault_config_default_values() {
        // Verify default mount and path match expected values
        let config = VaultConfig {
            addr: "http://v:8200".to_string(),
            token: "tok".to_string(),
            mount: "secret".to_string(),   // default
            path: "mimir".to_string(),     // default
        };
        let url = build_secret_path(&config);
        assert!(url.contains("/secret/data/mimir"));
    }

    // ========================================
    // UT-017e: build_write_path matches read path
    // ========================================
    #[test]
    fn test_build_write_path_matches_read() {
        let config = test_config();
        assert_eq!(build_write_path(&config), build_secret_path(&config));
    }

    // ========================================
    // UT-017f: parse_vault_response — extract key
    // ========================================
    #[test]
    fn test_parse_vault_response_success() {
        let response = json!({
            "data": {
                "data": {
                    "gemini_api_key": "AIza-test-key",
                    "jwt_secret": "my-jwt-secret"
                },
                "metadata": {
                    "version": 3
                }
            }
        });

        let val = parse_vault_response(&response, "gemini_api_key").unwrap();
        assert_eq!(val, "AIza-test-key");

        let jwt = parse_vault_response(&response, "jwt_secret").unwrap();
        assert_eq!(jwt, "my-jwt-secret");
    }

    // ========================================
    // UT-017g: parse_vault_response — missing key
    // ========================================
    #[test]
    fn test_parse_vault_response_missing_key() {
        let response = json!({
            "data": {
                "data": { "existing_key": "value" },
                "metadata": { "version": 1 }
            }
        });

        let result = parse_vault_response(&response, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_parse_vault_response_invalid_format() {
        let response = json!({ "error": "permission denied" });
        assert!(parse_vault_response(&response, "key").is_err());
    }

    #[test]
    fn test_parse_vault_version() {
        let response = json!({
            "data": {
                "data": {},
                "metadata": { "version": 5 }
            }
        });
        assert_eq!(parse_vault_version(&response), Some(5));
    }

    #[test]
    fn test_parse_vault_version_missing() {
        let response = json!({});
        assert_eq!(parse_vault_version(&response), None);
    }

    // ========================================
    // UT-017h: map_config_to_vault_key
    // ========================================
    #[test]
    fn test_map_config_to_vault_key() {
        assert_eq!(map_config_to_vault_key("GEMINI_API_KEY"), "gemini_api_key");
        assert_eq!(map_config_to_vault_key("JWT_SECRET"), "jwt_secret");
        assert_eq!(map_config_to_vault_key("S3_ACCESS_KEY"), "s3_access_key");
    }

    // ========================================
    // UT-017i: build_rotation_payload
    // ========================================
    #[test]
    fn test_build_rotation_payload() {
        let payload = build_rotation_payload("api_key", "new-secret-value");
        let data = payload.get("data").unwrap();
        assert_eq!(data.get("api_key").unwrap().as_str().unwrap(), "new-secret-value");
    }

    // ========================================
    // UT-017j: mask_secret
    // ========================================
    #[test]
    fn test_mask_secret_long() {
        assert_eq!(mask_secret("my-super-secret-key-123"), "my-s***23");
    }

    #[test]
    fn test_mask_secret_short() {
        assert_eq!(mask_secret("short"), "****");
    }

    #[test]
    fn test_mask_secret_exactly_eight() {
        assert_eq!(mask_secret("12345678"), "****");
    }

    #[test]
    fn test_mask_secret_nine_chars() {
        assert_eq!(mask_secret("123456789"), "1234***89");
    }

    // ========================================
    // UT-017k: VAULT_MANAGED_SECRETS constant
    // ========================================
    #[test]
    fn test_vault_managed_secrets_contains_expected_keys() {
        use crate::config::VAULT_MANAGED_SECRETS;

        // Verify all expected secret keys are in the managed list
        let expected = [
            "GEMINI_API_KEY",
            "GITHUB_TOKEN",
            "HEIMDALL_API_KEY",
            "JWT_SECRET",
            "S3_ACCESS_KEY",
            "S3_SECRET_KEY",
        ];

        for key in &expected {
            assert!(
                VAULT_MANAGED_SECRETS.contains(key),
                "VAULT_MANAGED_SECRETS should contain '{}'",
                key
            );
        }
        assert_eq!(VAULT_MANAGED_SECRETS.len(), expected.len());
    }

    // ========================================
    // UT-017l: VAULT_MANAGED_SECRETS → Vault key mapping
    // ========================================
    #[test]
    fn test_vault_managed_secrets_map_to_valid_vault_keys() {
        use crate::config::VAULT_MANAGED_SECRETS;

        // Each managed secret should map to a lowercase vault key
        let expected_mappings = [
            ("GEMINI_API_KEY", "gemini_api_key"),
            ("GITHUB_TOKEN", "github_token"),
            ("HEIMDALL_API_KEY", "heimdall_api_key"),
            ("JWT_SECRET", "jwt_secret"),
            ("S3_ACCESS_KEY", "s3_access_key"),
            ("S3_SECRET_KEY", "s3_secret_key"),
        ];

        for (env_key, vault_key) in &expected_mappings {
            assert!(VAULT_MANAGED_SECRETS.contains(env_key));
            assert_eq!(
                map_config_to_vault_key(env_key),
                *vault_key,
                "Mapping for {} should be {}",
                env_key,
                vault_key
            );
        }
    }

    // ========================================
    // UT-017m: VAULT_MANAGED_SECRETS — no non-secret keys
    // ========================================
    #[test]
    fn test_vault_managed_secrets_excludes_non_secrets() {
        use crate::config::VAULT_MANAGED_SECRETS;

        // These are configuration values, NOT secrets — should NOT be in the list
        let non_secret_keys = [
            "PORT",
            "MARIADB_URL",
            "QDRANT_URL",
            "REDIS_URL",
            "S3_ENDPOINT",
            "S3_BUCKET",
            "S3_REGION",
            "OLLAMA_URL",
            "LOCAL_MODEL",
            "RUST_LOG",
            "CRON_TICK_SECONDS",
        ];

        for key in &non_secret_keys {
            assert!(
                !VAULT_MANAGED_SECRETS.contains(key),
                "VAULT_MANAGED_SECRETS should NOT contain non-secret key '{}'",
                key
            );
        }
    }

    // ========================================
    // UT-017n: inject_vault_secrets — no-op when Vault disabled
    // ========================================
    #[tokio::test]
    async fn test_inject_vault_secrets_noop_without_vault() {
        // When VAULT_ADDR is not set, inject should be a no-op
        // (does not panic, does not modify env)
        unsafe { std::env::remove_var("VAULT_ADDR"); }

        let test_key = "GEMINI_API_KEY";
        let original = std::env::var(test_key).ok();

        crate::config::inject_vault_secrets().await;

        // Value should remain unchanged
        assert_eq!(std::env::var(test_key).ok(), original);
    }
}
