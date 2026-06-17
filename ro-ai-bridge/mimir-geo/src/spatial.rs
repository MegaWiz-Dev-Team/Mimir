//! GeoRust geometry ops exposed as the Hermodr `geo_*` tools. Pure-Rust, offline.
//! Coordinates are (lat, lng) at the API boundary; internally geo uses Point(x=lng,
//! y=lat), so we swap at the edges.

use crate::error::{GeoError, Result};
use geo::{Contains, Distance, Haversine};
use geo::{Coord, LineString, Point, Polygon};

fn pt(lat: f64, lng: f64) -> Point<f64> { Point::new(lng, lat) }

/// Great-circle distance between two (lat,lng) points, in metres.
pub fn geo_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    Haversine::distance(pt(a.0, a.1), pt(b.0, b.1))
}

/// Build a geo Polygon from an outer ring given as (lat,lng) vertices.
fn polygon(ring: &[(f64, f64)]) -> Result<Polygon<f64>> {
    if ring.len() < 3 {
        return Err(GeoError::Geometry(format!("polygon ring needs ≥3 vertices, got {}", ring.len())));
    }
    let coords: Vec<Coord<f64>> = ring.iter().map(|&(lat, lng)| Coord { x: lng, y: lat }).collect();
    Ok(Polygon::new(LineString::new(coords), vec![]))
}

/// Approximate circular buffer around a (lat,lng) point: a `segments`-gon at
/// `radius_m` metres (equirectangular offset — good for small radii / catchments).
/// Returns the ring as (lat,lng) vertices (closed: first == last).
pub fn geo_buffer(lat: f64, lng: f64, radius_m: f64, segments: usize) -> Result<Vec<(f64, f64)>> {
    if radius_m <= 0.0 { return Err(GeoError::BadRequest("radius_m must be > 0".into())); }
    let n = segments.max(8);
    let dlat = radius_m / 111_320.0;
    let dlng = radius_m / (111_320.0 * lat.to_radians().cos().abs().max(1e-6));
    let mut ring: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let t = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            (lat + dlat * t.sin(), lng + dlng * t.cos())
        })
        .collect();
    ring.push(ring[0]); // close
    Ok(ring)
}

/// Point-in-polygon join: for each point (lat,lng), the index of the first polygon
/// (each an outer (lat,lng) ring) that contains it, or `None`.
pub fn geo_join(points: &[(f64, f64)], polygons: &[Vec<(f64, f64)>]) -> Result<Vec<Option<usize>>> {
    let polys: Vec<Polygon<f64>> = polygons.iter().map(|r| polygon(r)).collect::<Result<_>>()?;
    Ok(points
        .iter()
        .map(|&(lat, lng)| {
            let p = pt(lat, lng);
            polys.iter().position(|poly| poly.contains(&p))
        })
        .collect())
}

/// Choropleth classification → class index (0..classes) per value.
/// `method` ∈ "equal_interval" | "quantile".
pub fn geo_choropleth(values: &[f64], classes: usize, method: &str) -> Result<Vec<usize>> {
    if classes == 0 { return Err(GeoError::BadRequest("classes must be ≥1".into())); }
    if values.is_empty() { return Ok(vec![]); }
    match method {
        "equal_interval" => {
            let (min, max) = values.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            let span = (max - min).max(f64::MIN_POSITIVE);
            Ok(values.iter().map(|&v| (((v - min) / span) * classes as f64).floor().min((classes - 1) as f64) as usize).collect())
        }
        "quantile" => {
            let mut sorted = values.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            // upper bound of each class by quantile
            let bounds: Vec<f64> = (1..classes)
                .map(|k| sorted[((sorted.len() * k) / classes).min(sorted.len() - 1)])
                .collect();
            Ok(values.iter().map(|&v| bounds.iter().position(|&b| v < b).unwrap_or(classes - 1)).collect())
        }
        other => Err(GeoError::BadRequest(format!("unknown choropleth method '{other}' (equal_interval|quantile)"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_bangkok_chiangmai() {
        let d = geo_distance((13.7563, 100.5018), (18.7883, 98.9853));
        assert!((d - 587_000.0).abs() < 20_000.0, "≈587 km, got {d}"); // BKK→CNX
    }
    #[test]
    fn buffer_then_contains_centre() {
        let ring = geo_buffer(13.7563, 100.5018, 1000.0, 16).unwrap();
        assert_eq!(ring.first(), ring.last());
        let inside = geo_join(&[(13.7563, 100.5018)], &[ring.clone()]).unwrap();
        assert_eq!(inside[0], Some(0), "centre is inside its own buffer");
        let outside = geo_join(&[(13.80, 100.55)], &[ring]).unwrap();
        assert_eq!(outside[0], None, "far point outside a 1km buffer");
    }
    #[test]
    fn choropleth_equal_interval() {
        let c = geo_choropleth(&[0.0, 5.0, 10.0], 2, "equal_interval").unwrap();
        assert_eq!(c[0], 0);
        assert_eq!(c[2], 1);
    }
    #[test]
    fn choropleth_quantile_splits() {
        let c = geo_choropleth(&[1.0, 2.0, 3.0, 4.0], 2, "quantile").unwrap();
        assert_eq!(c.len(), 4);
        assert!(c.iter().all(|&k| k < 2));
    }
}
