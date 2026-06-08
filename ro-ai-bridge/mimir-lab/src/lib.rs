//! # mimir-lab
//!
//! Asgard Analytics data engine (ADR-024). The cross-cutting (Tier B / AGPL)
//! compute layer for the `asgard_analytics` tenant:
//!
//! - [`engine`] — DuckDB wrapper with a read-only query guard + row cap.
//! - [`ingest`] — CSV → table with schema inference; Parquet export.
//! - [`schema`] — inferred column/table schema types.
//! - [`pii`]    — Skuggi Tier-1 PII gate (`Pending`/`Clean`/`Flagged`) on ingest.
//!
//! The relational dataset registry (datasets / dataset_versions / analyses /
//! report_jobs / geo_layers) is defined in `migrations/0001_init_analytics.sql`
//! and lives in the Mimir MariaDB; this crate owns the DuckDB-side compute.

pub mod engine;
pub mod error;
pub mod ingest;
pub mod pii;
pub mod schema;

pub use engine::{Engine, QueryResult};
pub use error::{LabError, Result};
pub use ingest::{export_parquet, ingest_csv, IngestResult};
pub use pii::{gate_table_column, scan_samples, PiiStatus};
pub use schema::{ColumnSchema, TableSchema};
