//! Dataset ingestion. MVP: CSV via DuckDB `read_csv_auto`, with schema
//! inference and (optional) Parquet materialization.

use crate::engine::Engine;
use crate::error::{valid_ident, LabError, Result};
use crate::schema::TableSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub table: String,
    pub row_count: u64,
    pub schema: TableSchema,
    /// Set when the table was also written out to Parquet.
    pub parquet_path: Option<String>,
}

/// Single-quote escape for a string literal interpolated into DuckDB SQL.
fn sql_str(s: &str) -> String {
    s.replace('\'', "''")
}

/// Ingest a CSV file into `table` inside the engine, inferring its schema.
///
/// `csv_path` is a trusted local path (operator/agent-tool supplied through a
/// validated registry, not raw user input). `table` is validated as an
/// identifier.
pub fn ingest_csv(engine: &Engine, csv_path: &str, table: &str) -> Result<IngestResult> {
    valid_ident(table)?;
    engine.execute(&format!(
        "CREATE OR REPLACE TABLE {table} AS SELECT * FROM read_csv_auto('{}')",
        sql_str(csv_path)
    ))?;
    let row_count = engine.query_scalar_u64(&format!("SELECT COUNT(*) FROM {table}"))?;
    let schema = engine.describe(&format!("SELECT * FROM {table}"))?;
    if schema.columns.is_empty() {
        return Err(LabError::Ingest(format!(
            "no columns inferred from {csv_path}"
        )));
    }
    Ok(IngestResult {
        table: table.to_string(),
        row_count,
        schema,
        parquet_path: None,
    })
}

/// Write an existing table out to Parquet (catalog materialization).
pub fn export_parquet(engine: &Engine, table: &str, parquet_path: &str) -> Result<()> {
    valid_ident(table)?;
    engine.execute(&format!(
        "COPY {table} TO '{}' (FORMAT PARQUET)",
        sql_str(parquet_path)
    ))?;
    Ok(())
}
