//! DuckDB spatial engine. Thin wrapper that loads the `spatial` extension and then
//! reuses mimir-lab's discipline: read-only statement guard, row-cap, query timeout
//! (interrupt_handle + watchdog), and Tyr audit on every query.
//!
//! P4 TODO:
//! - [ ] `GeoEngine::open` → `Connection::open_in_memory()` + `INSTALL spatial; LOAD spatial;`
//!       (the bundled build ships the ext; no network install on-prem).
//! - [ ] Reuse `mimir_lab::Engine`'s read-only guard + row-cap + AuditSink rather than
//!       re-implementing — extract that into a shared `mimir-lab` `query` helper, or
//!       depend on `mimir-lab` here (preferred: one governance path).
//! - [ ] `attach_geojson(path, view)` / `attach_parquet(...)` → `ST_Read` views.
//! - [ ] Spatial queries go through the SAME `query_readonly_timeout` so they are
//!       capped/timed/audited identically to tabular queries.

use crate::error::Result;

/// A spatial-capable DuckDB handle (in-memory; data attached as views).
pub struct GeoEngine {
    // TODO: hold the duckdb::Connection + the shared AuditSink + row_cap/timeout config.
}

impl GeoEngine {
    /// Open an in-memory engine with the `spatial` extension loaded.
    pub fn open() -> Result<Self> {
        // TODO: Connection::open_in_memory(); execute "INSTALL spatial; LOAD spatial;".
        todo!("P4: open DuckDB + LOAD spatial")
    }

    /// Read-only spatial query (capped + timed + Tyr-audited), columns + rows JSON —
    /// identical contract to `mimir_lab::api::run_sql` so the api layer is uniform.
    pub fn query_readonly(&self, _sql: &str, _tenant_id: &str) -> Result<serde_json::Value> {
        todo!("P4: delegate to the shared read-only/timeout/audit path")
    }
}
