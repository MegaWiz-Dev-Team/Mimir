//! GeoJSON ingest — parse → per-feature geometry + a dataset summary (count,
//! geometry-type histogram, bbox, property keys). The format deferred from mimir-lab
//! P1. Shapefile + Excel ingest are TODO (shapefile / calamine crates).

use crate::error::{GeoError, Result};
use geo::BoundingRect;
use geo_types::Geometry;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct GeoSummary {
    pub features: usize,
    /// geometry kind → count (e.g. "Point": 42, "Polygon": 3).
    pub geometry_types: BTreeMap<String, usize>,
    /// [min_lng, min_lat, max_lng, max_lat], or None if no geometry.
    pub bbox: Option<[f64; 4]>,
    /// distinct property keys across features (the candidate attribute columns).
    pub property_keys: Vec<String>,
}

fn kind(g: &Geometry<f64>) -> &'static str {
    match g {
        Geometry::Point(_) => "Point",
        Geometry::Line(_) => "Line",
        Geometry::LineString(_) => "LineString",
        Geometry::Polygon(_) => "Polygon",
        Geometry::MultiPoint(_) => "MultiPoint",
        Geometry::MultiLineString(_) => "MultiLineString",
        Geometry::MultiPolygon(_) => "MultiPolygon",
        Geometry::GeometryCollection(_) => "GeometryCollection",
        Geometry::Rect(_) => "Rect",
        Geometry::Triangle(_) => "Triangle",
    }
}

/// Parse a GeoJSON document → summary. Accepts FeatureCollection / Feature / Geometry.
pub fn summarize_geojson(src: &str) -> Result<GeoSummary> {
    let gj: geojson::GeoJson = src.parse().map_err(|e| GeoError::Ingest(format!("invalid GeoJSON: {e}")))?;
    let features: Vec<geojson::Feature> = match gj {
        geojson::GeoJson::FeatureCollection(fc) => fc.features,
        geojson::GeoJson::Feature(f) => vec![f],
        geojson::GeoJson::Geometry(g) => vec![geojson::Feature {
            bbox: None, geometry: Some(g), id: None, properties: None, foreign_members: None,
        }],
    };

    let mut types: BTreeMap<String, usize> = BTreeMap::new();
    let mut keys: std::collections::BTreeSet<String> = Default::default();
    let mut bbox: Option<[f64; 4]> = None;

    for f in &features {
        if let Some(props) = &f.properties {
            keys.extend(props.keys().cloned());
        }
        if let Some(gjg) = &f.geometry {
            let geom: Geometry<f64> = Geometry::try_from(gjg.clone())
                .map_err(|e| GeoError::Ingest(format!("geometry conversion: {e}")))?;
            *types.entry(kind(&geom).to_string()).or_insert(0) += 1;
            if let Some(r) = geom.bounding_rect() {
                let (m, x) = (r.min(), r.max());
                bbox = Some(match bbox {
                    None => [m.x, m.y, x.x, x.y],
                    Some([a, b, c, d]) => [a.min(m.x), b.min(m.y), c.max(x.x), d.max(x.y)],
                });
            }
        }
    }

    Ok(GeoSummary {
        features: features.len(),
        geometry_types: types,
        bbox,
        property_keys: keys.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const FC: &str = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"A","ahi":42},"geometry":{"type":"Point","coordinates":[100.5,13.7]}},
      {"type":"Feature","properties":{"name":"B"},"geometry":{"type":"Point","coordinates":[98.9,18.7]}}
    ]}"#;
    #[test]
    fn summarize_feature_collection() {
        let s = summarize_geojson(FC).unwrap();
        assert_eq!(s.features, 2);
        assert_eq!(s.geometry_types.get("Point"), Some(&2));
        assert_eq!(s.property_keys, vec!["ahi".to_string(), "name".to_string()]);
        let b = s.bbox.unwrap();
        assert!((b[0] - 98.9).abs() < 1e-6 && (b[2] - 100.5).abs() < 1e-6);
    }
    #[test]
    fn rejects_garbage() {
        assert!(summarize_geojson("not json").is_err());
    }
}
