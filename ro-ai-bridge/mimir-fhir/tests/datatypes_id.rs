//! TDD tests for FHIR R5 `Id` primitive.
//!
//! Sprint 1 Day 1 — first datatype, demonstrates TDD red-green discipline
//! that the rest of Sprint 1 follows.

use mimir_fhir::datatypes::{Id, IdError};

// --- Construction: happy path ---

#[test]
fn id_accepts_simple_ascii() {
    assert!(Id::new("patient-001").is_ok());
    assert!(Id::new("encounter.v1").is_ok());
    assert!(Id::new("ABC123").is_ok());
    assert!(Id::new("a").is_ok()); // min length 1
}

#[test]
fn id_accepts_max_length_64() {
    let max = "x".repeat(64);
    assert!(Id::new(&max).is_ok());
}

#[test]
fn id_accepts_thai_safe_synthetic_ulid() {
    // ULID-like (used by mimir-well per ADR-011) — all ascii alphanumeric, fits 26 chars
    assert!(Id::new("01J5XQNK7VFW9RZGBQ2N3PMABC").is_ok());
}

// --- Construction: invalid length ---

#[test]
fn id_rejects_empty() {
    assert_eq!(Id::new(""), Err(IdError::InvalidLength(0)));
}

#[test]
fn id_rejects_too_long() {
    let too_long = "x".repeat(65);
    assert_eq!(Id::new(&too_long), Err(IdError::InvalidLength(65)));
}

// --- Construction: invalid characters ---

#[test]
fn id_rejects_slash() {
    // Slash is a common bug — confuses with FHIR URL paths like "Patient/123"
    assert_eq!(Id::new("patient/001"), Err(IdError::InvalidCharacter('/')));
}

#[test]
fn id_rejects_underscore() {
    // FHIR spec excludes underscore — caught at compile time prevents accidental
    // use of programming-language idioms in FHIR ids
    assert_eq!(Id::new("patient_001"), Err(IdError::InvalidCharacter('_')));
}

#[test]
fn id_rejects_whitespace() {
    assert_eq!(Id::new("patient 001"), Err(IdError::InvalidCharacter(' ')));
}

#[test]
fn id_rejects_thai_characters() {
    // Thai script not allowed in FHIR Id — must be ASCII grammar.
    // Thai names go in HumanName, not Id.
    assert_eq!(Id::new("ผู้ป่วย-001"), Err(IdError::InvalidCharacter('ผ')));
}

// --- Display / AsRef ---

#[test]
fn id_displays_as_inner_string() {
    let id = Id::new("patient-001").unwrap();
    assert_eq!(id.to_string(), "patient-001");
    assert_eq!(id.as_str(), "patient-001");
    assert_eq!(id.as_ref(), "patient-001");
}

// --- Serde round-trip ---

#[test]
fn id_serializes_as_json_string() {
    let id = Id::new("patient-001").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"patient-001\"");
}

#[test]
fn id_deserializes_from_json_string() {
    let id: Id = serde_json::from_str("\"patient-001\"").unwrap();
    assert_eq!(id.as_str(), "patient-001");
}

#[test]
fn id_deserialize_rejects_invalid_grammar() {
    // Non-conformant id from external system must NOT silently parse.
    let result: Result<Id, _> = serde_json::from_str("\"patient/001\"");
    assert!(
        result.is_err(),
        "id deserialize must reject FHIR-non-conformant input"
    );
}

#[test]
fn id_round_trips_through_json() {
    let original = Id::new("encounter.v1").unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Id = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
