//! TDD tests for Sprint 1 Day 7 metadata datatypes —
//! `Instant`, `Extension` (full polymorphism), `Annotation`, `Meta`,
//! `Narrative`, `NarrativeStatus`.

use std::str::FromStr;

use mimir_fhir::datatypes::{
    Annotation, Code, CodeableConcept, Coding, DateTime, Decimal, Extension, Instant, InstantError,
    Markdown, Meta, Narrative, NarrativeStatus, Quantity, Reference, Uri,
    TH_SUB_DISTRICT_EXTENSION_URL,
};

// =============================================================================
// Instant
// =============================================================================

#[test]
fn instant_accepts_full_rfc3339_with_tz() {
    assert!(Instant::new("2026-05-25T07:00:00+07:00").is_ok());
    assert!(Instant::new("2026-05-25T00:00:00Z").is_ok());
    assert!(Instant::new("2026-05-25T07:00:00.123Z").is_ok());
}

#[test]
fn instant_rejects_partial_dates() {
    // Instant is stricter than DateTime — partial precision NOT allowed
    assert!(matches!(
        Instant::new("2026"),
        Err(InstantError::InvalidFormat(_))
    ));
    assert!(matches!(
        Instant::new("2026-05"),
        Err(InstantError::InvalidFormat(_))
    ));
    assert!(matches!(
        Instant::new("2026-05-25"),
        Err(InstantError::InvalidFormat(_))
    ));
}

#[test]
fn instant_rejects_missing_timezone() {
    assert!(Instant::new("2026-05-25T07:00:00").is_err());
}

#[test]
fn instant_serializes_as_string() {
    let i = Instant::new("2026-05-25T07:00:00Z").unwrap();
    let json = serde_json::to_string(&i).unwrap();
    assert_eq!(json, "\"2026-05-25T07:00:00Z\"");
}

#[test]
fn instant_round_trips() {
    let original = Instant::new("2026-05-25T07:00:00+07:00").unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Instant = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// =============================================================================
// Extension — full polymorphism (9 value[x] variants)
// =============================================================================

#[test]
fn extension_string_variant_still_works() {
    // Sprint 1 Day 5 behavior preserved after Day 7 expansion
    let ext = Extension::string(Uri::new(TH_SUB_DISTRICT_EXTENSION_URL).unwrap(), "สีลม");
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueString"], "สีลม");
    assert!(json.get("valueCode").is_none());
}

#[test]
fn extension_code_variant() {
    let ext = Extension::code(
        Uri::new("https://example.com/StructureDefinition/severity").unwrap(),
        Code::new("severe").unwrap(),
    );
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueCode"], "severe");
    assert!(json.get("valueString").is_none());
}

#[test]
fn extension_boolean_variant() {
    let ext = Extension::boolean(
        Uri::new("https://example.com/StructureDefinition/is-emergency").unwrap(),
        true,
    );
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueBoolean"], true);
}

#[test]
fn extension_datetime_variant() {
    let ext = Extension::datetime(
        Uri::new("https://example.com/StructureDefinition/onset").unwrap(),
        DateTime::new("2024-01-15").unwrap(),
    );
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueDateTime"], "2024-01-15");
}

#[test]
fn extension_decimal_variant_with_clinical_precision() {
    let mut ext = Extension::string(
        Uri::new("https://example.com/StructureDefinition/dose").unwrap(),
        "placeholder",
    );
    ext.value_string = None;
    ext.value_decimal = Some(Decimal::from_str("2.5").unwrap());
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueDecimal"], "2.5");
    assert!(json.get("valueString").is_none());
}

#[test]
fn extension_quantity_variant() {
    let mut ext = Extension::string(
        Uri::new("https://example.com/StructureDefinition/dose-amount").unwrap(),
        "placeholder",
    );
    ext.value_string = None;
    ext.value_quantity = Some(Quantity::ucum(
        Decimal::from_str("500").unwrap(),
        "mg",
        Code::new("mg").unwrap(),
    ));
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueQuantity"]["unit"], "mg");
}

#[test]
fn extension_codeable_concept_variant() {
    let mut ext = Extension::string(
        Uri::new("https://example.com/StructureDefinition/diagnosis-tag").unwrap(),
        "placeholder",
    );
    ext.value_string = None;
    ext.value_codeable_concept = Some(CodeableConcept::from_text("Hypertension"));
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueCodeableConcept"]["text"], "Hypertension");
}

#[test]
fn extension_reference_variant() {
    let mut ext = Extension::string(
        Uri::new("https://example.com/StructureDefinition/related-patient").unwrap(),
        "placeholder",
    );
    ext.value_string = None;
    ext.value_reference = Some(Reference::literal("Patient/A12345"));
    let json = serde_json::to_value(&ext).unwrap();
    assert_eq!(json["valueReference"]["reference"], "Patient/A12345");
}

#[test]
fn extension_nested_children() {
    // E.g. Thai address mapping that needs sub-district + sub-district code as
    // two structured children rather than one valueString
    let inner1 = Extension::string(
        Uri::new("https://example.com/sub-district-name").unwrap(),
        "สีลม",
    );
    let inner2 = Extension::code(
        Uri::new("https://example.com/sub-district-code").unwrap(),
        Code::new("101001").unwrap(),
    );
    let parent = Extension::nested(
        Uri::new("https://example.com/sub-district").unwrap(),
        vec![inner1, inner2],
    );
    let json = serde_json::to_value(&parent).unwrap();
    assert!(json["extension"].is_array());
    assert_eq!(json["extension"].as_array().unwrap().len(), 2);
    // No scalar value[x] on parent
    assert!(json.get("valueString").is_none());
}

#[test]
fn extension_round_trips_quantity() {
    let mut original = Extension::string(Uri::new("https://example.com/Q").unwrap(), "placeholder");
    original.value_string = None;
    original.value_quantity = Some(Quantity::ucum(
        Decimal::from_str("70").unwrap(),
        "kg",
        Code::new("kg").unwrap(),
    ));
    let json = serde_json::to_string(&original).unwrap();
    let restored: Extension = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// =============================================================================
// Annotation
// =============================================================================

#[test]
fn annotation_text_only_factory() {
    let a = Annotation::text_only(Markdown::new("Patient stable").unwrap());
    assert!(a.author_reference.is_none());
    assert!(a.author_string.is_none());
    assert!(a.time.is_none());
}

#[test]
fn annotation_by_reference_factory() {
    let author = Reference::literal("Practitioner/dr-somchai");
    let a = Annotation::by_reference(
        author.clone(),
        Markdown::new("Reviewed and signed").unwrap(),
    );
    assert_eq!(a.author_reference, Some(author));
    assert!(a.author_string.is_none());
}

#[test]
fn annotation_by_string_factory_for_legacy_unstructured_author() {
    let a = Annotation::by_string(
        "Nurse Wilai",
        Markdown::new("Pre-op note from paper chart").unwrap(),
    );
    assert!(a.author_reference.is_none());
    assert_eq!(a.author_string.as_deref(), Some("Nurse Wilai"));
}

#[test]
fn annotation_with_time_sets_time() {
    let a = Annotation::text_only(Markdown::new("note").unwrap())
        .with_time(DateTime::new("2026-05-25").unwrap());
    assert_eq!(a.time.as_ref().unwrap().as_str(), "2026-05-25");
}

#[test]
fn annotation_author_polymorphism_serializes_camel_case() {
    let a = Annotation::by_reference(
        Reference::literal("Practitioner/X"),
        Markdown::new("note").unwrap(),
    );
    let json = serde_json::to_value(&a).unwrap();
    assert!(json.get("authorReference").is_some());
    assert!(json.get("author_reference").is_none());
    assert!(json.get("authorString").is_none());
}

#[test]
fn annotation_round_trips_with_full_polymorphism() {
    let original = Annotation::by_string(
        "Dr. Somchai",
        Markdown::new("Patient reported chest pain at 14:30").unwrap(),
    )
    .with_time(DateTime::new("2026-05-25T14:35:00+07:00").unwrap());
    let json = serde_json::to_string(&original).unwrap();
    let restored: Annotation = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// =============================================================================
// Meta
// =============================================================================

#[test]
fn meta_conforming_to_single_profile() {
    let profile_url = Uri::new("https://fhir.moph.go.th/StructureDefinition/TH-Patient").unwrap();
    let m = Meta::conforming_to(profile_url.clone());
    assert_eq!(m.profile, vec![profile_url]);
    assert!(m.version_id.is_none()); // derived from Tyr — not set at construction
    assert!(m.last_updated.is_none());
}

#[test]
fn meta_multiple_profiles() {
    let m = Meta::with_profiles(vec![
        Uri::new("https://fhir.moph.go.th/StructureDefinition/TH-Patient").unwrap(),
        Uri::new("https://fhir.moph.go.th/StructureDefinition/MoPH-PC-Patient").unwrap(),
    ]);
    assert_eq!(m.profile.len(), 2);
}

#[test]
fn meta_omits_empty_fields() {
    let m = Meta::default();
    let json = serde_json::to_value(&m).unwrap();
    assert!(json.get("versionId").is_none());
    assert!(json.get("lastUpdated").is_none());
    assert!(json.get("profile").is_none());
    assert!(json.get("security").is_none());
    assert!(json.get("tag").is_none());
}

#[test]
fn meta_round_trips_with_security_and_tag() {
    let original = Meta {
        version_id: Some("event-12345".into()),
        last_updated: Some(Instant::new("2026-05-25T07:00:00Z").unwrap()),
        source: Some(Uri::new("urn:mimir-fhir").unwrap()),
        profile: vec![Uri::new("https://fhir.moph.go.th/StructureDefinition/TH-Patient").unwrap()],
        security: vec![Coding::new(
            Uri::new("http://terminology.hl7.org/CodeSystem/v3-Confidentiality").unwrap(),
            Code::new("R").unwrap(),
        )
        .with_display("Restricted")],
        tag: vec![Coding::new(
            Uri::new("http://example.com/asgard/tenant").unwrap(),
            Code::new("asgard_medical").unwrap(),
        )],
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: Meta = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn meta_versionid_renames_to_camel_case() {
    let m = Meta {
        version_id: Some("v3".into()),
        ..Meta::default()
    };
    let json = serde_json::to_value(&m).unwrap();
    assert_eq!(json["versionId"], "v3");
    assert!(json.get("version_id").is_none());
}

// =============================================================================
// Narrative
// =============================================================================

#[test]
fn narrative_empty_factory() {
    let n = Narrative::empty();
    assert_eq!(n.status, NarrativeStatus::Empty);
    assert!(n.div.contains("<div"));
}

#[test]
fn narrative_generated_factory() {
    let n = Narrative::generated(
        r#"<div xmlns="http://www.w3.org/1999/xhtml"><p>Patient summary</p></div>"#,
    );
    assert_eq!(n.status, NarrativeStatus::Generated);
    assert!(n.div.contains("Patient summary"));
}

#[test]
fn narrative_status_serializes_lowercase() {
    let cases = [
        (NarrativeStatus::Generated, "generated"),
        (NarrativeStatus::Extensions, "extensions"),
        (NarrativeStatus::Additional, "additional"),
        (NarrativeStatus::Empty, "empty"),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_value(variant).unwrap();
        assert_eq!(json, expected);
    }
}

#[test]
fn narrative_round_trips() {
    let original = Narrative::generated(
        r#"<div xmlns="http://www.w3.org/1999/xhtml"><b>Lab results</b>: HbA1c 7.2%</div>"#,
    );
    let json = serde_json::to_string(&original).unwrap();
    let restored: Narrative = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}
