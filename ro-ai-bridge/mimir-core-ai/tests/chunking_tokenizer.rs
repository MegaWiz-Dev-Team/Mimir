//! Integration tests for the BGE-M3 tokenizer path in `services::chunking`.
//!
//! Sprint 48 C.2: replace the `chars / 4` heuristic with a real tokenizer
//! load when `BGE_M3_TOKENIZER_PATH` is set. The default Asgard install
//! caches the tokenizer at:
//!
//!   `Heimdall/gateway/.fastembed_cache/models--BAAI--bge-m3/snapshots/<sha>/tokenizer.json`
//!
//! These tests **skip with a clear log** when the env is not set (e.g. on
//! a fresh CI runner without the fastembed cache). When set, they exercise
//! the actual tokenizer path end-to-end so we know chunk-size math matches
//! what the embedder will see.
//!
//! Isolated from the lib-test target due to pre-existing compile breakage
//! in mcp_server / a2a / runner test fixtures.

use mimir_core_ai::services::chunking::{ChunkStrategy, chunk};

/// Resolve the BGE-M3 tokenizer path from env. Skip the test gracefully
/// if not set so CI without the model cache doesn't fail.
fn tokenizer_path() -> Option<String> {
    match std::env::var("BGE_M3_TOKENIZER_PATH") {
        Ok(p) if !p.is_empty() && std::path::Path::new(&p).exists() => Some(p),
        _ => {
            eprintln!(
                "[skip] BGE_M3_TOKENIZER_PATH unset or file missing — set to the bge-m3 \
                 tokenizer.json (e.g. Heimdall/gateway/.fastembed_cache/...) to exercise \
                 the real tokenizer path"
            );
            None
        }
    }
}

/// Smoke test: when env is set, a non-empty input must produce >0 tokens.
/// (The chunking pipeline calls `estimate_tokens` indirectly; we observe
/// the count via `ChunkResult::token_count`.)
#[test]
fn bge_m3_tokenizer_produces_nonzero_count_when_env_set() {
    let Some(_path) = tokenizer_path() else {
        return;
    };

    let text = "The quick brown fox jumps over the lazy dog.";
    let strategy = ChunkStrategy::default();
    let chunks = chunk(text, &strategy).expect("chunk should succeed");

    assert!(!chunks.is_empty(), "Expected at least one chunk");
    assert!(
        chunks[0].token_count > 0,
        "token_count must be > 0 for non-empty content"
    );
}

/// The real tokenizer must produce a DIFFERENT count from `chars / 4`
/// for at least some inputs — otherwise the heuristic was already
/// accurate and we got no value from the upgrade. Thai is the canonical
/// case where the heuristic over-counts.
#[test]
fn thai_tokens_differ_from_chars_div_four_when_env_set() {
    let Some(_path) = tokenizer_path() else {
        return;
    };

    // ~120 Thai chars. chars/4 = 30 tokens.
    // BGE-M3 tokenizer on Thai typically produces FEWER tokens because
    // common Thai sequences map to single BPE units. We just assert the
    // count differs — direction depends on tokenizer vocab.
    let thai = "ยาลดน้ำหนักเป็นยาที่ช่วยลดน้ำหนักได้อย่างมีประสิทธิภาพการออกกำลังกายอย่างสม่ำเสมอช่วยเสริมประสิทธิภาพของยา";
    let strategy = ChunkStrategy::Fixed {
        size: 500,
        overlap: 50,
    };
    let chunks = chunk(thai, &strategy).expect("chunk should succeed");
    assert!(!chunks.is_empty());

    let real_tokens = chunks[0].token_count;
    let heuristic_tokens = (thai.len() as f64 / 4.0).ceil() as usize;

    assert_ne!(
        real_tokens, heuristic_tokens,
        "Real BGE-M3 tokenizer should produce a different count than chars/4 for Thai \
         (real={}, heuristic={}). If equal, the env var may not be wired into the \
         test runner correctly.",
        real_tokens, heuristic_tokens
    );
}

/// Empty content must yield 0 tokens — both paths agree on this.
#[test]
fn empty_content_yields_zero_tokens() {
    // No skip needed — both paths agree on the empty case.
    let strategy = ChunkStrategy::default();
    let chunks = chunk("", &strategy).expect("chunk should succeed");
    assert!(chunks.is_empty(), "Empty input → no chunks");
}
