//! Performance optimization — connection pool config and in-memory TTL cache.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════════
// Pool Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Database connection pool configuration from environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub max_lifetime_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 2,
            acquire_timeout_secs: 30,
            idle_timeout_secs: 300,
            max_lifetime_secs: 1800,
        }
    }
}

impl PoolConfig {
    /// Parse pool config from environment variables.
    pub fn from_env() -> Self {
        Self {
            max_connections: std::env::var("DB_POOL_MAX")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            min_connections: std::env::var("DB_POOL_MIN")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            acquire_timeout_secs: std::env::var("DB_POOL_ACQUIRE_TIMEOUT")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            idle_timeout_secs: std::env::var("DB_POOL_IDLE_TIMEOUT")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(300),
            max_lifetime_secs: std::env::var("DB_POOL_MAX_LIFETIME")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(1800),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// In-Memory TTL Cache
// ═══════════════════════════════════════════════════════════════════════════════

/// A single cache entry with expiry.
#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    expires_at: Instant,
}

/// Simple in-memory TTL cache for expensive read-only queries.
/// Thread-safe via interior mutability (designed for single-threaded test scenarios;
/// for production use with concurrent Axum handlers, wrap in Arc<Mutex<>>).
#[derive(Debug)]
pub struct InMemoryCache {
    entries: HashMap<String, CacheEntry>,
    default_ttl: Duration,
}

impl InMemoryCache {
    /// Create a new cache with default TTL.
    pub fn new(default_ttl_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            default_ttl: Duration::from_secs(default_ttl_secs),
        }
    }

    /// Get a cached value if it exists and hasn't expired.
    pub fn get(&self, key: &str) -> Option<String> {
        self.entries.get(key).and_then(|entry| {
            if Instant::now() < entry.expires_at {
                Some(entry.value.clone())
            } else {
                None
            }
        })
    }

    /// Insert a value with the default TTL.
    pub fn set(&mut self, key: String, value: String) {
        self.entries.insert(key, CacheEntry {
            value,
            expires_at: Instant::now() + self.default_ttl,
        });
    }

    /// Insert a value with a custom TTL.
    pub fn set_with_ttl(&mut self, key: String, value: String, ttl_secs: u64) {
        self.entries.insert(key, CacheEntry {
            value,
            expires_at: Instant::now() + Duration::from_secs(ttl_secs),
        });
    }

    /// Remove expired entries to free memory.
    pub fn evict_expired(&mut self) -> usize {
        let now = Instant::now();
        let before = self.entries.len();
        self.entries.retain(|_, entry| now < entry.expires_at);
        before - self.entries.len()
    }

    /// Get number of active (non-expired) entries.
    pub fn len(&self) -> usize {
        let now = Instant::now();
        self.entries.values().filter(|e| now < e.expires_at).count()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get cache stats.
    pub fn stats(&self) -> CacheStats {
        let now = Instant::now();
        let total = self.entries.len();
        let active = self.entries.values().filter(|e| now < e.expires_at).count();
        CacheStats {
            total_entries: total,
            active_entries: active,
            expired_entries: total - active,
            default_ttl_secs: self.default_ttl.as_secs(),
        }
    }
}

/// Cache statistics.
#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub expired_entries: usize,
    pub default_ttl_secs: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Hot Query Index Recommendations
// ═══════════════════════════════════════════════════════════════════════════════

/// Returns recommended indexes for hot query patterns.
pub fn recommended_indexes() -> Vec<IndexRecommendation> {
    vec![
        IndexRecommendation {
            table: "data_sources".into(),
            columns: "tenant_id, source_type".into(),
            reason: "WHERE tenant_id = ? AND source_type = ? (sources list)".into(),
        },
        IndexRecommendation {
            table: "pipeline_runs".into(),
            columns: "tenant_id, created_at DESC".into(),
            reason: "ORDER BY created_at DESC (pipeline status)".into(),
        },
        IndexRecommendation {
            table: "llm_usage_logs".into(),
            columns: "tenant_id, created_at DESC".into(),
            reason: "Analytics queries grouped by tenant + time".into(),
        },
        IndexRecommendation {
            table: "agent_configs".into(),
            columns: "tenant_id".into(),
            reason: "Agent list filtered by tenant".into(),
        },
        IndexRecommendation {
            table: "agent_conversations".into(),
            columns: "agent_id, created_at DESC".into(),
            reason: "Conversation history per agent".into(),
        },
    ]
}

/// A database index recommendation.
#[derive(Debug, Serialize)]
pub struct IndexRecommendation {
    pub table: String,
    pub columns: String,
    pub reason: String,
}

/// Generate SQL for creating all recommended indexes.
pub fn generate_index_sql() -> String {
    recommended_indexes().iter().map(|idx| {
        let index_name = format!("idx_{}_{}", idx.table, idx.columns.replace(", ", "_").replace(" DESC", ""));
        format!("CREATE INDEX IF NOT EXISTS {} ON {} ({});", index_name, idx.table, idx.columns)
    }).collect::<Vec<_>>().join("\n")
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // UT-014ca: PoolConfig — default values
    // ========================================
    #[test]
    fn test_pool_config_defaults() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.acquire_timeout_secs, 30);
        assert_eq!(config.idle_timeout_secs, 300);
        assert_eq!(config.max_lifetime_secs, 1800);
    }

    // ========================================
    // UT-014cb: PoolConfig — from_env with defaults
    // ========================================
    #[test]
    fn test_pool_config_from_env_defaults() {
        let config = PoolConfig::from_env();
        // Without env vars set, should use defaults
        assert!(config.max_connections >= 1);
        assert!(config.min_connections >= 1);
    }

    // ========================================
    // UT-014cc: Cache — set and get
    // ========================================
    #[test]
    fn test_cache_set_and_get() {
        let mut cache = InMemoryCache::new(60);
        cache.set("key1".into(), "value1".into());
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
    }

    // ========================================
    // UT-014cd: Cache — miss for unknown key
    // ========================================
    #[test]
    fn test_cache_miss() {
        let cache = InMemoryCache::new(60);
        assert_eq!(cache.get("nonexistent"), None);
    }

    // ========================================
    // UT-014ce: Cache — expiry
    // ========================================
    #[test]
    fn test_cache_expiry() {
        let mut cache = InMemoryCache::new(60);
        // Use a custom TTL of 0 seconds to test immediate expiry
        cache.set_with_ttl("expire_key".into(), "value".into(), 0);
        // After 0 seconds, should be expired
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(cache.get("expire_key"), None);
    }

    // ========================================
    // UT-014cf: Cache — stats
    // ========================================
    #[test]
    fn test_cache_stats() {
        let mut cache = InMemoryCache::new(60);
        cache.set("k1".into(), "v1".into());
        cache.set("k2".into(), "v2".into());
        let stats = cache.stats();
        assert_eq!(stats.active_entries, 2);
        assert_eq!(stats.default_ttl_secs, 60);
    }

    // ========================================
    // UT-014cg: Cache — evict expired
    // ========================================
    #[test]
    fn test_cache_evict_expired() {
        let mut cache = InMemoryCache::new(60);
        cache.set("active".into(), "v1".into());
        cache.set_with_ttl("expired".into(), "v2".into(), 0);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let evicted = cache.evict_expired();
        assert_eq!(evicted, 1);
        assert_eq!(cache.get("active"), Some("v1".to_string()));
    }

    // ========================================
    // UT-014ch: Cache — clear all
    // ========================================
    #[test]
    fn test_cache_clear() {
        let mut cache = InMemoryCache::new(60);
        cache.set("k1".into(), "v1".into());
        cache.set("k2".into(), "v2".into());
        cache.clear();
        assert!(cache.is_empty());
    }

    // ========================================
    // UT-014ci: recommended_indexes — returns entries
    // ========================================
    #[test]
    fn test_recommended_indexes() {
        let indexes = recommended_indexes();
        assert!(indexes.len() >= 5);
        assert!(indexes.iter().any(|i| i.table == "data_sources"));
        assert!(indexes.iter().any(|i| i.table == "llm_usage_logs"));
    }

    // ========================================
    // UT-014cj: generate_index_sql — valid SQL
    // ========================================
    #[test]
    fn test_generate_index_sql() {
        let sql = generate_index_sql();
        assert!(sql.contains("CREATE INDEX IF NOT EXISTS"));
        assert!(sql.contains("data_sources"));
        assert!(sql.contains("pipeline_runs"));
    }
}
