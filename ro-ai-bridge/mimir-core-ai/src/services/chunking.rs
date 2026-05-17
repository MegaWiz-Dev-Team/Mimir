//! Chunking service: split extracted text into chunks for embedding.
//!
//! Pure functions — no I/O, no database. Each function takes `&str`
//! and returns `Vec<ChunkResult>`.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

// ─── Types ─────────────────────────────────────────────────────────────────────

/// Chunking strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkStrategy {
    /// Split by character count with overlap.
    Fixed { size: usize, overlap: usize },
    /// Split by Markdown structure: headings → paragraphs → sentences.
    Recursive { max_size: usize },
    /// Split by embedding similarity (deferred to Sprint 10).
    Semantic,
}

impl Default for ChunkStrategy {
    fn default() -> Self {
        ChunkStrategy::Fixed {
            size: 500,
            overlap: 50,
        }
    }
}

/// Result of chunking a single piece of text.
#[derive(Debug, Clone, Serialize)]
pub struct ChunkResult {
    pub chunk_index: usize,
    pub content: String,
    pub token_count: usize,
    pub metadata: Value,
}

// ─── Router ────────────────────────────────────────────────────────────────────

/// Split text into chunks using the specified strategy.
pub fn chunk(text: &str, strategy: &ChunkStrategy) -> Result<Vec<ChunkResult>> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    info!("Chunking {} chars with strategy {:?}", text.len(), strategy);

    match strategy {
        ChunkStrategy::Fixed { size, overlap } => chunk_fixed(text, *size, *overlap),
        ChunkStrategy::Recursive { max_size } => chunk_recursive(text, *max_size),
        ChunkStrategy::Semantic => {
            bail!("Semantic chunking not yet implemented — requires embeddings (Sprint 10)")
        }
    }
}

/// Detect document type and recommend optimal chunking strategy.
///
/// - Markdown with headings (`##`) → `Recursive`
/// - Plain text / short content → `Fixed`
pub fn auto_recommend(text: &str) -> ChunkStrategy {
    let heading_count = text
        .lines()
        .filter(|line| line.starts_with("## ") || line.starts_with("### "))
        .count();

    if heading_count >= 2 {
        info!(
            "Auto-recommend: Recursive (found {} headings)",
            heading_count
        );
        ChunkStrategy::Recursive { max_size: 500 }
    } else {
        info!(
            "Auto-recommend: Fixed (plain text, {} headings)",
            heading_count
        );
        ChunkStrategy::Fixed {
            size: 500,
            overlap: 50,
        }
    }
}

// ─── Fixed Chunker ─────────────────────────────────────────────────────────────

/// Split text into fixed-size chunks with overlap.
///
/// - `size`: target chunk size in characters
/// - `overlap`: number of characters to overlap between consecutive chunks
pub fn chunk_fixed(text: &str, size: usize, overlap: usize) -> Result<Vec<ChunkResult>> {
    if size == 0 {
        bail!("Chunk size must be > 0");
    }
    if overlap >= size {
        bail!("Overlap ({}) must be less than size ({})", overlap, size);
    }

    let text = text.trim();
    if text.is_empty() {
        return Ok(vec![]);
    }

    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let step = size - overlap;
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while start < total {
        let end = (start + size).min(total);
        let content: String = chars[start..end].iter().collect();
        let token_count = estimate_tokens(&content);

        chunks.push(ChunkResult {
            chunk_index: index,
            content,
            token_count,
            metadata: json!({
                "strategy": "fixed",
                "char_start": start,
                "char_end": end,
                "size": size,
                "overlap": overlap
            }),
        });

        start += step;
        index += 1;

        // Avoid tiny trailing chunks (< 20% of size)
        if start < total && (total - start) < size / 5 {
            // Extend last chunk to include remaining
            let remaining: String = chars[start..total].iter().collect();
            if let Some(last) = chunks.last_mut() {
                last.content.push_str(&remaining);
                last.token_count = estimate_tokens(&last.content);
                last.metadata = json!({
                    "strategy": "fixed",
                    "char_start": start - step,
                    "char_end": total,
                    "size": size,
                    "overlap": overlap,
                    "extended": true
                });
            }
            break;
        }
    }

    Ok(chunks)
}

// ─── Recursive Chunker ─────────────────────────────────────────────────────────

/// Split text by Markdown structure, respecting heading hierarchy.
///
/// Strategy: split by `## ` headings first, then by `\n\n` paragraphs,
/// then by sentences if still over `max_size`.
pub fn chunk_recursive(text: &str, max_size: usize) -> Result<Vec<ChunkResult>> {
    if max_size == 0 {
        bail!("Max size must be > 0");
    }

    let text = text.trim();
    if text.is_empty() {
        return Ok(vec![]);
    }

    // Step 1: Split by headings (## or ###)
    let sections = split_by_headings(text);

    let mut chunks = Vec::new();
    let mut index = 0;

    for section in sections {
        let section = section.trim();
        if section.is_empty() {
            continue;
        }

        if section.len() <= max_size {
            chunks.push(ChunkResult {
                chunk_index: index,
                content: section.to_string(),
                token_count: estimate_tokens(section),
                metadata: json!({ "strategy": "recursive", "split_level": "heading" }),
            });
            index += 1;
        } else {
            // Step 2: Split long sections by paragraphs
            let paragraphs = split_by_paragraphs(section);
            for para in paragraphs {
                let para = para.trim();
                if para.is_empty() {
                    continue;
                }

                if para.len() <= max_size {
                    chunks.push(ChunkResult {
                        chunk_index: index,
                        content: para.to_string(),
                        token_count: estimate_tokens(para),
                        metadata: json!({ "strategy": "recursive", "split_level": "paragraph" }),
                    });
                    index += 1;
                } else {
                    // Step 3: Split long paragraphs by sentences
                    let sentence_chunks = split_by_sentences(para, max_size);
                    for sc in sentence_chunks {
                        if sc.len() <= max_size {
                            chunks.push(ChunkResult {
                                chunk_index: index,
                                content: sc.clone(),
                                token_count: estimate_tokens(&sc),
                                metadata: json!({ "strategy": "recursive", "split_level": "sentence" }),
                            });
                            index += 1;
                        } else {
                            // Step 4: Fallback to fixed split for very long text without sentence breaks
                            let fixed = chunk_fixed(&sc, max_size, 0).unwrap_or_default();
                            for fc in fixed {
                                chunks.push(ChunkResult {
                                    chunk_index: index,
                                    content: fc.content,
                                    token_count: fc.token_count,
                                    metadata: json!({ "strategy": "recursive", "split_level": "fixed_fallback" }),
                                });
                                index += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(chunks)
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

/// Rough token count estimate (~4 chars per token for English).
fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}

/// Split text by Markdown headings (`## ` or `### `).
fn split_by_headings(text: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if (line.starts_with("## ") || line.starts_with("### ")) && !current.trim().is_empty() {
            sections.push(current.clone());
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        sections.push(current);
    }

    sections
}

/// Split text by double newlines (paragraph breaks).
fn split_by_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n").map(|s| s.to_string()).collect()
}

/// Detect if a character belongs to the Thai Unicode block (U+0E00..U+0E7F).
///
/// Used to decide when whitespace-after-Thai is treated as a soft sentence
/// boundary — Thai script has no explicit sentence terminator like the Latin
/// full stop, so we have to infer boundaries from context.
fn is_thai_char(c: char) -> bool {
    matches!(c as u32, 0x0E00..=0x0E7F)
}

/// Split long text by sentences, accumulating until max_size.
///
/// Recognizes:
/// - Latin punctuation: `.`, `!`, `?` (with trailing whitespace)
/// - Newline (`\n`) — strongest boundary in practice for both Thai and English
/// - Thai paiyannoi (`ฯ`) — abbreviation/end marker
///
/// Thai script has no explicit sentence-terminating punctuation, so we also
/// treat whitespace immediately after a Thai character as a soft boundary
/// (PyThaiNLP `sent_tokenize` does similar heuristics). This is a regex-based
/// approximation; for higher-quality Thai segmentation, a future patch should
/// wire an HTTP call to a PyThaiNLP sidecar via `THAI_NLP_ENDPOINT`. The
/// public function signature stays sync so callers don't have to refactor.
///
/// Sprint 48 B-48f context: this function is on the hot path for Thai semantic
/// search ingestion. The previous English-only splitter caused Thai paragraphs
/// to fall through to `fixed_fallback` (mid-word cuts), which would have
/// produced poor BGE-M3 embeddings for Qdrant Thai retrieval.
fn split_by_sentences(text: &str, max_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    // Walk the string character by character so we can detect Thai-context
    // boundaries (whitespace after a Thai char) in addition to the hard
    // delimiters. This is roughly O(n) like split_inclusive but with context.
    let chars: Vec<char> = text.chars().collect();
    let mut prev: Option<char> = None;
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        current.push(c);

        let is_hard_delim = c == '.' || c == '!' || c == '?' || c == '\n' || c == 'ฯ';
        let is_soft_thai_boundary = c.is_whitespace()
            && c != '\n' // newline already handled as hard delim
            && prev.map(is_thai_char).unwrap_or(false);

        if is_hard_delim || is_soft_thai_boundary {
            // Boundary reached. Flush current if accumulated >= max_size.
            // Otherwise keep accumulating so chunks don't get too tiny.
            if current.len() >= max_size && !current.trim().is_empty() {
                chunks.push(current.trim().to_string());
                current.clear();
            }
        }

        prev = Some(c);
        i += 1;
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

// ═══════════════════════════════════════════════════════════════════════════════
// TDD Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Fixed Chunker Tests ───────────────────────────────────────────────────

    #[test]
    fn test_chunk_fixed_basic() {
        let text = "a".repeat(1000);
        let result = chunk_fixed(&text, 300, 50).unwrap();

        assert!(
            result.len() >= 3,
            "Should produce at least 3 chunks, got {}",
            result.len()
        );
        assert_eq!(result[0].chunk_index, 0);
        assert_eq!(result[0].content.len(), 300);
        assert!(result[0].token_count > 0);
    }

    #[test]
    fn test_chunk_fixed_overlap() {
        let text: String = (0..100).map(|i| format!("{:03}", i)).collect(); // "000001002..." = 300 chars
        let result = chunk_fixed(&text, 100, 20).unwrap();

        assert!(result.len() >= 3, "Should have at least 3 chunks");

        // Check overlap: last 20 chars of chunk 0 == first 20 chars of chunk 1
        let end_of_first = &result[0].content[80..100];
        let start_of_second = &result[1].content[0..20];
        assert_eq!(end_of_first, start_of_second, "Overlap should match");
    }

    #[test]
    fn test_chunk_fixed_small_text() {
        let text = "Hello world";
        let result = chunk_fixed(text, 500, 50).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Hello world");
    }

    #[test]
    fn test_chunk_fixed_invalid_params() {
        assert!(chunk_fixed("text", 0, 0).is_err(), "Size 0 should error");
        assert!(
            chunk_fixed("text", 10, 10).is_err(),
            "Overlap >= size should error"
        );
        assert!(
            chunk_fixed("text", 10, 15).is_err(),
            "Overlap > size should error"
        );
    }

    #[test]
    fn test_chunk_fixed_empty() {
        let result = chunk_fixed("", 100, 10).unwrap();
        assert!(result.is_empty());

        let result2 = chunk_fixed("   ", 100, 10).unwrap();
        assert!(result2.is_empty());
    }

    // ─── Recursive Chunker Tests ───────────────────────────────────────────────

    #[test]
    fn test_chunk_recursive_headings() {
        let text = "\
## Introduction
This is the intro paragraph.

## Methods
This describes the methods used.

## Results
Here are the results.
";
        let result = chunk_recursive(text, 500).unwrap();
        assert_eq!(result.len(), 3, "Should split into 3 heading sections");
        assert!(result[0].content.contains("Introduction"));
        assert!(result[1].content.contains("Methods"));
        assert!(result[2].content.contains("Results"));
    }

    #[test]
    fn test_chunk_recursive_long_paragraph() {
        let long_para = "a".repeat(600);
        let text = format!("## Section\n{}", long_para);
        let result = chunk_recursive(&text, 300).unwrap();
        assert!(result.len() >= 2, "Long paragraph should be split further");
    }

    #[test]
    fn test_chunk_recursive_preserves_structure() {
        let text = "\
## Chapter 1
Short paragraph here.

## Chapter 2
Another short paragraph.
";
        let result = chunk_recursive(text, 1000).unwrap();
        // Each chapter fits within max_size, so should stay intact
        assert_eq!(result.len(), 2);
        assert!(result[0].content.contains("Chapter 1"));
        assert!(result[1].content.contains("Chapter 2"));
    }

    // ─── Auto-recommend Tests ──────────────────────────────────────────────────

    #[test]
    fn test_auto_recommend_markdown() {
        let text = "## Heading 1\nContent\n## Heading 2\nMore content\n### Sub-heading\nDetails";
        let strategy = auto_recommend(text);
        assert!(matches!(strategy, ChunkStrategy::Recursive { .. }));
    }

    #[test]
    fn test_auto_recommend_plain() {
        let text = "This is just a plain text document without any headings or structure.";
        let strategy = auto_recommend(text);
        assert!(matches!(strategy, ChunkStrategy::Fixed { .. }));
    }

    // ─── Router Tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_chunk_empty_text() {
        let result = chunk("", &ChunkStrategy::default()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_chunk_semantic_not_implemented() {
        let result = chunk("some text", &ChunkStrategy::Semantic);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }

    #[test]
    fn test_chunk_router_fixed() {
        let text = "Hello world. This is a test.";
        let strategy = ChunkStrategy::Fixed {
            size: 500,
            overlap: 50,
        };
        let result = chunk(text, &strategy).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].content.contains("Hello world"));
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("abcd"), 1); // 4 chars = 1 token
        assert_eq!(estimate_tokens("abcdefgh"), 2); // 8 chars = 2 tokens
        assert_eq!(estimate_tokens(""), 0);
    }

    // ─── Thai Sentence Splitter Tests (Sprint 48 chunking remediation C.1) ────
    //
    // Audit found that the old splitter only recognized `.` / `!` / `?`,
    // which never fire in pure Thai. Result: Thai paragraphs fell through
    // to `fixed_fallback`, cutting mid-word and producing bad embeddings.
    // These tests pin the new Thai-aware behavior.

    #[test]
    fn test_is_thai_char() {
        // Common Thai consonants / vowels in the U+0E00..U+0E7F block.
        assert!(is_thai_char('ก'));
        assert!(is_thai_char('า'));
        assert!(is_thai_char('ฯ'));
        // Non-Thai must not match.
        assert!(!is_thai_char('a'));
        assert!(!is_thai_char('0'));
        assert!(!is_thai_char(' '));
        assert!(!is_thai_char('。')); // CJK full stop, not Thai
    }

    #[test]
    fn test_split_by_sentences_thai_paragraph_chunks() {
        // Long Thai paragraph (~360 chars). The old splitter never broke
        // this up because Thai has no `.`/`!`/`?`. Now the soft
        // whitespace-after-Thai boundary plus newlines should keep chunks
        // bounded near `max_size`.
        let thai = "ยาลดน้ำหนักเป็นยาที่ช่วยลดน้ำหนักได้อย่างมีประสิทธิภาพ \
                    การออกกำลังกายอย่างสม่ำเสมอช่วยเสริมประสิทธิภาพของยา \
                    ผู้ป่วยควรปรึกษาแพทย์ก่อนใช้ยาทุกครั้ง \
                    ผลข้างเคียงที่พบบ่อยคือคลื่นไส้และปวดศีรษะ \
                    หากมีอาการรุนแรงควรพบแพทย์ทันที";
        let chunks = split_by_sentences(thai, 100);
        // With max_size=100 the long string MUST produce >1 chunk —
        // otherwise the splitter degraded back to single-chunk behavior.
        assert!(
            chunks.len() >= 2,
            "Thai paragraph must split into multiple chunks (got {}: {:?})",
            chunks.len(),
            chunks
        );
        // No chunk may be empty after trim.
        for c in &chunks {
            assert!(!c.trim().is_empty());
        }
    }

    #[test]
    fn test_split_by_sentences_thai_paiyannoi_boundary() {
        // Thai paiyannoi (ฯ) is the closest analogue to a sentence-end mark
        // in Thai script. The splitter must treat it as a hard delimiter.
        let text = "องค์การสหประชาชาติฯ ดำเนินงานหลายด้าน ตัวอย่างเช่นด้านสาธารณสุข \
                    สถาบันวิจัยฯ ตีพิมพ์ผลงานทุกปี";
        let chunks = split_by_sentences(text, 60);
        assert!(
            chunks.len() >= 2,
            "ฯ should produce sentence breaks (got {}: {:?})",
            chunks.len(),
            chunks
        );
    }

    #[test]
    fn test_split_by_sentences_mixed_thai_english() {
        // Mixed-script text used by Thai medical writers. Should split
        // at BOTH Latin `.` AND Thai-space boundaries.
        let text = "Metformin คือยาลดน้ำตาลในเลือด. \
                    ใช้รักษาเบาหวานชนิดที่ 2. \
                    ขนาดยาเริ่มต้น 500 mg วันละครั้ง. \
                    Adjust dose based on eGFR.";
        let chunks = split_by_sentences(text, 50);
        assert!(
            chunks.len() >= 3,
            "Mixed Thai/English must split at periods + Thai boundaries (got {}: {:?})",
            chunks.len(),
            chunks
        );
    }

    #[test]
    fn test_split_by_sentences_english_still_works() {
        // Regression guard: the original English behavior must not break.
        let text =
            "The quick brown fox jumps over the lazy dog. \
             It was a dark and stormy night. \
             To be or not to be that is the question. \
             All animals are equal but some are more equal than others.";
        let chunks = split_by_sentences(text, 60);
        assert!(
            chunks.len() >= 3,
            "English sentence split must still work (got {}: {:?})",
            chunks.len(),
            chunks
        );
    }

    #[test]
    fn test_chunk_recursive_thai_long_paragraph_uses_sentence_split() {
        // End-to-end: a Thai paragraph >max_size routed through
        // chunk_recursive used to hit `fixed_fallback` (mid-character cuts);
        // now it should reach `sentence` level instead.
        let long_thai = "นโยบายการรักษาผู้ป่วยเบาหวานชนิดที่ 2 ในประเทศไทย \
                         เน้นการควบคุมระดับน้ำตาลในเลือดร่วมกับการปรับเปลี่ยน \
                         พฤติกรรมการกินอาหารและการออกกำลังกาย \
                         ยาที่ใช้ในขั้นต้นมักเป็น Metformin \
                         เนื่องจากมีหลักฐานทางคลินิกสนับสนุนมากที่สุด \
                         หากควบคุมระดับน้ำตาลได้ไม่ดีแพทย์อาจพิจารณา \
                         เพิ่มยากลุ่ม SGLT2 inhibitor หรือ GLP-1 agonist";
        let text = format!("## นโยบาย\n{}", long_thai);
        let result = chunk_recursive(&text, 120).unwrap();

        assert!(
            result.len() >= 2,
            "Long Thai paragraph must split (got {}: lens {:?})",
            result.len(),
            result.iter().map(|c| c.content.len()).collect::<Vec<_>>()
        );
        // Ideally at least one chunk reaches the `sentence` level rather
        // than degrading to `fixed_fallback`. (Some chunks may still be
        // `fixed_fallback` if no boundary lands near the size budget, but
        // sentence-level splits should appear for at least one chunk.)
        let split_levels: Vec<String> = result
            .iter()
            .filter_map(|c| {
                c.metadata
                    .get("split_level")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        assert!(
            split_levels.iter().any(|l| l == "sentence"),
            "At least one chunk should split at sentence level, got: {:?}",
            split_levels
        );
    }
}
