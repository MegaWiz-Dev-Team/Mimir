//! TDD tests for FHIR R5 `CodeableConcept` datatype.

use mimir_fhir::datatypes::{Code, CodeableConcept, Coding, Uri};

// --- Construction ---

#[test]
fn codeable_concept_from_single_coding() {
    let coding = Coding::new(
        Uri::new("http://loinc.org").unwrap(),
        Code::new("8480-6").unwrap(),
    );
    let cc = CodeableConcept::from_coding(coding.clone());

    assert_eq!(cc.coding.len(), 1);
    assert_eq!(cc.coding[0], coding);
    assert!(cc.text.is_none());
}

#[test]
fn codeable_concept_from_text_only() {
    let cc = CodeableConcept::from_text("Severe headache");
    assert!(cc.coding.is_empty());
    assert_eq!(cc.text.as_deref(), Some("Severe headache"));
}

#[test]
fn codeable_concept_with_text_adds_display() {
    let coding = Coding::new(
        Uri::new("http://snomed.info/sct").unwrap(),
        Code::new("25064002").unwrap(),
    );
    let cc = CodeableConcept::from_coding(coding).with_text("Headache");

    assert_eq!(cc.coding.len(), 1);
    assert_eq!(cc.text.as_deref(), Some("Headache"));
}

#[test]
fn codeable_concept_multiple_codings_represent_translations() {
    // Same concept ("headache") in two terminology systems
    let snomed = Coding::new(
        Uri::new("http://snomed.info/sct").unwrap(),
        Code::new("25064002").unwrap(),
    );
    let icd10 = Coding::new(
        Uri::new("http://hl7.org/fhir/sid/icd-10").unwrap(),
        Code::new("R51").unwrap(),
    );
    let cc = CodeableConcept {
        coding: vec![snomed, icd10],
        text: Some("Headache".into()),
    };

    assert_eq!(cc.coding.len(), 2);
}

// --- Serde ---

#[test]
fn codeable_concept_serializes_with_coding_array() {
    let cc = CodeableConcept::from_coding(Coding::new(
        Uri::new("http://loinc.org").unwrap(),
        Code::new("8480-6").unwrap(),
    ))
    .with_text("Systolic BP");

    let json = serde_json::to_value(&cc).unwrap();
    assert!(json["coding"].is_array());
    assert_eq!(json["coding"][0]["code"], "8480-6");
    assert_eq!(json["text"], "Systolic BP");
}

#[test]
fn codeable_concept_omits_empty_coding_array() {
    // Text-only concept must not emit empty "coding": []
    let cc = CodeableConcept::from_text("free text only");
    let json = serde_json::to_value(&cc).unwrap();
    assert!(
        json.get("coding").is_none(),
        "empty coding array should be omitted, not serialized as []"
    );
    assert_eq!(json["text"], "free text only");
}

#[test]
fn codeable_concept_deserializes_minimal() {
    let json = r#"{"text":"abdominal pain"}"#;
    let cc: CodeableConcept = serde_json::from_str(json).unwrap();
    assert!(cc.coding.is_empty());
    assert_eq!(cc.text.as_deref(), Some("abdominal pain"));
}

#[test]
fn codeable_concept_round_trips_dual_coded() {
    let original = CodeableConcept {
        coding: vec![
            Coding::new(
                Uri::new("http://snomed.info/sct").unwrap(),
                Code::new("25064002").unwrap(),
            ),
            Coding::new(
                Uri::new("http://hl7.org/fhir/sid/icd-10").unwrap(),
                Code::new("R51").unwrap(),
            ),
        ],
        text: Some("Headache".into()),
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: CodeableConcept = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
