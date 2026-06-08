//! Skuggi PII gate for ingested datasets.
//!
//! A dataset is `Pending` until scanned. We scan sampled cell text with the
//! shared `skuggi-core` Tier-1 engine; any detection flips it to `Flagged`
//! (with the categories), otherwise `Clean`. Per ADR-024 a dataset must not be
//! queryable until it leaves `Pending`, and `Flagged` datasets are quarantined
//! for review.

use crate::engine::Engine;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PiiStatus {
    Pending,
    Clean,
    Flagged { categories: Vec<String> },
}

/// Scan a batch of sample strings; returns the gate decision.
pub fn scan_samples(samples: &[String]) -> PiiStatus {
    let mut cats: BTreeSet<String> = BTreeSet::new();
    for s in samples {
        for c in skuggi_core::scan_categories(s) {
            cats.insert(c.to_string());
        }
    }
    if cats.is_empty() {
        PiiStatus::Clean
    } else {
        PiiStatus::Flagged {
            categories: cats.into_iter().collect(),
        }
    }
}

/// Pull up to `limit` text values from `column` of `table`, then gate them.
/// Non-text columns are cast to text by the read-only query path.
pub fn gate_table_column(
    engine: &Engine,
    table: &str,
    column: &str,
    limit: usize,
) -> Result<PiiStatus> {
    // table/column validated by the query path's describe + identifier safety upstream;
    // here we go through query_readonly which is mutation-safe.
    let sql = format!("SELECT \"{}\" FROM \"{}\"", column.replace('"', "\"\""), table.replace('"', "\"\""));
    let res = engine.query_readonly(&sql, limit)?;
    let samples: Vec<String> = res
        .rows
        .into_iter()
        .filter_map(|mut r| r.pop().flatten())
        .collect();
    Ok(scan_samples(&samples))
}
