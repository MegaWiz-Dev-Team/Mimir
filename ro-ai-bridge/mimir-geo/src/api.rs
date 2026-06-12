//! HTTP handlers backing the Hermodr `geo_*` / `stats_*` MCP tools, mounted into the
//! existing `analytics-api` axum server alongside mimir-lab's `/api/v1/analytics/*`.
//! Same request/response shape conventions as `mimir_lab::api` (tenant_id in body,
//! columns+rows or a typed result; errors → 4xx/5xx JSON).
//!
//! P4 routes (add to `mimir-lab/src/server.rs` router):
//!   POST /api/v1/analytics/geo/query        → spatial SQL (ST_*), capped/audited
//!   POST /api/v1/analytics/geo/ingest       → GeoJSON/Shapefile/Excel ingest
//!   POST /api/v1/analytics/geo/h3           → latlng→cell / aggregate
//!   POST /api/v1/analytics/geo/choropleth   → classify → classes + (optional) ECharts/GeoJSON
//!   POST /api/v1/analytics/stats/{moran|lisa|kriging|pointpattern} → sandboxed Python
//!
//! P4 TODO:
//! - [ ] Deserialize structs per route (reuse spatial/h3/stats_py signatures).
//! - [ ] Wire into the analytics-api server; `analyst-geo`/`analyst-stats` tool
//!       allowlists already name these (keep single-tool-per-agent for gemma
//!       convergence — see asgard_analytics_tenant reliability note).

// TODO(P4): implement handlers once engine/spatial/h3/stats_py land.
