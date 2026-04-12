//! Trace Telemetry Engine for the Trio RAG Pipeline
//!
//! Captures per-step execution metadata (duration, parameters, input/output summaries)
//! for the Pipeline Trace Visualizer in the Dashboard UI.
//!
//! Usage: Create a `TraceCollector`, push events as each retrieval stage completes,
//! then call `.finish()` to get the final `Vec<TraceEvent>`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

/// A single trace event representing one step in the retrieval pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Human-readable step name (e.g. "Vector Search", "Tree Pruning")
    pub step: String,
    /// Execution status: "success" | "timeout" | "error" | "skipped"
    pub status: String,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
    /// Step-specific configuration snapshot (top_k, model, weights, etc.)
    pub parameters: Value,
    /// Summary of what was sent into this step (truncated for bandwidth)
    pub input_summary: String,
    /// Summary of what came back from this step (truncated for bandwidth)
    pub output_summary: String,
    /// Count of items entering this step
    pub items_in: usize,
    /// Count of items leaving this step
    pub items_out: usize,
}

/// Collects trace events across the pipeline execution.
///
/// Each `fetch_*` function pushes its own `TraceEvent` into the collector.
/// At the end, `finish()` returns the full trace log.
#[derive(Debug, Clone)]
pub struct TraceCollector {
    events: Vec<TraceEvent>,
    enabled: bool,
}

impl TraceCollector {
    /// Create a new collector. If `enabled` is false, all push operations are no-ops.
    pub fn new(enabled: bool) -> Self {
        Self {
            events: Vec::with_capacity(if enabled { 6 } else { 0 }),
            enabled,
        }
    }

    /// Returns whether tracing is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Push a completed trace event.
    pub fn push(&mut self, event: TraceEvent) {
        if self.enabled {
            self.events.push(event);
        }
    }

    /// Consume the collector and return the trace log.
    /// Returns `None` if tracing was disabled.
    pub fn finish(self) -> Option<Vec<TraceEvent>> {
        if self.enabled {
            Some(self.events)
        } else {
            None
        }
    }
}

/// Helper to create a trace event for a successful retrieval step.
pub fn trace_success(
    step: &str,
    start: Instant,
    parameters: Value,
    input_summary: &str,
    output_summary: &str,
    items_in: usize,
    items_out: usize,
) -> TraceEvent {
    TraceEvent {
        step: step.to_string(),
        status: "success".to_string(),
        duration_ms: start.elapsed().as_millis() as u64,
        parameters,
        input_summary: truncate_str(input_summary, 500),
        output_summary: truncate_str(output_summary, 500),
        items_in,
        items_out,
    }
}

/// Helper to create a trace event for a timed-out step.
pub fn trace_timeout(step: &str, timeout_secs: u64) -> TraceEvent {
    TraceEvent {
        step: step.to_string(),
        status: "timeout".to_string(),
        duration_ms: timeout_secs * 1000,
        parameters: json!({"timeout_secs": timeout_secs}),
        input_summary: String::new(),
        output_summary: format!("Timed out after {}s", timeout_secs),
        items_in: 0,
        items_out: 0,
    }
}

/// Helper to create a trace event for a skipped step.
pub fn trace_skipped(step: &str, reason: &str) -> TraceEvent {
    TraceEvent {
        step: step.to_string(),
        status: "skipped".to_string(),
        duration_ms: 0,
        parameters: json!({}),
        input_summary: String::new(),
        output_summary: reason.to_string(),
        items_in: 0,
        items_out: 0,
    }
}

/// Helper to create a trace event for an errored step.
pub fn trace_error(step: &str, start: Instant, error: &str) -> TraceEvent {
    TraceEvent {
        step: step.to_string(),
        status: "error".to_string(),
        duration_ms: start.elapsed().as_millis() as u64,
        parameters: json!({}),
        input_summary: String::new(),
        output_summary: truncate_str(error, 500),
        items_in: 0,
        items_out: 0,
    }
}

/// Truncate a string to a max length, appending "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_disabled() {
        let collector = TraceCollector::new(false);
        assert!(!collector.is_enabled());
        assert!(collector.finish().is_none());
    }

    #[test]
    fn test_collector_enabled() {
        let mut collector = TraceCollector::new(true);
        assert!(collector.is_enabled());

        let start = Instant::now();
        collector.push(trace_success(
            "Vector Search",
            start,
            json!({"top_k": 10}),
            "test query",
            "Found 5 results",
            1,
            5,
        ));

        let log = collector.finish().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].step, "Vector Search");
        assert_eq!(log[0].status, "success");
        assert_eq!(log[0].items_out, 5);
    }

    #[test]
    fn test_trace_timeout() {
        let event = trace_timeout("Tree Search", 45);
        assert_eq!(event.status, "timeout");
        assert_eq!(event.duration_ms, 45000);
    }

    #[test]
    fn test_trace_skipped() {
        let event = trace_skipped("Graph Search", "Weight is 0");
        assert_eq!(event.status, "skipped");
        assert_eq!(event.output_summary, "Weight is 0");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }
}
