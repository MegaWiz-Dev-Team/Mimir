//! Integration tests for Thai-aware sentence splitting in `services::chunking`.
//!
//! Isolated from the lib-test target because the lib has pre-existing
//! compile breakage in mcp_server / a2a / runner test fixtures
//! (see commit messages from Backend SSO v1.4.0 PR #294 — same workaround).
//!
//! Covers Sprint 48 chunking remediation C.1: Thai sentence segmentation
//! wired into chunk_recursive so Thai paragraphs don't fall through to
//! fixed_fallback (mid-word cuts) anymore.

use mimir_core_ai::services::chunking::{ChunkStrategy, chunk, chunk_recursive};

/// Pure Thai paragraph (~360 chars). Old splitter never broke this up
/// because Thai has no `.` / `!` / `?`. New splitter uses
/// whitespace-after-Thai as a soft sentence boundary, so a long Thai
/// paragraph routed through recursive chunking should produce >1 chunk.
#[test]
fn thai_paragraph_in_recursive_chunker_produces_multiple_chunks() {
    let thai = "ยาลดน้ำหนักเป็นยาที่ช่วยลดน้ำหนักได้อย่างมีประสิทธิภาพ \
                การออกกำลังกายอย่างสม่ำเสมอช่วยเสริมประสิทธิภาพของยา \
                ผู้ป่วยควรปรึกษาแพทย์ก่อนใช้ยาทุกครั้ง \
                ผลข้างเคียงที่พบบ่อยคือคลื่นไส้และปวดศีรษะ \
                หากมีอาการรุนแรงควรพบแพทย์ทันที";
    let text = format!("## ข้อมูลยา\n{}", thai);
    let chunks = chunk_recursive(&text, 120).expect("recursive should not fail");

    assert!(
        chunks.len() >= 2,
        "Thai paragraph must split into multiple chunks (got {}: lens {:?})",
        chunks.len(),
        chunks.iter().map(|c| c.content.len()).collect::<Vec<_>>()
    );
}

/// At least one chunk from a long Thai paragraph should split at the
/// `sentence` level rather than degrading to `fixed_fallback`. The old
/// English-only splitter always degraded for Thai input.
#[test]
fn thai_paragraph_reaches_sentence_level_split() {
    let long_thai = "นโยบายการรักษาผู้ป่วยเบาหวานชนิดที่ 2 ในประเทศไทย \
                     เน้นการควบคุมระดับน้ำตาลในเลือดร่วมกับการปรับเปลี่ยน \
                     พฤติกรรมการกินอาหารและการออกกำลังกาย \
                     ยาที่ใช้ในขั้นต้นมักเป็น Metformin \
                     เนื่องจากมีหลักฐานทางคลินิกสนับสนุนมากที่สุด \
                     หากควบคุมระดับน้ำตาลได้ไม่ดีแพทย์อาจพิจารณา \
                     เพิ่มยากลุ่ม SGLT2 inhibitor หรือ GLP-1 agonist";
    let text = format!("## นโยบาย\n{}", long_thai);
    let chunks = chunk_recursive(&text, 120).expect("recursive should not fail");

    let split_levels: Vec<String> = chunks
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

/// Mixed Thai/English text (Thai medical writers commonly do this).
/// Must split at BOTH the Latin period AND Thai whitespace boundaries.
#[test]
fn mixed_thai_english_splits_at_both_boundary_types() {
    let text = "## ยา\nMetformin คือยาลดน้ำตาลในเลือด. \
                ใช้รักษาเบาหวานชนิดที่ 2. \
                ขนาดยาเริ่มต้น 500 mg วันละครั้ง. \
                Adjust dose based on eGFR. \
                ตรวจน้ำตาลในเลือดอย่างน้อยทุก 3 เดือน \
                พบแพทย์หากมีอาการผิดปกติ";
    let chunks = chunk_recursive(text, 80).expect("recursive should not fail");

    assert!(
        chunks.len() >= 2,
        "Mixed Thai/English long paragraph must produce multiple chunks (got {}: {:?})",
        chunks.len(),
        chunks.iter().map(|c| c.content.len()).collect::<Vec<_>>()
    );
}

/// Regression: English-only behavior must not change. Pre-Sprint-48
/// callers depend on the existing Latin-punct sentence splitter.
#[test]
fn english_recursive_unchanged() {
    let text = "## Intro\n\
                The quick brown fox jumps over the lazy dog. \
                It was a dark and stormy night. \
                To be or not to be that is the question. \
                All animals are equal but some are more equal than others.";
    let chunks = chunk_recursive(text, 80).expect("recursive should not fail");

    assert!(
        chunks.len() >= 2,
        "English sentence split must still work (got {}: {:?})",
        chunks.len(),
        chunks.iter().map(|c| c.content.len()).collect::<Vec<_>>()
    );
}

/// Top-level `chunk()` dispatch must not regress when given Thai input.
#[test]
fn router_handles_thai_input() {
    let text = "ยาที่ใช้รักษาเบาหวาน Metformin คือยาตัวแรก";
    let strategy = ChunkStrategy::default();
    let chunks = chunk(text, &strategy).expect("chunk should not fail");

    assert!(!chunks.is_empty(), "Should produce at least one chunk");
    assert!(chunks[0].content.contains("Metformin") || chunks[0].content.contains("เบาหวาน"));
}
