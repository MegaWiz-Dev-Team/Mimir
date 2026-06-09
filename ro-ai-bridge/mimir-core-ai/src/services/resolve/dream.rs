//! The nightly "dream pass" worker.
//!
//! Entities ingested in the same batch never get compared to each other at
//! ingest time (neither existed in the graph when the other was written). The
//! dream pass closes that gap: it re-runs deduplication over recently ingested
//! nodes only — reusing their stored embeddings, so it makes no embedding calls —
//! and flags new `DUPLICATE_OF` review pairs (idempotent; it never merges).
//!
//! Runs when organic traffic is low (hence "dream"). It is opt-in and off by
//! default; the app spawns [`start_dream_worker`] only when configured.

use neo4rs::Graph;
use tracing::{info, warn};

use super::store::{self, ResolveParams};

/// Run one dream pass across the given tenants, looking back `lookback_hours`.
/// Returns the total number of duplicate pairs flagged. Per-tenant failures are
/// logged and skipped (one bad tenant must not abort the sweep).
pub async fn run_dream_pass_once(
    graph: &Graph,
    tenant_ids: &[String],
    lookback_hours: i64,
    params: &ResolveParams,
) -> usize {
    let since = (chrono::Utc::now() - chrono::Duration::hours(lookback_hours)).to_rfc3339();
    let mut total = 0usize;
    for t in tenant_ids {
        match store::dream_pass(graph, t, &since, params).await {
            Ok(n) => {
                total += n;
                info!(target: "resolve_dream", tenant = %t, flagged = n, since = %since, "dream pass tenant complete");
            }
            Err(e) => {
                warn!(target: "resolve_dream", tenant = %t, error = %e, "dream pass tenant failed");
            }
        }
    }
    total
}

/// Spawn the recurring dream-pass background task.
///
/// - `interval_secs`: how often to wake (e.g. 86400 for daily).
/// - `lookback_hours`: how far back "recent" reaches (use slightly more than the
///   interval, e.g. 25h for a daily pass, so nothing falls through a gap).
pub fn start_dream_worker(
    graph: Graph,
    tenant_ids: Vec<String>,
    interval_secs: u64,
    lookback_hours: i64,
    params: ResolveParams,
) {
    tokio::spawn(async move {
        info!(
            target: "resolve_dream",
            interval_secs,
            lookback_hours,
            tenants = tenant_ids.len(),
            "🌙 dream-pass worker started"
        );
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            let flagged = run_dream_pass_once(&graph, &tenant_ids, lookback_hours, &params).await;
            info!(target: "resolve_dream", flagged, "dream pass sweep complete");
        }
    });
}
