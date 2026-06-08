//! Pure entity *resolution* — the "what should we call this?" step.
//!
//! Resolution finds the canonical name for an incoming entity by comparing it
//! against existing nodes **of the same type** (type-gating) through a
//! short-circuit chain: exact → fuzzy → semantic. It deliberately does **not**
//! merge anything; deciding whether two entities are the *same* is the job of
//! `super::scoring` + `super::gate` (the deduplication / identity step).
//!
//! Similar names are not strong enough evidence of identity — "Jensen Huang the
//! CEO" and a same-named doctor share a name and type but are different people.
//! Resolution only assigns a name and tracks aliases; identity is decided later.

use super::scoring::{cosine, fuzzy_ratio};
use super::Embedder;
use crate::services::dedup::normalize_text;

/// Maximum number of aliases retained on a node (alias-growth guard).
pub const MAX_ALIASES: usize = 64;

/// How a resolution match was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMethod {
    Exact,
    Fuzzy,
    Semantic,
}

/// An existing graph node considered as a naming candidate. Embeddings are the
/// ones stored on the `:Entity` node at ingest, so resolution never re-embeds
/// candidates.
#[derive(Debug, Clone)]
pub struct NameCandidate {
    pub canonical_name: String, // already normalized
    pub aliases: Vec<String>,   // already normalized
    pub entity_type: String,
    pub embedding: Vec<f32>,
}

/// Outcome of the resolution chain.
#[derive(Debug, Clone, PartialEq)]
pub enum NameResolution {
    /// Matched an existing same-type node; `canonical_name` is that node's name.
    /// If the incoming surface form differs, `alias_to_add` carries the
    /// normalized form the caller should append to that node's alias set.
    Matched {
        canonical_name: String,
        via: MatchMethod,
        score: f32,
        alias_to_add: Option<String>,
    },
    /// No confident match — caller creates a new node with this canonical name.
    New { canonical_name: String },
}

/// Normalize an entity name for matching. Thin wrapper over the shared
/// `dedup::normalize_text` so resolution and chunk-dedup stay consistent.
pub fn normalize_entity_name(name: &str) -> String {
    normalize_text(name)
}

/// Merge `incoming` (plus the source node's own `name`) into an existing alias
/// set: case-insensitive dedup (via normalization), excludes the canonical name
/// itself, capped at [`MAX_ALIASES`]. Pure.
pub fn merge_alias_set(
    canonical_name: &str,
    existing: &[String],
    incoming: &[String],
) -> Vec<String> {
    let canon = normalize_entity_name(canonical_name);
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(canon.clone());
    for a in existing.iter().chain(incoming.iter()) {
        let n = normalize_entity_name(a);
        if n.is_empty() || seen.contains(&n) {
            continue;
        }
        seen.insert(n.clone());
        out.push(n);
        if out.len() >= MAX_ALIASES {
            break;
        }
    }
    out
}

/// A candidate survivor name when choosing a canonical form (used at merge time).
#[derive(Debug, Clone)]
pub struct CanonicalCandidate {
    pub name: String,
    pub has_ontology_code: bool,
    pub frequency: u32,
}

/// Choose the canonical name among candidates: prefer an ontology-coded form,
/// then the longest such, then the most frequent, then longest overall. Pure.
pub fn select_canonical_name(candidates: &[CanonicalCandidate]) -> Option<String> {
    candidates
        .iter()
        .max_by(|a, b| {
            a.has_ontology_code
                .cmp(&b.has_ontology_code)
                .then(a.frequency.cmp(&b.frequency))
                .then(a.name.chars().count().cmp(&b.name.chars().count()))
        })
        .map(|c| c.name.clone())
}

/// Run the type-gated short-circuit resolution chain.
///
/// `fuzzy_threshold` / `semantic_threshold` are the minimum similarities for a
/// fuzzy / semantic match. The query is embedded lazily — only if exact and
/// fuzzy both fail — so the common case costs no embedding call.
pub async fn resolve_chain(
    raw_name: &str,
    entity_type: &str,
    candidates: &[NameCandidate],
    embedder: &dyn Embedder,
    fuzzy_threshold: f32,
    semantic_threshold: f32,
) -> anyhow::Result<NameResolution> {
    let query = normalize_entity_name(raw_name);

    // Type-gating: only ever compare against the same entity_type.
    let same_type: Vec<&NameCandidate> = candidates
        .iter()
        .filter(|c| c.entity_type == entity_type)
        .collect();

    // 1. Exact — canonical name or any alias equals the query.
    for c in &same_type {
        if c.canonical_name == query || c.aliases.iter().any(|a| a == &query) {
            return Ok(NameResolution::Matched {
                canonical_name: c.canonical_name.clone(),
                via: MatchMethod::Exact,
                score: 1.0,
                alias_to_add: None,
            });
        }
    }

    // 2. Fuzzy — best canonical-name ratio above threshold.
    let mut best_fuzzy: Option<(&NameCandidate, f32)> = None;
    for c in &same_type {
        let r = fuzzy_ratio(&query, &c.canonical_name);
        if r >= fuzzy_threshold && best_fuzzy.map_or(true, |(_, br)| r > br) {
            best_fuzzy = Some((c, r));
        }
    }
    if let Some((c, score)) = best_fuzzy {
        return Ok(NameResolution::Matched {
            canonical_name: c.canonical_name.clone(),
            via: MatchMethod::Fuzzy,
            score,
            alias_to_add: Some(query),
        });
    }

    // 3. Semantic — embed the query lazily, compare against stored embeddings.
    if !same_type.is_empty() {
        let q_emb = embedder.embed(&query).await?;
        let mut best_sem: Option<(&NameCandidate, f32)> = None;
        for c in &same_type {
            if c.embedding.len() != q_emb.len() {
                continue; // dimension drift — skip rather than mis-compare
            }
            let sim = cosine(&q_emb, &c.embedding)?;
            if sim >= semantic_threshold && best_sem.map_or(true, |(_, bs)| sim > bs) {
                best_sem = Some((c, sim));
            }
        }
        if let Some((c, score)) = best_sem {
            return Ok(NameResolution::Matched {
                canonical_name: c.canonical_name.clone(),
                via: MatchMethod::Semantic,
                score,
                alias_to_add: Some(query),
            });
        }
    }

    // 4. No confident match.
    Ok(NameResolution::New {
        canonical_name: query,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// Deterministic stub: maps a fixed set of strings to vectors, else zeros.
    struct StubEmbedder {
        map: std::collections::HashMap<String, Vec<f32>>,
        dim: usize,
    }
    impl StubEmbedder {
        fn new(dim: usize) -> Self {
            Self { map: Default::default(), dim }
        }
        fn with(mut self, text: &str, v: Vec<f32>) -> Self {
            self.map.insert(text.to_string(), v);
            self
        }
    }
    #[async_trait]
    impl Embedder for StubEmbedder {
        async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
            Ok(self
                .map
                .get(text)
                .cloned()
                .unwrap_or_else(|| vec![0.0; self.dim]))
        }
        fn model_id(&self) -> &str {
            "stub"
        }
        fn dim(&self) -> usize {
            self.dim
        }
    }

    fn cand(name: &str, ty: &str, emb: Vec<f32>) -> NameCandidate {
        NameCandidate {
            canonical_name: normalize_entity_name(name),
            aliases: vec![],
            entity_type: ty.to_string(),
            embedding: emb,
        }
    }

    #[test]
    fn normalize_collapses_and_lowercases() {
        assert_eq!(normalize_entity_name("  JPMorgan   Chase "), "jpmorgan chase");
    }

    #[test]
    fn merge_alias_dedups_and_excludes_canonical() {
        let out = merge_alias_set(
            "John Smith",
            &["jon smith".into()],
            &["John Smith".into(), "JOHN  SMITH".into(), "j. smith".into()],
        );
        assert!(out.contains(&"jon smith".to_string()));
        assert!(out.contains(&"j. smith".to_string()));
        assert!(!out.contains(&"john smith".to_string()), "canonical excluded");
        assert_eq!(out.iter().filter(|a| *a == "jon smith").count(), 1, "deduped");
    }

    #[test]
    fn merge_alias_caps_growth() {
        let many: Vec<String> = (0..200).map(|i| format!("alias {i}")).collect();
        let out = merge_alias_set("canon", &[], &many);
        assert_eq!(out.len(), MAX_ALIASES);
    }

    #[test]
    fn select_canonical_prefers_ontology_coded() {
        let c = vec![
            CanonicalCandidate { name: "htn".into(), has_ontology_code: false, frequency: 99 },
            CanonicalCandidate { name: "hypertension".into(), has_ontology_code: true, frequency: 3 },
        ];
        assert_eq!(select_canonical_name(&c).unwrap(), "hypertension");
    }

    #[tokio::test]
    async fn exact_match_short_circuits() {
        let cands = vec![cand("Aspirin", "DRUG", vec![1.0, 0.0])];
        let emb = StubEmbedder::new(2);
        let r = resolve_chain("  aspirin ", "DRUG", &cands, &emb, 0.9, 0.9)
            .await
            .unwrap();
        assert_eq!(
            r,
            NameResolution::Matched {
                canonical_name: "aspirin".into(),
                via: MatchMethod::Exact,
                score: 1.0,
                alias_to_add: None,
            }
        );
    }

    #[tokio::test]
    async fn type_gating_blocks_cross_type_match() {
        // Same name, different type → must NOT match; becomes New.
        let cands = vec![cand("apple", "ORG", vec![1.0, 0.0])];
        let emb = StubEmbedder::new(2);
        let r = resolve_chain("apple", "FOOD", &cands, &emb, 0.9, 0.9)
            .await
            .unwrap();
        assert_eq!(r, NameResolution::New { canonical_name: "apple".into() });
    }

    #[tokio::test]
    async fn fuzzy_match_adds_alias() {
        let cands = vec![cand("john smith", "PERSON", vec![0.0, 0.0])];
        let emb = StubEmbedder::new(2);
        let r = resolve_chain("Jon Smith", "PERSON", &cands, &emb, 0.85, 0.99)
            .await
            .unwrap();
        match r {
            NameResolution::Matched { canonical_name, via, alias_to_add, .. } => {
                assert_eq!(canonical_name, "john smith");
                assert_eq!(via, MatchMethod::Fuzzy);
                assert_eq!(alias_to_add, Some("jon smith".to_string()));
            }
            other => panic!("expected fuzzy match, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn semantic_match_when_lexically_distant() {
        // Surface forms differ lexically (fuzzy fails) but embeddings align.
        let cands = vec![cand("myocardial infarction", "DISEASE", vec![1.0, 0.0, 0.0])];
        let emb = StubEmbedder::new(3).with("heart attack", vec![0.99, 0.1, 0.0]);
        let r = resolve_chain("heart attack", "DISEASE", &cands, &emb, 0.9, 0.9)
            .await
            .unwrap();
        match r {
            NameResolution::Matched { canonical_name, via, alias_to_add, .. } => {
                assert_eq!(canonical_name, "myocardial infarction");
                assert_eq!(via, MatchMethod::Semantic);
                assert_eq!(alias_to_add, Some("heart attack".to_string()));
            }
            other => panic!("expected semantic match, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_match_creates_new() {
        let cands = vec![cand("aspirin", "DRUG", vec![1.0, 0.0])];
        let emb = StubEmbedder::new(2); // unknown query → zero vector, cosine 0
        let r = resolve_chain("warfarin", "DRUG", &cands, &emb, 0.9, 0.9)
            .await
            .unwrap();
        assert_eq!(r, NameResolution::New { canonical_name: "warfarin".into() });
    }
}