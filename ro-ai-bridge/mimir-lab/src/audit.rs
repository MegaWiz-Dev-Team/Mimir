//! Audit emission for analytics actions (Tyr-ingestible).
//!
//! Mirrors the repo convention (`mimir-well::ProvSink`, Heimdall auth audit): a
//! sink trait with a default `tracing` emitter and a no-op for tests. Tyr
//! ingests the structured `tracing` events (`target = "analytics.audit"`); on a
//! customer box a log shipper forwards them to Tyr. Per ADR-024 every query /
//! export over `asgard_analytics` data is audited.

use std::sync::Mutex;

/// One audit record for an analytics action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    /// e.g. `analytics.query`, `analytics.export`.
    pub action: &'static str,
    pub tenant_id: Option<String>,
    pub actor: Option<String>,
    /// What was acted on — a table name, dataset id, or (truncated) SQL.
    pub target: Option<String>,
    /// `ok` | `timeout` | `denied` | `error`.
    pub outcome: &'static str,
    pub detail: Option<String>,
}

/// Who an action is attributed to (attached to the engine).
#[derive(Debug, Clone, Default)]
pub struct AuditContext {
    pub tenant_id: Option<String>,
    pub actor: Option<String>,
}

/// Receives audit events. Implementations must be cheap + non-panicking.
pub trait AuditSink: Send + Sync {
    fn record(&self, event: &AuditEvent);
}

/// Default sink — emits a structured `tracing` event Tyr can scrape.
#[derive(Debug, Default)]
pub struct TracingAuditSink;

impl AuditSink for TracingAuditSink {
    fn record(&self, e: &AuditEvent) {
        // Tyr-ingestible shape (matches Heimdall/dual_mode_auth audit style).
        tracing::info!(
            target: "analytics.audit",
            action = e.action,
            tenant = e.tenant_id.as_deref().unwrap_or("-"),
            actor = e.actor.as_deref().unwrap_or("-"),
            outcome = e.outcome,
            target_obj = e.target.as_deref().unwrap_or("-"),
            detail = e.detail.as_deref().unwrap_or(""),
            "analytics.audit"
        );
    }
}

/// Drops events (tests / when audit is intentionally disabled).
#[derive(Debug, Default)]
pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    fn record(&self, _: &AuditEvent) {}
}

/// Captures events in memory for assertions in tests.
#[derive(Debug, Default)]
pub struct VecAuditSink {
    events: Mutex<Vec<AuditEvent>>,
}

impl VecAuditSink {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl AuditSink for VecAuditSink {
    fn record(&self, event: &AuditEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}
