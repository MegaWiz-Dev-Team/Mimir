//! Uber H3 hexagonal grid via `h3o` (pure Rust). Backs the `geo_h3` tool and the
//! H3 aggregation used by choropleth/heatmap layers in the portal.
//!
//! P4 TODO:
//! - [ ] `latlng_to_cell(lat, lng, res)` — `LatLng::new(...).to_cell(Resolution::try_from(res))`.
//! - [ ] `cell_boundary(cell)` → polygon ring (for map rendering).
//! - [ ] `k_ring(cell, k)` / `polyfill(polygon, res)` for binning points into cells.
//! - [ ] `aggregate(points, res)` → per-cell counts (the common heatmap path).

use crate::error::Result;

/// Lat/lng → H3 cell index at `res` (0–15), returned as the canonical hex string.
pub fn latlng_to_cell(_lat: f64, _lng: f64, _res: u8) -> Result<String> {
    todo!("P4: h3o LatLng::to_cell")
}

/// Bin points into H3 cells at `res` → (cell, count) pairs. The heatmap aggregation.
pub fn aggregate(_points: &[(f64, f64)], _res: u8) -> Result<Vec<(String, u64)>> {
    todo!("P4: polyfill/cell binning + counts")
}
