//! Neo4j configuration

use std::env;

/// Neo4j database configuration
#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    /// Bolt URI (e.g., "bolt://localhost:7687")
    pub uri: String,
    /// Username (default: "neo4j")
    pub user: String,
    /// Password
    pub password: String,
}

impl Neo4jConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            uri: env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string()),
            user: env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string()),
            password: env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "neo4j".to_string()),
        }
    }

    /// Create configuration with specific values
    pub fn new(
        uri: impl Into<String>,
        user: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            user: user.into(),
            password: password.into(),
        }
    }
}

impl Default for Neo4jConfig {
    fn default() -> Self {
        Self::from_env()
    }
}
