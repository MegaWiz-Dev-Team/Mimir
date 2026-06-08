//! Pure scoring primitives for entity deduplication.
//!
//! These functions answer the *identity* question ("is this the same real-world
//! entity?") and are deliberately free of any DB / network types so they can be
//! unit-tested inline, mirroring `services::dedup`.
//!
//! The combined dedup score blends a semantic signal (embedding cosine) with a
//! lexical signal (fuzzy name ratio): `0.7 * cosine + 0.3 * fuzzy`. The result is
//! routed into one of three bands. See [`band`].

/// Routing decision produced by [`band`] from a combined dedup score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// Near-certain identity (>= 0.95): eligible for auto-merge (still subject to
    /// the medical-safety gate in `super::gate`).
    AutoMerge,
    /// Uncertain middle (0.85..0.95): flag the pair for human review.
    Review,
    /// Weak evidence (< 0.85): treat as a brand-new entity.
    New,
}

/// Upper threshold: at or above this, identity is near-certain.
pub const AUTO_MERGE_THRESHOLD: f32 = 0.95;
/// Lower threshold: at or above this (but below auto-merge), escalate to a human.
pub const REVIEW_THRESHOLD: f32 = 0.85;

/// Weight applied to the embedding (semantic) component of the dedup score.
pub const EMBED_WEIGHT: f32 = 0.7;
/// Weight applied to the fuzzy (lexical) component of the dedup score.
pub const FUZZY_WEIGHT: f32 = 0.3;

/// Cosine similarity between two equal-length vectors, clamped to `[0, 1]`.
///
/// Returns an error on dimension mismatch rather than silently comparing — this
/// is the guard against embedding-model / dimension drift (a vector produced by
/// a different `EMBED_MODEL` must never be compared as if compatible).
pub fn cosine(a: &[f32], b: &[f32]) -> anyhow::Result<f32> {
    if a.len() != b.len() {
        return Err(anyhow::anyhow!(
            "cosine: dimension mismatch ({} vs {}) — embedding model/version drift?",
            a.len(),
            b.len()
        ));
    }
    if a.is_empty() {
        return Err(anyhow::anyhow!("cosine: empty vectors"));
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return Ok(0.0);
    }
    let sim = dot / (na.sqrt() * nb.sqrt());
    // Cosine of real vectors can drift slightly outside [-1, 1] from fp error;
    // we only care about positive similarity for identity, so clamp to [0, 1].
    Ok(sim.clamp(0.0, 1.0))
}

/// Levenshtein edit distance between two char sequences.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Lexical similarity of two strings in `[0, 1]`, derived from normalized
/// Levenshtein distance. `1.0` = identical, `0.0` = maximally different.
///
/// Inputs are expected to be already normalized (see `super::naming` /
/// `services::dedup::normalize_text`); this function does not normalize.
pub fn fuzzy_ratio(a: &str, b: &str) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let max_len = a.chars().count().max(b.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(a, b);
    1.0 - (dist as f32 / max_len as f32)
}

/// Blend the semantic and lexical signals into a single dedup score in `[0, 1]`.
pub fn combined_score(cosine_sim: f32, fuzzy: f32) -> f32 {
    (EMBED_WEIGHT * cosine_sim + FUZZY_WEIGHT * fuzzy).clamp(0.0, 1.0)
}

/// Route a combined dedup score into an action band.
pub fn band(score: f32) -> Band {
    if score >= AUTO_MERGE_THRESHOLD {
        Band::AutoMerge
    } else if score >= REVIEW_THRESHOLD {
        Band::Review
    } else {
        Band::New
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![0.1, 0.2, 0.3, 0.4];
        assert!(approx(cosine(&v, &v).unwrap(), 1.0));
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(approx(cosine(&a, &b).unwrap(), 0.0));
    }

    #[test]
    fn cosine_negative_clamped_to_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!(approx(cosine(&a, &b).unwrap(), 0.0));
    }

    #[test]
    fn cosine_dim_mismatch_errors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert!(cosine(&a, &b).is_err(), "dimension drift must error, not compare");
    }

    #[test]
    fn cosine_empty_errors() {
        let e: Vec<f32> = vec![];
        assert!(cosine(&e, &e).is_err());
    }

    #[test]
    fn fuzzy_identical_is_one() {
        assert!(approx(fuzzy_ratio("aspirin", "aspirin"), 1.0));
    }

    #[test]
    fn fuzzy_both_empty_is_one() {
        assert!(approx(fuzzy_ratio("", ""), 1.0));
    }

    #[test]
    fn fuzzy_one_edit() {
        // "jon smith" vs "john smith": insert 'h' = 1 edit over 10 chars.
        let r = fuzzy_ratio("jon smith", "john smith");
        assert!(r > 0.85 && r < 1.0, "got {r}");
    }

    #[test]
    fn fuzzy_completely_different_is_low() {
        let r = fuzzy_ratio("paris", "tokyo");
        assert!(r < 0.5, "got {r}");
    }

    #[test]
    fn combined_score_weights() {
        // 0.7*1.0 + 0.3*0.0 = 0.7
        assert!(approx(combined_score(1.0, 0.0), 0.7));
        // 0.7*0.0 + 0.3*1.0 = 0.3
        assert!(approx(combined_score(0.0, 1.0), 0.3));
        // both perfect = 1.0
        assert!(approx(combined_score(1.0, 1.0), 1.0));
    }

    #[test]
    fn band_thresholds() {
        assert_eq!(band(0.99), Band::AutoMerge);
        assert_eq!(band(0.95), Band::AutoMerge); // boundary inclusive
        assert_eq!(band(0.90), Band::Review);
        assert_eq!(band(0.85), Band::Review); // boundary inclusive
        assert_eq!(band(0.84), Band::New);
        assert_eq!(band(0.0), Band::New);
    }

    #[test]
    fn paris_problem_lands_in_review_not_merge() {
        // Two LOCATION nodes both named "paris" (France vs Texas): identical names
        // (fuzzy 1.0) but distinct context embeddings. With a moderate cosine the
        // combined score must NOT reach auto-merge — a human decides.
        let cos = 0.80; // similar-but-not-identical context
        let score = combined_score(cos, 1.0); // 0.7*0.8 + 0.3*1.0 = 0.86
        assert_eq!(band(score), Band::Review, "score was {score}");
    }
}