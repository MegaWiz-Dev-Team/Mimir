//! GeoRust geometry ops exposed as the Hermodr `geo_*` tools. Pure-Rust (no SQL)
//! for ops that are awkward in DuckDB or needed standalone. TDD with fixture
//! geometries (a few WKT/GeoJSON points + polygons in `tests/`).
//!
//! P4 TODO — `geo_buffer` / `geo_distance` / `geo_join` / `geo_choropleth`:
//! - [ ] buffer(geom, meters) — GeoRust `buffer`/`Polygon` (project to a metric CRS first).
//! - [ ] distance(a, b) — haversine / euclidean per CRS.
//! - [ ] point-in-polygon join (assign points to regions) — `Contains`.
//! - [ ] choropleth bucketing (equal-interval / quantile / jenks) → class per feature.

use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BufferReq {
    pub wkt: String,
    pub meters: f64,
}
#[derive(Debug, Serialize)]
pub struct GeomResult {
    pub wkt: String,
}

/// Buffer a geometry by `meters` (metric projection → buffer → back to WGS84).
pub fn geo_buffer(_req: &BufferReq) -> Result<GeomResult> {
    todo!("P4: GeoRust buffer with metric reprojection")
}

/// Choropleth classes for a numeric column. `method` ∈ equal_interval|quantile|jenks.
pub fn geo_choropleth(_values: &[f64], _classes: usize, _method: &str) -> Result<Vec<usize>> {
    todo!("P4: classify values into choropleth buckets")
}
