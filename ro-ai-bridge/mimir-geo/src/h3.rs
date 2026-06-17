//! Uber H3 hexagonal grid via `h3o` (pure Rust). Backs the `geo_h3` tool + the
//! H3 aggregation behind portal heatmap/choropleth layers.

use crate::error::{GeoError, Result};
use h3o::{CellIndex, LatLng, Resolution};
use std::collections::HashMap;

fn resolution(r: u8) -> Result<Resolution> {
    Resolution::try_from(r).map_err(|e| GeoError::H3(format!("bad resolution {r}: {e}")))
}
fn parse_cell(cell: &str) -> Result<CellIndex> {
    cell.parse::<CellIndex>().map_err(|e| GeoError::H3(format!("bad cell '{cell}': {e}")))
}
fn latlng(lat: f64, lng: f64) -> Result<LatLng> {
    LatLng::new(lat, lng).map_err(|e| GeoError::H3(e.to_string()))
}

/// Lat/lng → H3 cell at resolution `r` (0–15), canonical hex string.
pub fn latlng_to_cell(lat: f64, lng: f64, r: u8) -> Result<String> {
    Ok(latlng(lat, lng)?.to_cell(resolution(r)?).to_string())
}

/// Cell boundary as a (lat, lng) ring — for rendering the hexagon on a map.
pub fn cell_boundary(cell: &str) -> Result<Vec<(f64, f64)>> {
    Ok(parse_cell(cell)?.boundary().iter().map(|v| (v.lat(), v.lng())).collect())
}

/// k-ring (grid disk) around a cell → cell hex strings (includes the centre).
pub fn k_ring(cell: &str, k: u32) -> Result<Vec<String>> {
    Ok(parse_cell(cell)?.grid_disk::<Vec<_>>(k).into_iter().map(|c| c.to_string()).collect())
}

/// Bin points into H3 cells at resolution `r` → (cell, count), descending by count.
/// The heatmap aggregation.
pub fn aggregate(points: &[(f64, f64)], r: u8) -> Result<Vec<(String, u64)>> {
    let res = resolution(r)?;
    let mut counts: HashMap<CellIndex, u64> = HashMap::new();
    for &(lat, lng) in points {
        *counts.entry(latlng(lat, lng)?.to_cell(res)).or_insert(0) += 1;
    }
    let mut out: Vec<(String, u64)> = counts.into_iter().map(|(c, n)| (c.to_string(), n)).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cell_roundtrip_and_boundary() {
        let cell = latlng_to_cell(13.7563, 100.5018, 7).unwrap(); // Bangkok
        assert!(!cell.is_empty());
        let ring = cell_boundary(&cell).unwrap();
        assert_eq!(ring.len(), 6, "a hexagon has 6 vertices"); // non-pentagon cell
    }
    #[test]
    fn k_ring_grows() {
        let c = latlng_to_cell(13.7563, 100.5018, 7).unwrap();
        assert_eq!(k_ring(&c, 0).unwrap().len(), 1);
        assert_eq!(k_ring(&c, 1).unwrap().len(), 7); // centre + 6 neighbours
    }
    #[test]
    fn aggregate_bins_points() {
        let pts = vec![(13.7563, 100.5018), (13.7564, 100.5019), (18.7883, 98.9853)];
        let agg = aggregate(&pts, 6).unwrap();
        assert_eq!(agg.iter().map(|(_, n)| n).sum::<u64>(), 3);
        assert!(agg[0].1 >= 1);
    }
}
