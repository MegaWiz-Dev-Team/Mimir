//! TDD tests for FHIR R5 `Markdown` primitive.

use mimir_fhir::datatypes::{Markdown, MarkdownError};

// --- Happy path ---

#[test]
fn markdown_accepts_plain_text() {
    assert!(Markdown::new("Patient reported chest pain.").is_ok());
}

#[test]
fn markdown_accepts_markdown_syntax() {
    assert!(Markdown::new("**bold** and _italic_ and [link](http://example.com)").is_ok());
}

#[test]
fn markdown_accepts_multiline() {
    let multi = "Line 1\n\nLine 2\n\n- bullet\n- another";
    assert!(Markdown::new(multi).is_ok());
}

#[test]
fn markdown_accepts_thai_clinical_note() {
    // Thai script + markdown markers must work for Condition.note in Thai medical contexts
    assert!(Markdown::new("**ผู้ป่วย** ปฏิบัติตามคำแนะนำของแพทย์").is_ok());
}

// --- Rejects ---

#[test]
fn markdown_rejects_empty() {
    assert_eq!(Markdown::new(""), Err(MarkdownError::EmptyOrWhitespaceOnly));
}

#[test]
fn markdown_rejects_whitespace_only() {
    assert_eq!(
        Markdown::new("   "),
        Err(MarkdownError::EmptyOrWhitespaceOnly)
    );
    assert_eq!(
        Markdown::new("\n\t\n"),
        Err(MarkdownError::EmptyOrWhitespaceOnly)
    );
}

// --- Serde round-trip ---

#[test]
fn markdown_serializes_as_json_string() {
    let m = Markdown::new("**hello**").unwrap();
    assert_eq!(serde_json::to_string(&m).unwrap(), "\"**hello**\"");
}

#[test]
fn markdown_round_trips_through_json() {
    let original = Markdown::new("Multi-line\n\nclinical note").unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Markdown = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn markdown_deserialize_rejects_whitespace_only() {
    let result: Result<Markdown, _> = serde_json::from_str("\"   \"");
    assert!(result.is_err());
}
