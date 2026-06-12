//! Geospatial + Excel ingest — the formats deferred from mimir-lab P1 because they
//! need DuckDB extensions. Registers the dataset in the shared `analytics_datasets`
//! registry with a `geometry_column` + `srid`, and (for points) an H3 index column.
//!
//! P4 TODO:
//! - [ ] GeoJSON / Shapefile → `ST_Read('<path>')` view (spatial ext) → persist as
//!       Parquet in MinIO (`asgard-analytics` bucket) like the CSV path.
//! - [ ] Excel (.xlsx) → DuckDB `excel`/`st_read` extension or `calamine` crate fallback.
//! - [ ] Schema inference incl. detected geometry column + SRID; Skuggi PII gate on
//!       attribute columns (geometry itself is not PII, attributes may be).
//! - [ ] Register in `analytics_datasets` (reuse `mimir_lab::registry`) with geo extras.

use crate::error::Result;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct GeoIngestReport {
    pub dataset_id: String,
    pub rows: u64,
    pub geometry_column: Option<String>,
    pub srid: Option<i32>,
}

/// Ingest a GeoJSON/Shapefile/Excel file → registered dataset.
pub fn ingest_geo(_path: &str, _tenant_id: &str) -> Result<GeoIngestReport> {
    todo!("P4: ST_Read → infer geometry/SRID → Parquet→MinIO → registry")
}
