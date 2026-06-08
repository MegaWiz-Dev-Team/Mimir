//! Entity resolution + deduplication for the knowledge graph.
//!
//! Keeps the graph clean as it grows by separating two distinct decisions that
//! are commonly (and dangerously) collapsed into one fuzzy check:
//!
//! 1. **Resolution** ([`naming`]) — "what should we call this?" Assigns a
//!    canonical name + alias set via a type-gated exact→fuzzy→semantic chain.
//!    Never merges.
//! 2. **Deduplication** ([`scoring`] + [`gate`]) — "is this the same real-world
//!    entity?" Blends embedding cosine + fuzzy ratio into a confidence band, with
//!    a medical-safety gate that refuses to auto-merge clinical entities without
//!    an ontology-code match.
//!
//! Phase 1 is **flag-only**: uncertain pairs are proposed as `DUPLICATE_OF`
//! review edges for a human; nothing is ever silently merged. Auto-merge,
//! tombstoning, and the nightly "dream pass" are Phase 2.
//!
//! The decision logic ([`naming`], [`scoring`], [`gate`], and [`plan_phase1_action`])
//! is pure and unit-tested with no DB/network. Only [`cypher`] (string builders)
//! and the integration layer touch Neo4j / Heimdall.

pub mod cypher;
pub mod gate;
pub mod naming;
pub mod scoring;

use naming::NameResolution;
use scoring::Band;

/// Abstraction over the embedding backend (Heimdall in production, a stub in
/// tests). Defined here so the pure resolution chain stays unit-testable without
/// exposing `rag_engine`'s private `get_embedding` as a public API.
#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    /// Embed `text`, routing through the Heimdall gateway (never a third-party
    /// provider directly).
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
    /// Model identifier, stamped onto stored vectors + review edges for drift detection.
    fn model_id(&self) -> &str;
    /// Embedding dimensionality.
    fn dim(&self) -> usize;
}

/// What the Phase-1 pipeline should do with an incoming entity.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// No confident match — create a fresh node whose canonical name is its own.
    Create { canonical_name: String },
    /// Exact match to an existing node — assert its canonical name and, if the
    /// surface form differed, record the new alias. Identity is certain; no review.
    AssignCanonical {
        canonical_name: String,
        alias_to_add: Option<String>,
    },
    /// Non-exact (fuzzy/semantic) match — create the node but propose a
    /// `DUPLICATE_OF` edge for human review. In Phase 1 we never auto-merge.
    ///
    /// `resolve_chain`'s fuzzy/semantic thresholds are themselves the gate: any
    /// match that clears them is a credible duplicate worth a human's eyes (a
    /// semantic synonym like "heart attack"↔"myocardial infarction" is a perfect
    /// example — high cosine, near-zero lexical overlap). The blended dedup
    /// `band` is carried only as recorded confidence / for Phase-2 auto-merge; it
    /// does NOT suppress flagging, since the combined `0.7·cos + 0.3·fuzzy` score
    /// would otherwise hide exactly those lexically-distant synonyms.
    FlagDuplicate {
        canonical_name: String,
        band: Band,
    },
}

/// Pure Phase-1 decision: map a name resolution onto an [`Action`]. `dedup_band`
/// is the post-gate blended band, recorded on the review edge as confidence.
/// Flag-only — never returns a merge.
pub fn plan_phase1_action(resolution: &NameResolution, dedup_band: Band) -> Action {
    match resolution {
        NameResolution::New { canonical_name } => Action::Create {
            canonical_name: canonical_name.clone(),
        },
        NameResolution::Matched {
            canonical_name,
            via,
            alias_to_add,
            ..
        } => {
            if *via == naming::MatchMethod::Exact {
                // Identity certain — assert canonical, add alias if surface differed.
                Action::AssignCanonical {
                    canonical_name: canonical_name.clone(),
                    alias_to_add: alias_to_add.clone(),
                }
            } else {
                // Above-threshold fuzzy/semantic match → surface for human review.
                Action::FlagDuplicate {
                    canonical_name: canonical_name.clone(),
                    band: dedup_band,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::naming::{MatchMethod, NameResolution};
    use super::scoring::Band;
    use super::*;

    #[test]
    fn new_resolution_creates() {
        let r = NameResolution::New { canonical_name: "warfarin".into() };
        assert_eq!(
            plan_phase1_action(&r, Band::New),
            Action::Create { canonical_name: "warfarin".into() }
        );
    }

    #[test]
    fn exact_match_assigns_canonical_no_review() {
        let r = NameResolution::Matched {
            canonical_name: "aspirin".into(),
            via: MatchMethod::Exact,
            score: 1.0,
            alias_to_add: None,
        };
        assert_eq!(
            plan_phase1_action(&r, Band::AutoMerge),
            Action::AssignCanonical { canonical_name: "aspirin".into(), alias_to_add: None }
        );
    }

    #[test]
    fn semantic_match_in_review_band_is_flagged_not_merged() {
        let r = NameResolution::Matched {
            canonical_name: "myocardial infarction".into(),
            via: MatchMethod::Semantic,
            score: 0.9,
            alias_to_add: Some("heart attack".into()),
        };
        // Even at the auto-merge band, Phase 1 only flags — never merges.
        assert_eq!(
            plan_phase1_action(&r, Band::AutoMerge),
            Action::FlagDuplicate { canonical_name: "myocardial infarction".into(), band: Band::AutoMerge }
        );
    }

    #[test]
    fn nonexact_match_is_flagged_even_when_blended_band_is_low() {
        // A semantic synonym scores high on cosine but near-zero on fuzzy, so the
        // blended band can be New — yet it is exactly the duplicate a human should
        // review. resolve_chain's thresholds already gated it; we must not suppress.
        let r = NameResolution::Matched {
            canonical_name: "aspirin".into(),
            via: MatchMethod::Fuzzy,
            score: 0.86,
            alias_to_add: Some("aspirine".into()),
        };
        assert_eq!(
            plan_phase1_action(&r, Band::New),
            Action::FlagDuplicate { canonical_name: "aspirin".into(), band: Band::New }
        );
    }
}
