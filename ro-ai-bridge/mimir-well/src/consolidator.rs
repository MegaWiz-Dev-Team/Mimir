//! Async consolidation worker — dedup, supersede, contradict.
//!
//! ADR-011 §D3. Two modes: DryRun (logs intended actions, no writes) and
//! Apply (writes, requires explicit --confirm-apply at the runner).
//!
//! Three policy tiers (within tenant + tier):
//!   - content_hash match → auto-merge, no curator
//!   - cosine ≥ 0.98       → auto-merge, post-hoc audit log
//!   - 0.92 ≤ cosine < 0.98 → enqueue for mimir-curator review
//!   - CONTRADICTS edge    → high-priority curator queue + recursive research

/// Consolidator run mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Logs what would happen; never writes.
    #[default]
    DryRun,
    /// Writes merges + supersession edges + Label Studio tasks.
    Apply,
}

/// Per-pair decision the consolidator wants to take.
#[derive(Debug, Clone)]
pub struct ConsolidationAction {
    /// Tenant.
    pub tenant_id: String,
    /// Smaller ULID lex-wise.
    pub artifact_a: ulid::Ulid,
    /// Larger ULID lex-wise.
    pub artifact_b: ulid::Ulid,
    /// Cosine similarity over embeddings.
    pub similarity: f32,
    /// Action verb.
    pub kind: ActionKind,
}

/// What the consolidator decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// `content_hash` matched — auto-merge, no curator.
    AutoMergeHash,
    /// cosine ≥ 0.98 — auto-merge with post-hoc audit.
    AutoMergeHighSim,
    /// 0.92 ≤ cosine < 0.98 — enqueue for review.
    EnqueueReview,
    /// Contradiction detected — high-prio curator queue.
    EnqueueContradiction,
}

impl ActionKind {
    /// Threshold-based classification. Encapsulated so the policy lives in
    /// one place; thresholds can be tuned in Sprint 57+ after first month of
    /// curator data.
    pub fn classify(similarity: f32, content_hash_match: bool, is_contradiction: bool) -> Self {
        if is_contradiction {
            return Self::EnqueueContradiction;
        }
        if content_hash_match {
            return Self::AutoMergeHash;
        }
        if similarity >= 0.98 {
            Self::AutoMergeHighSim
        } else {
            Self::EnqueueReview
        }
    }
}

use crate::{Result, WellError};
use crate::reader::WellReader;
use crate::writer::WellWriter;

/// Report from one consolidator pass.
#[derive(Debug, Default, Clone)]
pub struct ConsolidationReport {
    /// Pairs auto-merged by content_hash match.
    pub auto_merged_hash: u64,
    /// Pairs auto-merged by high embedding similarity (≥ 0.98). V2 — currently 0.
    pub auto_merged_high_sim: u64,
    /// Pairs enqueued for curator review.
    pub enqueued_review: u64,
    /// Contradiction pairs enqueued (high priority).
    pub enqueued_contradiction: u64,
    /// Whether writes actually happened.
    pub mode: RunMode,
}

/// Consolidator service. Wraps reader + writer; talks to the queue table.
pub struct Consolidator {
    pool: sqlx::MySqlPool,
    writer: std::sync::Arc<WellWriter>,
    #[allow(dead_code)] // V2 wires reader for near-dup embedding similarity scan
    reader: std::sync::Arc<WellReader>,
}

impl Consolidator {
    /// Construct.
    pub fn new(
        pool: sqlx::MySqlPool,
        writer: std::sync::Arc<WellWriter>,
        reader: std::sync::Arc<WellReader>,
    ) -> Self {
        Self { pool, writer, reader }
    }

    /// Run one consolidation pass over a tenant.
    ///
    /// V1 scope (this impl):
    ///   - Hash-match dedup: find pairs with same `(tenant, tier, content_hash)`,
    ///     supersede newer → older.
    ///   - Higher-similarity / contradiction detection is V2 (needs embedding
    ///     index + LLM judge respectively).
    pub async fn run_pass(
        &self,
        tenant_id: &str,
        mode: RunMode,
    ) -> Result<ConsolidationReport> {
        let mut report = ConsolidationReport { mode, ..Default::default() };

        // Find content_hash groups with multiple fresh artifacts.
        let dup_groups: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT content_hash, tier, COUNT(*) AS n \
             FROM memory_artifact \
             WHERE tenant_id = ? AND consolidation_state = 'fresh' \
             GROUP BY content_hash, tier \
             HAVING n > 1",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        for (hash, tier, _n) in &dup_groups {
            // Fetch all duplicates for this (hash, tier), oldest first.
            let ids: Vec<String> = sqlx::query_scalar(
                "SELECT id FROM memory_artifact \
                 WHERE tenant_id = ? AND content_hash = ? AND tier = ? AND consolidation_state = 'fresh' \
                 ORDER BY created_at ASC",
            )
            .bind(tenant_id)
            .bind(hash)
            .bind(tier)
            .fetch_all(&self.pool)
            .await?;

            if ids.len() < 2 {
                continue;
            }
            let canonical = ids[0]
                .parse::<crate::model::ArtifactId>()
                .map_err(|e| WellError::Provenance(format!("canonical ulid: {e}")))?;

            for id_s in &ids[1..] {
                let action = ActionKind::classify(1.0, true, false);
                debug_assert_eq!(action, ActionKind::AutoMergeHash);

                let dup = id_s
                    .parse::<crate::model::ArtifactId>()
                    .map_err(|e| WellError::Provenance(format!("dup ulid: {e}")))?;

                match mode {
                    RunMode::DryRun => {
                        tracing::info!(
                            tenant = tenant_id,
                            canonical = %canonical,
                            duplicate = %dup,
                            "consolidator dry-run: would supersede duplicate → canonical"
                        );
                    }
                    RunMode::Apply => {
                        // Supersede newer duplicate with older canonical.
                        self.writer.supersede(tenant_id, dup, canonical).await?;
                    }
                }
                report.auto_merged_hash += 1;
            }
        }
        Ok(report)
    }

    /// Enqueue a pair for curator review. V2 will call this from a
    /// near-dup embedding scan; V1 keeps it public for direct enqueue testing.
    pub async fn enqueue_review(
        &self,
        tenant_id: &str,
        a: crate::model::ArtifactId,
        b: crate::model::ArtifactId,
        similarity: f32,
        is_contradiction: bool,
    ) -> Result<i64> {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let kind = if is_contradiction { "contradiction" } else { "near_dup" };
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO well_consolidation_queue \
             (tenant_id, artifact_a, artifact_b, similarity, kind) \
             VALUES (?, ?, ?, ?, ?) \
             RETURNING id",
        )
        .bind(tenant_id)
        .bind(lo.to_string())
        .bind(hi.to_string())
        .bind(similarity as f64)
        .bind(kind)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_contradiction_dominates() {
        assert_eq!(
            ActionKind::classify(0.999, true, true),
            ActionKind::EnqueueContradiction
        );
    }

    #[test]
    fn classify_hash_match_before_high_sim() {
        assert_eq!(
            ActionKind::classify(0.999, true, false),
            ActionKind::AutoMergeHash
        );
    }

    #[test]
    fn classify_high_sim_boundary() {
        assert_eq!(
            ActionKind::classify(0.98, false, false),
            ActionKind::AutoMergeHighSim
        );
        assert_eq!(
            ActionKind::classify(0.9799, false, false),
            ActionKind::EnqueueReview
        );
    }

    #[test]
    fn classify_review_band() {
        assert_eq!(
            ActionKind::classify(0.92, false, false),
            ActionKind::EnqueueReview
        );
    }
}
