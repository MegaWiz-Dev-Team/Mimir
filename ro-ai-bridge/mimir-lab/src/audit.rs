//! Audit emission for analytics actions (Tyr-ingestible).
//!
//! Mirrors the repo convention (`mimir-well::ProvSink`, Heimdall auth audit): a
//! sink trait with a default `tracing` emitter and a no-op for tests. Tyr
//! ingests the structured `tracing` events (`target = "analytics.audit"`); on a
//! customer box a log shipper forwards them to Tyr. Per ADR-024 every query /
//! export over `asgard_analytics` data is audited.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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

/// Active sink that **forwards** events to Tyr over HTTP — same transport as
/// Heimdall / `mimir-well::HttpProvSink`. `record` is non-blocking: it enqueues
/// onto an unbounded channel drained by a background task that POSTs to the Tyr
/// audit-ingest endpoint (2s timeout so a slow Tyr never stalls a query).
///
/// Must be constructed inside a Tokio runtime (it `tokio::spawn`s the drain).
pub struct HttpTyrSink {
    tx: tokio::sync::mpsc::UnboundedSender<AuditEvent>,
}

impl HttpTyrSink {
    /// Spawn the drain task. `auth_header` e.g. `Bearer <token>` is optional.
    pub fn spawn(url: impl Into<String>, auth_header: Option<String>) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AuditEvent>();
        let url = url.into();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("reqwest client");
        tokio::spawn(async move {
            while let Some(e) = rx.recv().await {
                let body = serde_json::json!({
                    "@type": "AuditEvent",
                    "asgard:component": "mimir-lab",
                    "asgard:action": e.action,
                    "asgard:tenant_id": e.tenant_id,
                    "asgard:actor": e.actor,
                    "asgard:outcome": e.outcome,
                    "asgard:target": e.target,
                    "asgard:detail": e.detail,
                    "asgard:emitted_at": chrono::Utc::now().to_rfc3339(),
                });
                let mut req = client
                    .post(&url)
                    .header("content-type", "application/json")
                    .json(&body);
                if let Some(h) = &auth_header {
                    req = req.header("authorization", h);
                }
                match req.send().await {
                    Ok(r) if !r.status().is_success() => {
                        tracing::warn!(target: "analytics.audit", status = %r.status(), "tyr audit POST non-2xx")
                    }
                    Err(e) => {
                        tracing::warn!(target: "analytics.audit", error = %e, "tyr audit POST failed")
                    }
                    _ => {}
                }
            }
        });
        Self { tx }
    }
}

impl AuditSink for HttpTyrSink {
    fn record(&self, event: &AuditEvent) {
        // never block / panic on the query path; dropped only if drain is gone.
        let _ = self.tx.send(event.clone());
    }
}

/// Pick the audit sink from env: `TYR_AUDIT_URL` set → forward to Tyr over HTTP
/// (`TYR_AUDIT_TOKEN` optional → `Bearer` header); otherwise the `tracing` sink
/// (scraped). Opt-in, matching `mimir-well::HttpProvSink` ("when Tyr is up").
/// Call inside a Tokio runtime.
pub fn sink_from_env() -> Arc<dyn AuditSink> {
    match std::env::var("TYR_AUDIT_URL") {
        Ok(url) if !url.is_empty() => {
            let auth = std::env::var("TYR_AUDIT_TOKEN")
                .ok()
                .filter(|t| !t.is_empty())
                .map(|t| format!("Bearer {t}"));
            Arc::new(HttpTyrSink::spawn(url, auth))
        }
        _ => Arc::new(TracingAuditSink),
    }
}
