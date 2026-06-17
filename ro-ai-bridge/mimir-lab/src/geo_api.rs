//! HTTP handlers for the `geo_*` / `stats_*` tools (backed by `mimir-geo`), mounted
//! into the analytics-api server next to the tabular `/query` handlers. Pure (no
//! DuckDB state) — deserialize → call mimir-geo → JSON. Points are (lat, lng).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mimir_geo::{h3, ingest_geo, spatial, stats, GeoError};
use serde::Deserialize;
use serde_json::{json, Value};

/// GeoError → HTTP response (bad-input → 400, else 422).
pub struct GeoErr(GeoError);
impl IntoResponse for GeoErr {
    fn into_response(self) -> Response {
        let code = match &self.0 {
            GeoError::BadRequest(_) | GeoError::Geometry(_) | GeoError::H3(_) | GeoError::Ingest(_) => {
                StatusCode::BAD_REQUEST
            }
            GeoError::Stats(_) => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (code, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}
impl From<GeoError> for GeoErr {
    fn from(e: GeoError) -> Self {
        GeoErr(e)
    }
}

fn def_segments() -> usize { 32 }
fn def_method() -> String { "quantile".into() }

#[derive(Deserialize)]
pub struct DistReq { pub a: (f64, f64), pub b: (f64, f64) }
pub async fn distance(Json(r): Json<DistReq>) -> Json<Value> {
    Json(json!({ "meters": spatial::geo_distance(r.a, r.b) }))
}

#[derive(Deserialize)]
pub struct BufferReq { pub lat: f64, pub lng: f64, pub radius_m: f64, #[serde(default = "def_segments")] pub segments: usize }
pub async fn buffer(Json(r): Json<BufferReq>) -> Result<Json<Value>, GeoErr> {
    Ok(Json(json!({ "ring": spatial::geo_buffer(r.lat, r.lng, r.radius_m, r.segments)? })))
}

#[derive(Deserialize)]
pub struct JoinReq { pub points: Vec<(f64, f64)>, pub polygons: Vec<Vec<(f64, f64)>> }
pub async fn join(Json(r): Json<JoinReq>) -> Result<Json<Value>, GeoErr> {
    Ok(Json(json!({ "assignment": spatial::geo_join(&r.points, &r.polygons)? })))
}

#[derive(Deserialize)]
pub struct ChoroReq { pub values: Vec<f64>, pub classes: usize, #[serde(default = "def_method")] pub method: String }
pub async fn choropleth(Json(r): Json<ChoroReq>) -> Result<Json<Value>, GeoErr> {
    Ok(Json(json!({ "classes": spatial::geo_choropleth(&r.values, r.classes, &r.method)? })))
}

#[derive(Deserialize)]
pub struct H3Req { pub points: Vec<(f64, f64)>, pub resolution: u8 }
pub async fn h3_aggregate(Json(r): Json<H3Req>) -> Result<Json<Value>, GeoErr> {
    let cells = h3::aggregate(&r.points, r.resolution)?;
    Ok(Json(json!({ "cells": cells.iter().map(|(c, n)| json!({ "cell": c, "count": n })).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
pub struct IngestReq { pub geojson: String }
pub async fn ingest(Json(r): Json<IngestReq>) -> Result<Json<Value>, GeoErr> {
    Ok(Json(json!(ingest_geo::summarize_geojson(&r.geojson)?)))
}

#[derive(Deserialize)]
pub struct MoranReq { pub points: Vec<(f64, f64)>, pub values: Vec<f64>, pub threshold_m: f64 }
pub async fn moran(Json(r): Json<MoranReq>) -> Result<Json<Value>, GeoErr> {
    Ok(Json(json!({ "moran_i": stats::morans_i(&r.points, &r.values, r.threshold_m)? })))
}

#[derive(Deserialize)]
pub struct NnReq { pub points: Vec<(f64, f64)> }
pub async fn nn(Json(r): Json<NnReq>) -> Result<Json<Value>, GeoErr> {
    Ok(Json(json!({ "mean_nn_m": stats::nn_mean_distance(&r.points)? })))
}
