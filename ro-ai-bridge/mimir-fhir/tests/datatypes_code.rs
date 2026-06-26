//! TDD tests for FHIR R5 `Code` primitive.
//!
//! FHIR R5 grammar: `[^\s]+(\s[^\s]+)*` — non-empty, no edge whitespace,
//! internal single spaces only.

use mimir_fhir::datatypes::{Code, CodeError};

// --- Happy path ---

#[test]
fn code_accepts_single_word() {
    assert!(Code::new("final").is_ok());
    assert!(Code::new("male").is_ok());
    assert!(Code::new("order").is_ok());
}

#[test]
fn code_accepts_internal_single_space() {
    assert!(Code::new("vital signs").is_ok());
    assert!(Code::new("a b c").is_ok());
}

#[test]
fn code_accepts_loinc_style() {
    // LOINC codes commonly include - and . (no spaces)
    assert!(Code::new("8480-6").is_ok());
    assert!(Code::new("29463-7").is_ok());
}

#[test]
fn code_accepts_snomed_style() {
    assert!(Code::new("89666000").is_ok()); // CPR procedure
}

// --- Empty / whitespace edges ---

#[test]
fn code_rejects_empty() {
    assert_eq!(Code::new(""), Err(CodeError::Empty));
}

#[test]
fn code_rejects_leading_whitespace() {
    assert_eq!(Code::new(" final"), Err(CodeError::EdgeWhitespace));
}

#[test]
fn code_rejects_trailing_whitespace() {
    assert_eq!(Code::new("final "), Err(CodeError::EdgeWhitespace));
}

// --- Forbidden whitespace types ---

#[test]
fn code_rejects_tab() {
    assert_eq!(
        Code::new("vital\tsigns"),
        Err(CodeError::InvalidWhitespace('\t'))
    );
}

#[test]
fn code_rejects_newline() {
    assert_eq!(
        Code::new("vital\nsigns"),
        Err(CodeError::InvalidWhitespace('\n'))
    );
}

#[test]
fn code_rejects_consecutive_spaces() {
    assert_eq!(
        Code::new("vital  signs"),
        Err(CodeError::ConsecutiveWhitespace)
    );
}

// --- Serde round-trip ---

#[test]
fn code_serializes_as_json_string() {
    let c = Code::new("final").unwrap();
    assert_eq!(serde_json::to_string(&c).unwrap(), "\"final\"");
}

#[test]
fn code_deserialize_rejects_invalid_grammar() {
    let result: Result<Code, _> = serde_json::from_str("\" final\"");
    assert!(result.is_err(), "code with leading space must not parse");
}

#[test]
fn code_round_trips_loinc() {
    let original = Code::new("8480-6").unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Code = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
