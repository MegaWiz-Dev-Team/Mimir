use std::env;
use dotenvy::dotenv;
use tracing::info;

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
