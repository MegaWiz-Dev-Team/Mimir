//! Dataset ingestion. MVP formats: CSV, Parquet, JSON — via DuckDB readers,
//! with schema inference and (optional) Parquet materialization.
//! GeoJSON / Excel land in P4 (need the spatial / excel extensions).

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

/// Supported MVP source formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    Csv,
    Parquet,
    Json,
}

impl SourceFormat {
    /// Infer the format from a path's extension.
    pub fn from_path(path: &str) -> Result<Self> {
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "csv" | "tsv" => Ok(Self::Csv),
            "parquet" | "pq" => Ok(Self::Parquet),
            "json" | "ndjson" | "jsonl" => Ok(Self::Json),
            other => Err(LabError::Ingest(format!(
                "unsupported format '.{other}' (GeoJSON/Excel land in P4)"
            ))),
        }
    }

    /// The DuckDB table function that reads this format.
    fn reader_sql(self, path: &str) -> String {
        let p = sql_str(path);
        match self {
            Self::Csv => format!("read_csv_auto('{p}')"),
            Self::Parquet => format!("read_parquet('{p}')"),
            Self::Json => format!("read_json_auto('{p}')"),
        }
    }
}

/// Single-quote escape for a string literal interpolated into DuckDB SQL.
fn sql_str(s: &str) -> String {
    s.replace('\'', "''")
}

/// Materialize `reader_sql` into `table` and return its inferred shape.
fn ingest_reader(engine: &Engine, table: &str, reader_sql: &str, src: &str) -> Result<IngestResult> {
    engine.execute(&format!(
        "CREATE OR REPLACE TABLE {table} AS SELECT * FROM {reader_sql}"
    ))?;
    let row_count = engine.query_scalar_u64(&format!("SELECT COUNT(*) FROM {table}"))?;
    let schema = engine.describe(&format!("SELECT * FROM {table}"))?;
    if schema.columns.is_empty() {
        return Err(LabError::Ingest(format!("no columns inferred from {src}")));
    }
    Ok(IngestResult {
        table: table.to_string(),
        row_count,
        schema,
        parquet_path: None,
    })
}

/// Ingest a file, picking the reader by extension (CSV / Parquet / JSON).
///
/// `path` is a trusted local path (operator/agent-tool supplied through a
/// validated registry, not raw user input). `table` is validated as an
/// identifier.
pub fn ingest_file(engine: &Engine, path: &str, table: &str) -> Result<IngestResult> {
    valid_ident(table)?;
    let fmt = SourceFormat::from_path(path)?;
    ingest_reader(engine, table, &fmt.reader_sql(path), path)
}

/// Ingest a CSV file (format forced regardless of extension).
pub fn ingest_csv(engine: &Engine, csv_path: &str, table: &str) -> Result<IngestResult> {
    valid_ident(table)?;
    ingest_reader(engine, table, &SourceFormat::Csv.reader_sql(csv_path), csv_path)
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
