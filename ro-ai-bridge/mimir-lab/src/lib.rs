//! # mimir-lab
//!
//! Asgard Analytics data engine (ADR-024). The cross-cutting (Tier B / AGPL)
//! compute layer for the `asgard_analytics` tenant:
//!
//! - [`engine`] — DuckDB wrapper with a read-only query guard + row cap.
//! - [`ingest`] — CSV → table with schema inference; Parquet export.
//! - [`schema`] — inferred column/table schema types.
//! - [`pii`]    — Skuggi Tier-1 PII gate (`Pending`/`Clean`/`Flagged`) on ingest.
//! - [`registry`] — relational dataset catalog (MariaDB via sqlx): dataset
//!   lifecycle register/list/profile/update-pii/version.
//! - [`storage`] — MinIO/S3 blob storage for dataset originals + versions.
//! - [`audit`]   — Tyr-ingestible audit emission on query/export.
//!
//! The relational dataset registry (datasets / dataset_versions / analyses /
//! report_jobs / geo_layers) is defined in `migrations/0001_init_analytics.sql`
//! and lives in the Mimir MariaDB; this crate owns the DuckDB-side compute.

pub mod api;
pub mod audit;
pub mod engine;
pub mod error;
pub mod geo_api;
pub mod ingest;
pub mod lit_api;
pub mod pii;
pub mod registry;
pub mod schema;
pub mod server;
pub mod storage;

pub use audit::{
    sink_from_env, AuditEvent, AuditSink, HttpTyrSink, NoopAuditSink, TracingAuditSink,
    VecAuditSink,
};
pub use engine::{Engine, QueryResult};
pub use error::{LabError, Result};
pub use ingest::{export_parquet, ingest_csv, ingest_file, IngestResult, SourceFormat};
pub use pii::{gate_table_column, scan_samples, PiiStatus};
pub use registry::{Dataset, NewDataset, Registry};
pub use schema::{ColumnSchema, TableSchema};
pub use storage::{Storage, StorageConfig};
