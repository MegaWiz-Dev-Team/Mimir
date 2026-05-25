//! TDD tests for FHIR R5 `Identifier` datatype.

use mimir_fhir::datatypes::{Code, CodeableConcept, Coding, Identifier, IdentifierUse, Uri};

// --- Construction ---

#[test]
fn identifier_new_sets_system_and_value() {
    let id = Identifier::new(
        Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
        "1234567890123",
    );
    assert_eq!(
        id.system.as_ref().unwrap().as_str(),
        "https://fhir.moph.go.th/identifier/citizen-id"
    );
    assert_eq!(id.value.as_deref(), Some("1234567890123"));
    assert!(id.use_.is_none());
}

#[test]
fn identifier_official_marker() {
    let id = Identifier::new(
        Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
        "1234567890123",
    )
    .official();
    assert_eq!(id.use_, Some(IdentifierUse::Official));
}

#[test]
fn identifier_with_type_attaches_codeable_concept() {
    let mr_type = CodeableConcept::from_coding(
        Coding::new(
            Uri::new("http://terminology.hl7.org/CodeSystem/v2-0203").unwrap(),
            Code::new("MR").unwrap(),
        )
        .with_display("Medical record number"),
    );

    let id = Identifier::new(
        Uri::new("urn:oid:1.2.36.146.595.217.0.1").unwrap(),
        "PT-001234",
    )
    .official()
    .with_type(mr_type.clone());

    assert_eq!(id.type_, Some(mr_type));
}

// --- Thai citizen ID convention (MOPH-PC1 Patient.identifier slice) ---

#[test]
fn identifier_thai_citizen_id_shape() {
    // Per MOPH-PC1 mapping doc — Patient.identifier MUST include
    // Thai citizen ID in this shape for asgard_medical tenant.
    let citizen = Identifier::new(
        Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
        "1101700123456",
    )
    .official();

    let json = serde_json::to_value(&citizen).unwrap();
    assert_eq!(json["use"], "official");
    assert_eq!(
        json["system"],
        "https://fhir.moph.go.th/identifier/citizen-id"
    );
    assert_eq!(json["value"], "1101700123456");
}

// --- Serde ---

#[test]
fn identifier_use_serializes_lowercase() {
    let cases = [
        (IdentifierUse::Usual, "usual"),
        (IdentifierUse::Official, "official"),
        (IdentifierUse::Temp, "temp"),
        (IdentifierUse::Secondary, "secondary"),
        (IdentifierUse::Old, "old"),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }
}

#[test]
fn identifier_use_renamed_to_use_on_wire() {
    let id = Identifier::new(Uri::new("http://example.com").unwrap(), "abc").official();
    let json = serde_json::to_value(&id).unwrap();
    // Rust field is `use_`; FHIR wire format is `use`
    assert_eq!(json["use"], "official");
    assert!(json.get("use_").is_none());
}

#[test]
fn identifier_type_renamed_to_type_on_wire() {
    let mr = CodeableConcept::from_text("MRN");
    let id = Identifier::new(Uri::new("http://example.com").unwrap(), "abc").with_type(mr);
    let json = serde_json::to_value(&id).unwrap();
    assert!(json.get("type").is_some());
    assert!(json.get("type_").is_none());
}

#[test]
fn identifier_omits_none_fields() {
    let id = Identifier::new(Uri::new("http://example.com").unwrap(), "abc");
    let json = serde_json::to_value(&id).unwrap();
    // Only system + value should be present
    assert!(json.get("use").is_none());
    assert!(json.get("type").is_none());
    assert_eq!(json["system"], "http://example.com");
    assert_eq!(json["value"], "abc");
}

#[test]
fn identifier_round_trips_official_with_type() {
    let original = Identifier::new(
        Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
        "1101700123456",
    )
    .official()
    .with_type(CodeableConcept::from_coding(
        Coding::new(
            Uri::new("http://terminology.hl7.org/CodeSystem/v2-0203").unwrap(),
            Code::new("NI").unwrap(),
        )
        .with_display("National identifier"),
    ));

    let json = serde_json::to_string(&original).unwrap();
    let restored: Identifier = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn identifier_deserialize_validates_inner_uri() {
    // system with whitespace must fail
    let json = r#"{"system":"http://has space.com","value":"abc"}"#;
    let result: Result<Identifier, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Uri validation must propagate");
}
