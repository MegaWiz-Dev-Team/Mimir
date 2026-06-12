//! Error type for mimir-geo. Mirrors `mimir-lab::error` so the analytics-api can
//! map both engines' failures uniformly.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeoError {
    #[error("duckdb: {0}")]
    Duck(String),
    #[error("geometry: {0}")]
    Geometry(String),
    #[error("h3: {0}")]
    H3(String),
    /// The Python spatial-stats sandbox failed, timed out, or was busy (serialized).
    #[error("python sandbox: {0}")]
    Sandbox(String),
    #[error("ingest: {0}")]
    Ingest(String),
    /// A write/DDL statement reached the read-only guard.
    #[error("read-only violation: {0}")]
    ReadOnly(String),
    #[error("invalid request: {0}")]
    BadRequest(String),
}

pub type Result<T> = std::result::Result<T, GeoError>;
