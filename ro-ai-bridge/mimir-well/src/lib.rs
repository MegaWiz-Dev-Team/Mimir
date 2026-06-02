//! Mimir Well — Tulving 3-tier memory artifact accumulation primitive.
//!
//! Design: see Asgard/docs/decisions/ADR-011-mimir-well-memory-artifacts.md
//! Schema: see migrations/sprint56_mimir_well_schema.sql
//!
//! Module layout follows ADR-011 sections:
//!   - `model`        — Artifact, Tier, Surface, Kind types (D1)
//!   - `writer`       — append-only writer (PROV-O emit → Tyr)
//!   - `reader`       — search + supersession chain navigation
//!   - `promotion`    — Bifrost session → tier classification → write (D5)
//!   - `consolidator` — async dedup worker (DryRun/Apply, D3)
//!   - `touched`      — :TOUCHED edge materialization from heimdall-trace (D2)
//!   - `pole_o`       — POLE+O label decoration for asgard_insurance (D4)
//!
//! Scaffolding status (2026-05-23): module stubs only. Implementation gated
//! by Sprint 56 kickoff after S1 Go/No-Go (2026-06-12).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod model;
pub mod writer;
pub mod reader;
pub mod promotion;
pub mod consolidator;
pub mod touched;
pub mod pole_o;

pub use model::{Artifact, ArtifactId, Kind, Surface, Tier, ConsolidationState};

/// Crate-wide error type. See module-level errors for variants.
#[derive(Debug, thiserror::Error)]
pub enum WellError {
    /// Underlying SQL failure.
    #[error("sql: {0}")]
    Sql(#[from] sqlx::Error),

    /// Provenance contract violation (missing PROV-O fields).
    #[error("provenance: {0}")]
    Provenance(String),

    /// Tier/surface mismatch — short<>episodic, long<>semantic, reasoning<>procedural.
    #[error("tier/surface mismatch: tier={tier:?} surface={surface:?}")]
    TierSurfaceMismatch {
        /// Stored tier.
        tier: Tier,
        /// Submitted surface label.
        surface: Surface,
    },

    /// Artifact does not exist.
    #[error("artifact not found: {0}")]
    NotFound(ArtifactId),

    /// Tenant scope mismatch — caller cannot operate across tenants.
    #[error("tenant scope: expected {expected}, got {got}")]
    TenantScope {
        /// Tenant the caller is bound to.
        expected: String,
        /// Tenant on the artifact being accessed.
        got: String,
    },

    /// Neo4j driver failure (touched materialization, POLE+O label writes).
    #[error("neo4j: {0}")]
    Neo4j(#[from] neo4rs::Error),

    /// HTTP client failure (TierClassifier → Heimdall, HttpProvSink → Tyr).
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// Unexpected response shape from upstream (Heimdall, Tyr).
    #[error("upstream: {0}")]
    Upstream(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, WellError>;
