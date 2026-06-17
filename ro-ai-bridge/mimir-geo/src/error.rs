//! Error type for mimir-geo. Mirrors `mimir-lab::error` so the analytics-api can
//! map both engines' failures uniformly.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeoError {
    #[error("geometry: {0}")]
    Geometry(String),
    #[error("h3: {0}")]
    H3(String),
    #[error("ingest: {0}")]
    Ingest(String),
    #[error("stats: {0}")]
    Stats(String),
    #[error("invalid request: {0}")]
    BadRequest(String),
}

pub type Result<T> = std::result::Result<T, GeoError>;
