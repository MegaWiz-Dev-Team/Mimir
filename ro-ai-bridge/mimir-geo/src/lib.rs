//! # mimir-geo — Asgard Analytics spatial engine (ADR-024, P4)
//!
//! Adds GIS + spatial-statistics on top of [`mimir-lab`]'s tabular engine, behind
//! the same governance (read-only guard, row-cap, Tyr audit, Skuggi PII gate). Three
//! layers:
//!
//! 1. **DuckDB spatial** ([`engine`]) — loads the `spatial` extension; `ST_*` ops +
//!    `ST_Read` for GeoJSON/Shapefile ingest. Shares the read-only/timeout/audit
//!    discipline of `mimir-lab::Engine`.
//! 2. **GeoRust + H3** ([`spatial`], [`h3`]) — pure-Rust geometry ops (buffer,
//!    distance, centroid, choropleth bucketing) and Uber H3 indexing (lat/lng→cell,
//!    k-ring, polyfill) for ops better done in Rust than SQL.
//! 3. **Sandboxed-Python spatial-stats** ([`stats_py`]) — PySAL/scipy (Moran's I,
//!    LISA, kriging, point-pattern) in an **isolated, resource-capped, serialized**
//!    venv. The one sanctioned Rust-first exception (ADR-024); serialized per the
//!    Mac-mini memory-pressure rule — never run two Python jobs concurrently.
//!
//! The [`api`] handlers back the Hermodr `geo_*` / `stats_*` MCP tools that the
//! `analyst-geo` / `analyst-stats` agents call (see asgard_analytics_tenant memory).
//!
//! Status: **P4 scaffold** — module skeletons + signatures + TDD targets. Implement
//! per `README.md` checklist.

pub mod api;
pub mod engine;
pub mod error;
pub mod h3;
pub mod ingest_geo;
pub mod spatial;
pub mod stats_py;

pub use error::{GeoError, Result};
