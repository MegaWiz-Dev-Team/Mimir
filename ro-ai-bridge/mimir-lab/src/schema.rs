//! Dataset schema types + DuckDB-backed schema inference.

use serde::{Deserialize, Serialize};

/// One column of an inferred dataset schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    /// DuckDB SQL type, e.g. `BIGINT`, `VARCHAR`, `DOUBLE`, `DATE`.
    pub sql_type: String,
    pub nullable: bool,
}

/// The schema of a dataset (ordered columns).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSchema {
    pub columns: Vec<ColumnSchema>,
}

impl TableSchema {
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }
}
