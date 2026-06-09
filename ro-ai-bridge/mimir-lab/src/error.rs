//! Error type for mimir-lab.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LabError {
    #[error("duckdb: {0}")]
    Duck(#[from] duckdb::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// A query that is not a read-only statement was passed to a read-only path.
    #[error("read-only violation: only SELECT/WITH/DESCRIBE/SUMMARIZE/EXPLAIN/PRAGMA/SHOW allowed (got: {0})")]
    NotReadOnly(String),

    /// An identifier (table/column name) failed validation.
    #[error("invalid identifier: {0}")]
    BadIdent(String),

    #[error("ingest: {0}")]
    Ingest(String),

    /// A query was interrupted for exceeding its wall-clock budget.
    #[error("query timeout: {0}")]
    Timeout(String),

    #[error("storage: {0}")]
    Storage(String),

    /// API-layer error (bad column reference, unsupported option, etc.).
    #[error("api: {0}")]
    Api(String),
}

pub type Result<T> = std::result::Result<T, LabError>;

/// Validate a SQL identifier (table/column name) we interpolate into DDL.
/// Conservative: ASCII alnum + underscore, must not start with a digit.
pub fn valid_ident(s: &str) -> Result<()> {
    let ok = !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.chars().next().unwrap().is_ascii_digit();
    if ok {
        Ok(())
    } else {
        Err(LabError::BadIdent(s.to_string()))
    }
}
