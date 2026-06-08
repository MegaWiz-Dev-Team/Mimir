//! DuckDB engine wrapper.
//!
//! Two design choices keep this robust across DuckDB crate versions and safe by
//! default:
//!   1. **Read-only guard** — [`Engine::query_readonly`] rejects anything that is
//!      not a SELECT/WITH/DESCRIBE/SUMMARIZE/EXPLAIN/PRAGMA/SHOW statement, so an
//!      agent tool call can never mutate via this path.
//!   2. **CAST-to-VARCHAR fetch** — rather than match every `ValueRef` variant
//!      (which churns between DuckDB versions), we `CAST` every projected column
//!      to VARCHAR in SQL and read uniformly as `Option<String>`. Values come
//!      back as their text form; callers re-type using the column schema.

use crate::error::{LabError, Result};
use crate::schema::{ColumnSchema, TableSchema};
use duckdb::Connection;

pub struct Engine {
    conn: Connection,
}

/// Result of a capped read-only query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    /// Rows of stringified cell values (NULL → `None`).
    pub rows: Vec<Vec<Option<String>>>,
    /// True if more rows existed beyond `row_cap` and were dropped.
    pub truncated: bool,
}

impl Engine {
    /// In-memory engine (ingest scratch space, tests).
    pub fn in_memory() -> Result<Self> {
        Ok(Self {
            conn: Connection::open_in_memory()?,
        })
    }

    /// File-backed engine (a tenant's persistent catalog db).
    pub fn open(path: &str) -> Result<Self> {
        Ok(Self {
            conn: Connection::open(path)?,
        })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run DDL/DML (used by ingest). Returns affected-row count where applicable.
    pub fn execute(&self, sql: &str) -> Result<usize> {
        Ok(self.conn.execute(sql, [])?)
    }

    /// Single scalar `u64` (e.g. `SELECT COUNT(*) ...`).
    pub fn query_scalar_u64(&self, sql: &str) -> Result<u64> {
        let n: i64 = self.conn.query_row(sql, [], |r| r.get(0))?;
        Ok(n.max(0) as u64)
    }

    /// Infer the schema of an arbitrary SELECT via `DESCRIBE`.
    pub fn describe(&self, select_sql: &str) -> Result<TableSchema> {
        let describe = format!("DESCRIBE {select_sql}");
        let mut stmt = self.conn.prepare(&describe)?;
        let mut rows = stmt.query([])?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next()? {
            // DESCRIBE columns: column_name, column_type, null, key, default, extra
            let name: String = row.get(0)?;
            let sql_type: String = row.get(1)?;
            let null_flag: Option<String> = row.get(2).ok().flatten();
            let nullable = null_flag
                .map(|s| s.eq_ignore_ascii_case("YES"))
                .unwrap_or(true);
            columns.push(ColumnSchema {
                name,
                sql_type,
                nullable,
            });
        }
        Ok(TableSchema { columns })
    }

    /// Run a **read-only** query, capped at `row_cap` rows.
    pub fn query_readonly(&self, sql: &str, row_cap: usize) -> Result<QueryResult> {
        guard_read_only(sql)?;
        let schema = self.describe(sql)?;

        // Project every column CAST to VARCHAR so we can fetch uniformly as text.
        let projection = schema
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| format!("CAST(t.\"{}\" AS VARCHAR) AS c{i}", c.name.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(", ");
        // Fetch one extra row to detect truncation.
        let wrapped = format!(
            "SELECT {projection} FROM ({sql}) AS t LIMIT {}",
            row_cap.saturating_add(1)
        );

        let ncols = schema.columns.len();
        let mut stmt = self.conn.prepare(&wrapped)?;
        let mut rows = stmt.query([])?;
        let mut out: Vec<Vec<Option<String>>> = Vec::new();
        while let Some(row) = rows.next()? {
            let mut rec = Vec::with_capacity(ncols);
            for i in 0..ncols {
                rec.push(row.get::<usize, Option<String>>(i)?);
            }
            out.push(rec);
        }

        let truncated = out.len() > row_cap;
        if truncated {
            out.truncate(row_cap);
        }
        Ok(QueryResult {
            columns: schema.columns,
            rows: out,
            truncated,
        })
    }
}

/// Allow only statements that cannot mutate state.
fn guard_read_only(sql: &str) -> Result<()> {
    let head = sql
        .trim_start()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    const ALLOWED: &[&str] = &[
        "SELECT", "WITH", "DESCRIBE", "SUMMARIZE", "EXPLAIN", "PRAGMA", "SHOW", "TABLE", "VALUES",
    ];
    if ALLOWED.contains(&head.as_str()) {
        Ok(())
    } else {
        Err(LabError::NotReadOnly(head))
    }
}
