//! TDD tests for FHIR R5 `Coding` datatype.

use mimir_fhir::datatypes::{Code, Coding, Uri};

// --- Construction ---

#[test]
fn coding_new_sets_system_and_code() {
    let loinc = Uri::new("http://loinc.org").unwrap();
    let bp_code = Code::new("8480-6").unwrap();
    let coding = Coding::new(loinc.clone(), bp_code.clone());

    assert_eq!(coding.system, Some(loinc));
    assert_eq!(coding.code, Some(bp_code));
    assert_eq!(coding.display, None);
}

#[test]
fn coding_with_display_attaches_display() {
    let coding = Coding::new(
        Uri::new("http://loinc.org").unwrap(),
        Code::new("8480-6").unwrap(),
    )
    .with_display("Systolic blood pressure");

    assert_eq!(coding.display.as_deref(), Some("Systolic blood pressure"));
}

#[test]
fn coding_default_is_all_none() {
    let coding = Coding::default();
    assert!(coding.system.is_none());
    assert!(coding.code.is_none());
    assert!(coding.display.is_none());
}

// --- Serde — JSON emit ---

#[test]
fn coding_serializes_loinc_vital_sign() {
    let coding = Coding::new(
        Uri::new("http://loinc.org").unwrap(),
        Code::new("8480-6").unwrap(),
    )
    .with_display("Systolic blood pressure");

    let json = serde_json::to_value(&coding).unwrap();
    assert_eq!(json["system"], "http://loinc.org");
    assert_eq!(json["code"], "8480-6");
    assert_eq!(json["display"], "Systolic blood pressure");
    // None fields must be omitted, not emitted as null
    assert!(json.get("version").is_none());
    assert!(json.get("userSelected").is_none());
}

#[test]
fn coding_user_selected_serializes_as_camel_case() {
    let mut coding = Coding::new(
        Uri::new("http://snomed.info/sct").unwrap(),
        Code::new("89666000").unwrap(),
    );
    coding.user_selected = Some(true);

    let json = serde_json::to_value(&coding).unwrap();
    // FHIR JSON uses camelCase, not snake_case
    assert_eq!(json["userSelected"], true);
    assert!(json.get("user_selected").is_none());
}

// --- Serde — JSON parse ---

#[test]
fn coding_deserializes_minimal_payload() {
    let json = r#"{"system":"http://loinc.org","code":"8480-6"}"#;
    let coding: Coding = serde_json::from_str(json).unwrap();
    assert_eq!(coding.system.unwrap().as_str(), "http://loinc.org");
    assert_eq!(coding.code.unwrap().as_str(), "8480-6");
    assert!(coding.display.is_none());
}

#[test]
fn coding_deserialize_validates_inner_code() {
    // Code with leading whitespace is FHIR-non-conformant — must fail at parse
    let json = r#"{"system":"http://loinc.org","code":" 8480-6"}"#;
    let result: Result<Coding, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "inner Code validation must propagate to Coding parse"
    );
}

#[test]
fn coding_round_trips_through_json() {
    let original = Coding::new(
        Uri::new("http://loinc.org").unwrap(),
        Code::new("29463-7").unwrap(),
    )
    .with_display("Body weight");

    let json = serde_json::to_string(&original).unwrap();
    let restored: Coding = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
