//! # mimir-geo — Asgard Analytics spatial engine (ADR-024, P4)
//!
//! Pure-Rust geometry + H3 + spatial statistics, **offline-safe** (no DuckDB
//! `spatial` extension, which needs a network INSTALL — see Cargo.toml note). Backs
//! the Hermodr `geo_*` / `stats_*` MCP tools that the `analyst-geo` / `analyst-stats`
//! agents call (asgard_analytics tenant).
//!
//! - [`spatial`] — GeoRust ops: `geo_distance`, point-in-polygon `geo_join`,
//!   `geo_buffer` (point→circle), `geo_choropleth` classification.
//! - [`h3`] — Uber H3 (h3o): lat/lng→cell, boundary, k-ring, point aggregation.
//! - [`stats`] — spatial statistics in Rust: global Moran's I, mean nearest-neighbour
//!   (kriging / LISA stay for the deferred Python sandbox — vendor PySAL wheels first).
//! - [`ingest_geo`] — GeoJSON → features + summary (Shapefile/Excel deferred).
//!
//! All ops are deterministic + explainable (no ML).

pub mod error;
pub mod h3;
pub mod ingest_geo;
pub mod spatial;
pub mod stats;

pub use error::{GeoError, Result};
