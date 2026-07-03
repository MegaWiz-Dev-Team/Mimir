//! TDD tests for the FHIR R5 `Encounter` resource — R5 field names
//! (`actualPeriod`, `admission`), OPD / IPD class, and lenient ingest.

use mimir_fhir::datatypes::{CodeableConcept, DateTime, Period, Reference, Uri};
use mimir_fhir::resources::{
    ConversionError, Encounter, EncounterAdmission, EncounterDiagnosis, EncounterStatus,
    ExternalEncounter,
};

#[test]
fn encounter_carries_required_status() {
    let e = Encounter::new(EncounterStatus::InProgress);
    assert_eq!(e.status, EncounterStatus::InProgress);
}

#[test]
fn encounter_status_serializes_kebab_case() {
    let json = serde_json::to_value(Encounter::new(EncounterStatus::InProgress)).unwrap();
    assert_eq!(json["status"], "in-progress");
    assert_eq!(json["resourceType"], "Encounter");
}

#[test]
fn encounter_entered_in_error_is_kebab_case() {
    assert_eq!(
        serde_json::to_value(EncounterStatus::EnteredInError).unwrap(),
        "entered-in-error"
    );
}

// Guards the R5 value-set code: the variant is `Discontinued`, and it MUST
// serialize as `"discontinued"` (not `"discarded"`). Because `Encounter` is
// `deny_unknown_fields` with no catch-all variant, a wrong code would also make
// real R5 payloads with `status:"discontinued"` fail to deserialize.
#[test]
fn encounter_discontinued_uses_r5_value_set_code() {
    assert_eq!(
        serde_json::to_value(EncounterStatus::Discontinued).unwrap(),
        "discontinued"
    );
    // Round-trips from the wire code an external R5 system would send.
    let parsed: EncounterStatus = serde_json::from_value(serde_json::json!("discontinued")).unwrap();
    assert_eq!(parsed, EncounterStatus::Discontinued);
}

#[test]
fn opd_class_uses_ambulatory_actcode() {
    let e = Encounter::new(EncounterStatus::Completed).outpatient();
    let coding = &e.class[0].coding[0];
    assert_eq!(coding.code.as_ref().unwrap().as_str(), "AMB");
    assert_eq!(
        coding.system.as_ref().unwrap().as_str(),
        "http://terminology.hl7.org/CodeSystem/v3-ActCode"
    );
}

#[test]
fn ipd_class_uses_inpatient_actcode() {
    let e = Encounter::new(EncounterStatus::Completed).inpatient();
    assert_eq!(e.class[0].coding[0].code.as_ref().unwrap().as_str(), "IMP");
}

#[test]
fn encounter_actual_period_is_the_r5_field_name() {
    let e = Encounter::new(EncounterStatus::Completed).with_actual_period(Period::between(
        DateTime::new("2026-05-24T09:00:00Z").unwrap(),
        DateTime::new("2026-05-24T10:30:00Z").unwrap(),
    ));
    let json = serde_json::to_value(&e).unwrap();
    // R5 uses `actualPeriod`, NOT R4 `period`.
    assert!(json.get("actualPeriod").is_some());
    assert!(json.get("period").is_none());
}

#[test]
fn encounter_admission_discharge_disposition_is_the_r5_field_name() {
    let e = Encounter::new(EncounterStatus::Completed).with_admission(EncounterAdmission {
        discharge_disposition: Some(CodeableConcept::from_text("home")),
    });
    let json = serde_json::to_value(&e).unwrap();
    // R5 uses `admission.dischargeDisposition`, NOT R4 `hospitalization.*`.
    assert_eq!(json["admission"]["dischargeDisposition"]["text"], "home");
    assert!(json.get("hospitalization").is_none());
}

#[test]
fn encounter_diagnosis_references_a_condition() {
    let diag = EncounterDiagnosis {
        condition: vec![
            Reference::literal("Condition/abc").with_type(Uri::new("Condition").unwrap())
        ],
        use_: vec![],
    };
    let e = Encounter::new(EncounterStatus::Completed).add_diagnosis(diag);
    assert_eq!(
        e.diagnosis[0].condition[0].reference.as_deref(),
        Some("Condition/abc")
    );
}

#[test]
fn encounter_round_trips() {
    let e = Encounter::new(EncounterStatus::Completed)
        .outpatient()
        .with_subject(Reference::literal("Patient/123"))
        .with_actual_period(Period::starting(DateTime::new("2026-05-24").unwrap()));
    let json = serde_json::to_string(&e).unwrap();
    let back: Encounter = serde_json::from_str(&json).unwrap();
    assert_eq!(e, back);
}

#[test]
fn external_encounter_missing_status_fails() {
    let ext: ExternalEncounter = serde_json::from_str(r#"{"resourceType":"Encounter"}"#).unwrap();
    let err = Encounter::try_from(ext).unwrap_err();
    assert_eq!(
        err,
        ConversionError::MissingRequiredField {
            resource: "Encounter",
            field: "status",
        }
    );
}

#[test]
fn external_encounter_ignores_unknown_fields() {
    let json = r#"{
        "resourceType":"Encounter",
        "status":"completed",
        "serviceProvider":{"reference":"Organization/1"},
        "appointment":[{"reference":"Appointment/9"}]
    }"#;
    let ext: ExternalEncounter = serde_json::from_str(json).unwrap();
    let e = Encounter::try_from(ext).unwrap();
    assert_eq!(e.status, EncounterStatus::Completed);
}

#[test]
fn encounter_rejects_unknown_field_strict() {
    let r: Result<Encounter, _> = serde_json::from_str(
        r#"{"resourceType":"Encounter","status":"completed","appointment":[]}"#,
    );
    assert!(r.is_err());
}

#[test]
fn encounter_appears_in_resource_schemas() {
    let schemas = mimir_fhir::schema_export::all_resource_schemas();
    assert!(schemas.contains_key("Encounter"));
}
