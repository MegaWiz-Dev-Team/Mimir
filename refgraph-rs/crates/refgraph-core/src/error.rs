//! RefGraph error types and handling

use thiserror::Error;

/// RefGraph result type
pub type Result<T> = std::result::Result<T, Error>;

/// RefGraph errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Extraction error: {0}")]
    ExtractionError(String),

    #[error("Deduplication error: {0}")]
    DeduplicationError(String),

    #[error("Graph error: {0}")]
    GraphError(String),

    #[error("Manifest error: {0}")]
    ManifestError(String),

    #[error("Mimir output error: {0}")]
    MimirOutputError(String),

    #[error("Entity error: {0}")]
    EntityError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Neo4j error: {0}")]
    Neo4jError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl Error {
    /// Create extraction error
    pub fn extraction(msg: impl Into<String>) -> Self {
        Self::ExtractionError(msg.into())
    }

    /// Create deduplication error
    pub fn dedup(msg: impl Into<String>) -> Self {
        Self::DeduplicationError(msg.into())
    }

    /// Create graph error
    pub fn graph(msg: impl Into<String>) -> Self {
        Self::GraphError(msg.into())
    }
}
