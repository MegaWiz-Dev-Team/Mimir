//! Cross-source deduplication service.
//!
//! Detects duplicate content across different data sources using
//! content fingerprinting (SHA-256 for exact, SimHash for fuzzy).

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// ─── Types ─────────────────────────────────────────────────────────────────────

/// Report of dedup results for a sync operation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DedupReport {
    pub total_chunks: usize,
    pub unique_chunks: usize,
    pub duplicate_chunks: usize,
    pub duplicate_sources: Vec<DuplicateSource>,
}

/// Info about a duplicate source reference.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicateSource {
    pub chunk_index: usize,
    pub content_hash: String,
    pub existing_source_id: i64,
}

// ─── Text Normalization ────────────────────────────────────────────────────────

/// Normalize text for consistent fingerprinting.
///
/// - Lowercase
/// - Collapse all whitespace to single space
/// - Trim leading/trailing whitespace
/// - Strip common punctuation that doesn't change meaning
pub fn normalize_text(text: &str) -> String {
    let lowered = text.to_lowercase();
    // Collapse whitespace
    let collapsed: String = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim().to_string()
}

// ─── Fingerprinting ────────────────────────────────────────────────────────────

/// Compute SHA-256 fingerprint of normalized text. Used for exact match dedup.
pub fn fingerprint(text: &str) -> String {
    let normalized = normalize_text(text);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Compute 64-bit SimHash for fuzzy matching.
///
/// Uses word-level shingles (bigrams) and hashes each,
/// then combines into a 64-bit fingerprint.
pub fn simhash(text: &str) -> u64 {
    let normalized = normalize_text(text);
    let words: Vec<&str> = normalized.split_whitespace().collect();

    if words.is_empty() {
        return 0;
    }

    // Use bigrams as features
    let mut weights = [0i32; 64];

    let shingles: Vec<String> = if words.len() == 1 {
        vec![words[0].to_string()]
    } else {
        words
            .windows(2)
            .map(|w| format!("{} {}", w[0], w[1]))
            .collect()
    };

    for shingle in &shingles {
        let hash = hash_string(shingle);
        for i in 0..64 {
            if (hash >> i) & 1 == 1 {
                weights[i] += 1;
            } else {
                weights[i] -= 1;
            }
        }
    }

    let mut result: u64 = 0;
    for i in 0..64 {
        if weights[i] > 0 {
            result |= 1u64 << i;
        }
    }
    result
}

/// Hamming distance between two SimHash values.
/// Lower = more similar. 0 = identical.
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Check if two SimHash values are similar within a threshold.
/// Default threshold: 3 bits (out of 64) = ~95% similar.
pub fn is_similar(a: u64, b: u64, threshold: u32) -> bool {
    hamming_distance(a, b) <= threshold
}

// ─── Dedup Report Builder ──────────────────────────────────────────────────────

/// Track dedup state during chunk processing.
#[derive(Debug, Default)]
pub struct DedupTracker {
    pub report: DedupReport,
    seen_hashes: HashMap<String, i64>, // hash → source_id
}

impl DedupTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a unique chunk.
    pub fn record_unique(&mut self) {
        self.report.total_chunks += 1;
        self.report.unique_chunks += 1;
    }

    /// Record a duplicate chunk.
    pub fn record_duplicate(
        &mut self,
        chunk_index: usize,
        content_hash: &str,
        existing_source_id: i64,
    ) {
        self.report.total_chunks += 1;
        self.report.duplicate_chunks += 1;
        self.report.duplicate_sources.push(DuplicateSource {
            chunk_index,
            content_hash: content_hash.to_string(),
            existing_source_id,
        });
    }

    /// Track a hash locally within this run.
    pub fn track_hash(&mut self, hash: &str, source_id: i64) {
        self.seen_hashes.insert(hash.to_string(), source_id);
    }

    /// Check if a hash was already seen in this run.
    pub fn is_seen(&self, hash: &str) -> Option<i64> {
        self.seen_hashes.get(hash).copied()
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

/// Simple hash function for SimHash feature hashing.
fn hash_string(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV-1a prime
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_text() {
        assert_eq!(normalize_text("  Hello   World  "), "hello world");
        assert_eq!(normalize_text("UPPERCASE"), "uppercase");
        assert_eq!(
            normalize_text("tabs\there\nand\nnewlines"),
            "tabs here and newlines"
        );
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn test_fingerprint_consistent() {
        let hash1 = fingerprint("Hello World");
        let hash2 = fingerprint("Hello World");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_fingerprint_case_insensitive() {
        let hash1 = fingerprint("Hello World");
        let hash2 = fingerprint("hello world");
        assert_eq!(hash1, hash2, "Normalization should make these match");
    }

    #[test]
    fn test_fingerprint_whitespace_insensitive() {
        let hash1 = fingerprint("Hello   World");
        let hash2 = fingerprint("Hello World");
        assert_eq!(hash1, hash2, "Collapsed whitespace should match");
    }

    #[test]
    fn test_fingerprint_different() {
        let hash1 = fingerprint("Hello World");
        let hash2 = fingerprint("Goodbye World");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_simhash_similar() {
        let h1 = simhash("The quick brown fox jumps over the lazy dog");
        let h2 = simhash("The quick brown fox jumped over the lazy dog"); // changed "jumps" → "jumped"
        let dist = hamming_distance(h1, h2);
        assert!(
            dist <= 10,
            "Similar texts should have low hamming distance, got {}",
            dist
        );
    }

    #[test]
    fn test_simhash_different() {
        let h1 = simhash("The quick brown fox jumps over the lazy dog");
        let h2 = simhash("A completely different sentence about something else entirely");
        let dist = hamming_distance(h1, h2);
        assert!(
            dist > 5,
            "Different texts should have high hamming distance, got {}",
            dist
        );
    }

    #[test]
    fn test_simhash_identical() {
        let h1 = simhash("Exact same text here");
        let h2 = simhash("Exact same text here");
        assert_eq!(hamming_distance(h1, h2), 0);
    }

    #[test]
    fn test_is_similar() {
        let h1 = simhash("The quick brown fox");
        let h2 = simhash("The quick brown fox");
        assert!(is_similar(h1, h2, 3));
    }

    #[test]
    fn test_dedup_tracker() {
        let mut tracker = DedupTracker::new();
        tracker.record_unique();
        tracker.record_unique();
        tracker.record_duplicate(2, "abc123", 42);

        assert_eq!(tracker.report.total_chunks, 3);
        assert_eq!(tracker.report.unique_chunks, 2);
        assert_eq!(tracker.report.duplicate_chunks, 1);
        assert_eq!(tracker.report.duplicate_sources.len(), 1);
        assert_eq!(tracker.report.duplicate_sources[0].existing_source_id, 42);
    }

    #[test]
    fn test_dedup_tracker_seen_hashes() {
        let mut tracker = DedupTracker::new();
        tracker.track_hash("hash1", 10);
        tracker.track_hash("hash2", 20);

        assert_eq!(tracker.is_seen("hash1"), Some(10));
        assert_eq!(tracker.is_seen("hash2"), Some(20));
        assert_eq!(tracker.is_seen("hash3"), None);
    }

    #[test]
    fn test_simhash_empty() {
        assert_eq!(simhash(""), 0);
        assert_eq!(simhash("   "), 0);
    }
}
