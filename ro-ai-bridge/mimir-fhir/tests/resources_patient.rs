//! TDD tests for the FHIR R5 `Patient` resource — TH conventions, strict wire
//! format, and lenient (`ExternalPatient`) ingest.

use mimir_fhir::datatypes::{Address, ContactPoint, Date, HumanName};
use mimir_fhir::resources::{AdministrativeGender, ExternalPatient, Patient};

fn sample_patient() -> Patient {
    Patient::with_citizen_id("1234567890123")
        .add_name(HumanName::thai("บุญส่ง", "กิติชัย"))
        .add_name(HumanName::english("Boonsong", "Kittichai"))
        .with_gender(AdministrativeGender::Male)
        .with_birth_date(Date::new("1980-01-15").unwrap())
        .add_telecom(ContactPoint::phone("+66-2-123-4567"))
        .add_address(Address::thai(
            "123 ถ.สีลม",
            "สีลม",
            "บางรัก",
            "กรุงเทพมหานคร",
            "10500",
        ))
}

#[test]
fn patient_with_citizen_id_sets_official_identifier() {
    let p = Patient::with_citizen_id("1234567890123");
    assert_eq!(p.citizen_id(), Some("1234567890123"));
    let id = &p.identifier[0];
    assert_eq!(
        id.system.as_ref().unwrap().as_str(),
        "https://fhir.moph.go.th/identifier/citizen-id"
    );
}

#[test]
fn patient_serializes_resource_type() {
    let json = serde_json::to_value(sample_patient()).unwrap();
    assert_eq!(json["resourceType"], "Patient");
}

#[test]
fn patient_uses_camelcase_birth_date_on_wire() {
    let json = serde_json::to_value(sample_patient()).unwrap();
    assert_eq!(json["birthDate"], "1980-01-15");
    assert!(json.get("birth_date").is_none());
}

#[test]
fn patient_carries_bilingual_names() {
    let json = serde_json::to_value(sample_patient()).unwrap();
    let names = json["name"].as_array().unwrap();
    assert_eq!(names.len(), 2);
}

#[test]
fn patient_omits_empty_collections_but_keeps_resource_type() {
    let json = serde_json::to_value(Patient::new()).unwrap();
    assert!(json.get("identifier").is_none());
    assert!(json.get("name").is_none());
    assert!(json.get("contact").is_none());
    assert_eq!(json["resourceType"], "Patient");
}

#[test]
fn patient_round_trips() {
    let p = sample_patient();
    let json = serde_json::to_string(&p).unwrap();
    let back: Patient = serde_json::from_str(&json).unwrap();
    assert_eq!(p, back);
}

#[test]
fn patient_rejects_wrong_resource_type() {
    let r: Result<Patient, _> = serde_json::from_str(r#"{"resourceType":"Observation"}"#);
    assert!(r.is_err());
}

#[test]
fn patient_rejects_unknown_field_strict() {
    // Canonical `Patient` is `deny_unknown_fields`.
    let r: Result<Patient, _> = serde_json::from_str(r#"{"resourceType":"Patient","careTeam":[]}"#);
    assert!(r.is_err());
}

#[test]
fn external_patient_ignores_unknown_fields() {
    // The lenient ingest type silently drops fields we do not model.
    let json = r#"{
        "resourceType":"Patient",
        "id":"abc",
        "gender":"female",
        "careTeam":[{"reference":"CareTeam/1"}],
        "photo":[{"contentType":"image/png"}],
        "_birthDate":{"extension":[]}
    }"#;
    let ext: ExternalPatient = serde_json::from_str(json).unwrap();
    assert_eq!(ext.gender, Some(AdministrativeGender::Female));

    let p: Patient = ext.try_into().unwrap();
    assert_eq!(p.gender, Some(AdministrativeGender::Female));
    assert_eq!(p.id.unwrap().as_str(), "abc");
}

#[test]
fn external_patient_conversion_is_total() {
    // `Patient` has no required fields — an empty external patient converts.
    let ext: ExternalPatient = serde_json::from_str("{}").unwrap();
    let p = Patient::try_from(ext).unwrap();
    assert!(p.identifier.is_empty());
    // The discriminator is stamped on conversion.
    assert_eq!(p.resource_type, Patient::new().resource_type);
}

#[test]
fn administrative_gender_serializes_lowercase() {
    for (g, s) in [
        (AdministrativeGender::Male, "male"),
        (AdministrativeGender::Female, "female"),
        (AdministrativeGender::Other, "other"),
        (AdministrativeGender::Unknown, "unknown"),
    ] {
        assert_eq!(serde_json::to_value(g).unwrap(), s);
    }
}

#[test]
fn patient_appears_in_resource_schemas() {
    let schemas = mimir_fhir::schema_export::all_resource_schemas();
    assert!(schemas.contains_key("Patient"));
}
