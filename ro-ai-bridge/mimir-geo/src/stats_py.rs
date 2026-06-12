//! Sandboxed-Python spatial statistics (PySAL / scipy) — the one sanctioned
//! Rust-first exception (ADR-024). Rust owns orchestration + governance; Python is a
//! **stateless, isolated, resource-capped, SERIALIZED** compute kernel.
//!
//! ## Why serialized (hard rule)
//! Per the Mac-mini memory-pressure rule ([[feedback_mac_mini_memory_pressure]]):
//! never run two Python jobs (or a Python job + a heavy model load) concurrently. A
//! process-wide `Mutex`/permit serializes invocations; a second caller waits or gets
//! `GeoError::Sandbox("busy")`.
//!
//! ## Transport (no PyO3)
//! Rust writes a request JSON to a temp file, spawns `python/.venv/bin/python
//! python/stats.py <reqfile> <respfile>` with:
//!   - a dedicated venv (`python/requirements.txt`, no system site-packages),
//!   - `ulimit`/`RLIMIT_AS` memory cap + wall-clock timeout (kill on overrun),
//!   - no network, cwd-jailed.
//! Then reads the response JSON. Keeps the heavy SciPy stack out of the Rust build
//! and crash-isolated.
//!
//! ## Tools (back `stats_*` MCP tools for `analyst-stats`)
//! `stats_moran` (global Moran's I), `stats_lisa` (local indicators), `stats_kriging`
//! (ordinary kriging interpolation), `stats_pointpattern` (Ripley's K / NN).
//!
//! P4 TODO:
//! - [ ] `SandboxPermit` (process-global `Mutex<()>` or `tokio::Semaphore(1)`).
//! - [ ] `run(method, payload)` — write req → spawn capped python → read resp → JSON.
//! - [ ] timeout + RLIMIT_AS + kill-on-overrun; map failures → `GeoError::Sandbox`.
//! - [ ] every run Tyr-audited (method + row-count + duration; never the raw data).

use crate::error::Result;
use serde_json::Value;

/// Run a spatial-statistics method in the serialized Python sandbox.
/// `method` ∈ moran | lisa | kriging | pointpattern.
pub fn run(_method: &str, _payload: &Value) -> Result<Value> {
    // TODO: acquire SandboxPermit (serialize) → temp req/resp → capped child → parse.
    todo!("P4: sandboxed, serialized, resource-capped Python spatial-stats")
}
