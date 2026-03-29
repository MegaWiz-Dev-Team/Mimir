use std::collections::HashMap;

/// Simple BM25 sparse vector generator for hybrid search.
/// Generates sparse vectors using term frequency hashing.
///
/// This approach:
/// 1. Tokenizes text on whitespace + punctuation boundaries (Unicode-aware)
/// 2. Computes term frequencies
/// 3. Uses murmurhash-like hashing to map terms → sparse indices
/// 4. Returns Qdrant-compatible sparse vector format

const SPARSE_DIM_SPACE: u32 = 100_000;

/// Tokenize text into lowercase words, handling Thai + English + medical terms.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_string())
        .collect()
}

/// Simple string hash → sparse dimension index (deterministic, no external crate needed).
fn hash_term(term: &str) -> u32 {
    let mut h: u32 = 0;
    for b in term.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    h % SPARSE_DIM_SPACE
}

/// Represents a sparse vector for Qdrant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

/// Generate a BM25-style sparse vector from text content.
/// Each unique term gets a dimension (via hash), weighted by log(1 + tf).
pub fn text_to_sparse_vector(text: &str) -> SparseVector {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return SparseVector { indices: vec![], values: vec![] };
    }

    // Count term frequencies
    let mut tf: HashMap<String, u32> = HashMap::new();
    for tok in &tokens {
        *tf.entry(tok.clone()).or_insert(0) += 1;
    }

    // Build sparse vector: index = hash(term), value = log(1 + tf)
    let mut index_map: HashMap<u32, f32> = HashMap::new();
    for (term, count) in &tf {
        let idx = hash_term(term);
        let val = (1.0 + *count as f32).ln();
        // If hash collision, take the max
        let entry = index_map.entry(idx).or_insert(0.0);
        *entry = entry.max(val);
    }

    let mut pairs: Vec<(u32, f32)> = index_map.into_iter().collect();
    pairs.sort_by_key(|(idx, _)| *idx);

    SparseVector {
        indices: pairs.iter().map(|(i, _)| *i).collect(),
        values: pairs.iter().map(|(_, v)| *v).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sparse_vector() {
        let sv = text_to_sparse_vector("sleep apnea treatment options for patients");
        assert!(!sv.indices.is_empty());
        assert_eq!(sv.indices.len(), sv.values.len());
        // All values should be positive
        assert!(sv.values.iter().all(|v| *v > 0.0));
    }

    #[test]
    fn test_repeated_terms_higher_weight() {
        let sv = text_to_sparse_vector("sleep sleep sleep apnea");
        // "sleep" appears 3x → ln(1+3) ≈ 1.386
        // "apnea" appears 1x → ln(1+1) ≈ 0.693
        let max_val = sv.values.iter().cloned().fold(0.0f32, f32::max);
        assert!(max_val > 1.0);
    }

    #[test]
    fn test_empty_text() {
        let sv = text_to_sparse_vector("");
        assert!(sv.indices.is_empty());
    }

    #[test]
    fn test_thai_text() {
        // Thai spaces between phrases
        let sv = text_to_sparse_vector("การรักษา ผู้ป่วย ภาวะ หยุดหายใจ ขณะหลับ");
        assert!(!sv.indices.is_empty());
    }
}
