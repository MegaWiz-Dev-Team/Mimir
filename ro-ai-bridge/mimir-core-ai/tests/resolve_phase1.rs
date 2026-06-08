//! Integration-level smoke tests for the Phase-1 entity resolution pipeline.
//!
//! These exercise the *pure* public surface of `services::resolve` end to end.
//! They live as an integration test (compiled against the library WITHOUT
//! `cfg(test)`) so they run independently of any other module's inline unit
//! tests. The canonical, fine-grained TDD assertions live inline in each
//! `resolve/*.rs` module.

use async_trait::async_trait;
use mimir_core_ai::services::resolve::{
    cypher, gate,
    naming::{self, NameCandidate, NameResolution},
    plan_phase1_action,
    scoring::{self, Band},
    Action, Embedder,
};

/// Deterministic embedder for tests.
struct StubEmbedder {
    map: std::collections::HashMap<String, Vec<f32>>,
    dim: usize,
}
impl StubEmbedder {
    fn new(dim: usize) -> Self {
        Self { map: Default::default(), dim }
    }
    fn with(mut self, t: &str, v: Vec<f32>) -> Self {
        self.map.insert(t.into(), v);
        self
    }
}
#[async_trait]
impl Embedder for StubEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(self.map.get(text).cloned().unwrap_or_else(|| vec![0.0; self.dim]))
    }
    fn model_id(&self) -> &str {
        "stub"
    }
    fn dim(&self) -> usize {
        self.dim
    }
}

#[test]
fn scoring_bands_and_weights() {
    assert!((scoring::combined_score(1.0, 0.0) - 0.7).abs() < 1e-5);
    assert_eq!(scoring::band(0.96), Band::AutoMerge);
    assert_eq!(scoring::band(0.88), Band::Review);
    assert_eq!(scoring::band(0.5), Band::New);
    // Paris vs Paris,TX: identical name, similar-not-identical context → Review.
    let score = scoring::combined_score(0.80, 1.0);
    assert_eq!(scoring::band(score), Band::Review);
}

#[test]
fn cosine_guards_dimension_drift() {
    assert!(scoring::cosine(&[1.0, 2.0, 3.0], &[1.0, 2.0]).is_err());
}

#[test]
fn medical_gate_blocks_uncoded_automerge() {
    assert_eq!(gate::medical_gate(Band::AutoMerge, false, "DRUG"), Band::Review);
    assert_eq!(gate::medical_gate(Band::AutoMerge, true, "DRUG"), Band::AutoMerge);
    assert_eq!(gate::medical_gate(Band::AutoMerge, false, "ORG"), Band::AutoMerge);
}

#[test]
fn cypher_builders_are_tenant_scoped_and_flag_only() {
    let all = [
        cypher::build_store_embedding_cypher(),
        cypher::build_set_canonical_and_aliases_cypher(),
        cypher::build_find_candidates_cypher(),
        cypher::build_flag_duplicate_cypher(),
        cypher::build_review_queue_cypher(),
    ];
    for q in all {
        assert!(q.contains("tenant_id"));
    }
    let flag = cypher::build_flag_duplicate_cypher();
    assert!(flag.contains("DUPLICATE_OF"));
    assert!(!flag.contains("SAME_AS"));
    // Phase 1 is flag-only: no merge/tombstone builders exist.
    assert!(!all.iter().any(|q| q.contains("MERGED_INTO") || q.contains("Tombstoned")));
}

#[tokio::test]
async fn end_to_end_exact_match_assigns_canonical() {
    let cands = vec![NameCandidate {
        canonical_name: naming::normalize_entity_name("Aspirin"),
        aliases: vec![],
        entity_type: "DRUG".into(),
        embedding: vec![1.0, 0.0],
    }];
    let emb = StubEmbedder::new(2);
    let res = naming::resolve_chain("  ASPIRIN ", "DRUG", &cands, &emb, 0.9, 0.9)
        .await
        .unwrap();
    let action = plan_phase1_action(&res, Band::AutoMerge);
    assert_eq!(
        action,
        Action::AssignCanonical { canonical_name: "aspirin".into(), alias_to_add: None }
    );
}

#[tokio::test]
async fn end_to_end_semantic_dup_is_flagged_never_merged() {
    let cands = vec![NameCandidate {
        canonical_name: "myocardial infarction".into(),
        aliases: vec![],
        entity_type: "DISEASE".into(),
        embedding: vec![1.0, 0.0, 0.0],
    }];
    let emb = StubEmbedder::new(3).with("heart attack", vec![0.99, 0.05, 0.0]);
    let res = naming::resolve_chain("heart attack", "DISEASE", &cands, &emb, 0.95, 0.9)
        .await
        .unwrap();

    // Recompute the dedup band the way the orchestrator will, then gate it.
    let (cos, fuzzy) = match &res {
        NameResolution::Matched { score, .. } => (*score, scoring::fuzzy_ratio("heart attack", "myocardial infarction")),
        _ => panic!("expected a match"),
    };
    let raw_band = scoring::band(scoring::combined_score(cos, fuzzy));
    let gated = gate::medical_gate(raw_band, false, "DISEASE");

    let action = plan_phase1_action(&res, gated);
    match action {
        Action::FlagDuplicate { canonical_name, .. } => {
            assert_eq!(canonical_name, "myocardial infarction");
        }
        other => panic!("medical near-dup must be flagged for review, got {other:?}"),
    }
}

#[tokio::test]
async fn end_to_end_unrelated_entity_is_created() {
    let cands = vec![NameCandidate {
        canonical_name: "aspirin".into(),
        aliases: vec![],
        entity_type: "DRUG".into(),
        embedding: vec![1.0, 0.0],
    }];
    let emb = StubEmbedder::new(2);
    let res = naming::resolve_chain("warfarin", "DRUG", &cands, &emb, 0.9, 0.9)
        .await
        .unwrap();
    assert_eq!(plan_phase1_action(&res, Band::New), Action::Create { canonical_name: "warfarin".into() });
}
