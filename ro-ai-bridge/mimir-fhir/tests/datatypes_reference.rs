//! TDD tests for FHIR R5 `Reference` datatype.

use mimir_fhir::datatypes::{Code, CodeableConcept, Coding, Identifier, Reference, Uri};

// --- Literal references ---

#[test]
fn reference_literal_relative() {
    let r = Reference::literal("Patient/A12345");
    assert_eq!(r.reference.as_deref(), Some("Patient/A12345"));
    assert!(r.identifier.is_none());
    assert!(r.display.is_none());
}

#[test]
fn reference_literal_absolute_url() {
    let r = Reference::literal("https://other.example.com/fhir/r5/Patient/X");
    assert_eq!(
        r.reference.as_deref(),
        Some("https://other.example.com/fhir/r5/Patient/X")
    );
}

#[test]
fn reference_literal_contained() {
    let r = Reference::literal("#contained-org-1");
    assert_eq!(r.reference.as_deref(), Some("#contained-org-1"));
}

#[test]
fn reference_with_display_adds_display() {
    let r = Reference::literal("Patient/A12345").with_display("Mr. Somchai");
    assert_eq!(r.display.as_deref(), Some("Mr. Somchai"));
}

// --- Logical references via Identifier ---

#[test]
fn reference_logical_via_identifier() {
    let citizen = Identifier::new(
        Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap(),
        "1101700123456",
    )
    .official();
    let r = Reference::logical(citizen.clone());
    assert_eq!(r.identifier, Some(citizen));
    assert!(r.reference.is_none());
}

#[test]
fn reference_with_type_helps_logical_lookup() {
    let mrn = Identifier::new(
        Uri::new("http://hospital.example.com/mrn").unwrap(),
        "M-001",
    );
    let r = Reference::logical(mrn)
        .with_type(Uri::new("Patient").unwrap())
        .with_display("Patient by hospital MRN");
    assert_eq!(r.type_.as_ref().unwrap().as_str(), "Patient");
}

// --- Identifier.assigner round-trip (the Reference ↔ Identifier cycle) ---

#[test]
fn identifier_with_assigner_round_trips() {
    let hospital = Reference::literal("Organization/asgard-medical").with_display("Asgard Medical");
    let mrn = Identifier::new(Uri::new("urn:oid:1.2.3.4.5").unwrap(), "MRN-001")
        .official()
        .with_assigner(hospital.clone());

    assert_eq!(mrn.assigner.as_deref(), Some(&hospital));

    let json = serde_json::to_string(&mrn).unwrap();
    let restored: Identifier = serde_json::from_str(&json).unwrap();
    assert_eq!(mrn, restored);
}

// --- Serde — wire format ---

#[test]
fn reference_omits_none_fields() {
    let r = Reference::literal("Patient/X");
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["reference"], "Patient/X");
    assert!(json.get("identifier").is_none());
    assert!(json.get("type").is_none());
    assert!(json.get("display").is_none());
}

#[test]
fn reference_type_renamed_to_type_on_wire() {
    let r = Reference::literal("X").with_type(Uri::new("Patient").unwrap());
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["type"], "Patient");
    assert!(json.get("type_").is_none());
}

#[test]
fn reference_with_full_identifier_round_trips() {
    let snomed_org_type = CodeableConcept::from_coding(
        Coding::new(
            Uri::new("http://terminology.hl7.org/CodeSystem/v2-0203").unwrap(),
            Code::new("MR").unwrap(),
        )
        .with_display("Medical record number"),
    );
    let mrn = Identifier::new(
        Uri::new("urn:oid:1.2.36.146.595.217.0.1").unwrap(),
        "PT-1234",
    )
    .official()
    .with_type(snomed_org_type);
    let r = Reference::logical(mrn).with_display("Patient PT-1234 at OurHospital");

    let json = serde_json::to_string(&r).unwrap();
    let restored: Reference = serde_json::from_str(&json).unwrap();
    assert_eq!(r, restored);
}

#[test]
fn reference_deserialize_validates_inner_uri() {
    let json = r#"{"type":"has space"}"#;
    let result: Result<Reference, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Uri validation must propagate through Reference.type_"
    );
}
