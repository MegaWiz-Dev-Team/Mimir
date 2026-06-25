//! TDD tests for the R4 ↔ R5 translator scaffold — `Patient` pass-through,
//! `Encounter` renames, and round-trip identity on the lossless subset.

use mimir_fhir::datatypes::{
    Code, CodeableConcept, Coding, DateTime, HumanName, Id, Identifier, Period, Reference, Uri,
};
use mimir_fhir::resources::{AdministrativeGender, EncounterStatus};
use mimir_fhir::translate::r4::{R4Encounter, R4Hospitalization, R4Patient};
use mimir_fhir::translate::{r4_to_r5, r5_to_r4};

const ACT_CODE: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";

#[test]
fn r4_to_r5_patient_is_pass_through() {
    let r4 = R4Patient {
        name: vec![HumanName::english("Doe", "Jane")],
        ..R4Patient::default()
    };
    let r5 = r4_to_r5::patient(r4);
    assert_eq!(r5.name[0].family.as_deref(), Some("Doe"));
    // The R5 `Patient` stamps its `resourceType`.
    let json = serde_json::to_value(&r5).unwrap();
    assert_eq!(json["resourceType"], "Patient");
}

#[test]
fn r4_to_r5_encounter_renames_period_to_actual_period() {
    let r4 = R4Encounter {
        status: Some("finished".to_string()),
        period: Some(Period::starting(DateTime::new("2026-05-24").unwrap())),
        ..R4Encounter::default()
    };
    let r5 = r4_to_r5::encounter(r4);
    assert_eq!(r5.status, EncounterStatus::Completed); // finished -> completed
    assert!(r5.actual_period.is_some());
}

#[test]
fn r4_to_r5_encounter_renames_hospitalization_to_admission() {
    let r4 = R4Encounter {
        status: Some("finished".to_string()),
        hospitalization: Some(R4Hospitalization {
            discharge_disposition: Some(CodeableConcept::from_text("home")),
        }),
        ..R4Encounter::default()
    };
    let r5 = r4_to_r5::encounter(r4);
    let dd = r5.admission.unwrap().discharge_disposition.unwrap();
    assert_eq!(dd.text.as_deref(), Some("home"));
}

#[test]
fn r4_to_r5_encounter_widens_class_coding_to_codeable_concept() {
    let r4 = R4Encounter {
        status: Some("in-progress".to_string()),
        class: Some(Coding::new(
            Uri::new(ACT_CODE).unwrap(),
            Code::new("AMB").unwrap(),
        )),
        ..R4Encounter::default()
    };
    let r5 = r4_to_r5::encounter(r4);
    assert_eq!(r5.class.len(), 1);
    assert_eq!(r5.class[0].coding[0].code.as_ref().unwrap().as_str(), "AMB");
}

#[test]
fn unknown_r4_status_maps_to_unknown() {
    assert_eq!(
        r4_to_r5::r4_status_to_r5("nonsense"),
        EncounterStatus::Unknown
    );
    // R4-only states collapse to in-progress.
    assert_eq!(
        r4_to_r5::r4_status_to_r5("arrived"),
        EncounterStatus::InProgress
    );
}

#[test]
fn encounter_round_trips_r4_to_r5_to_r4_on_lossless_subset() {
    let original = R4Encounter {
        id: Some(Id::new("enc-1").unwrap()),
        identifier: vec![Identifier::new(
            Uri::new("https://fhir.moph.go.th/identifier/an").unwrap(),
            "AN-9001",
        )],
        status: Some("finished".to_string()),
        class: Some(Coding::new(
            Uri::new(ACT_CODE).unwrap(),
            Code::new("IMP").unwrap(),
        )),
        subject: Some(Reference::literal("Patient/123")),
        period: Some(Period::between(
            DateTime::new("2026-05-24T09:00:00Z").unwrap(),
            DateTime::new("2026-05-25T12:00:00Z").unwrap(),
        )),
        hospitalization: Some(R4Hospitalization {
            discharge_disposition: Some(CodeableConcept::from_text("referred-out")),
        }),
    };

    let r5 = r4_to_r5::encounter(original.clone());
    let back = r5_to_r4::encounter(&r5);
    assert_eq!(original, back);
}

#[test]
fn patient_round_trips_r4_to_r5_to_r4() {
    let original = R4Patient {
        id: Some(Id::new("pat-1").unwrap()),
        name: vec![HumanName::thai("บุญส่ง", "กิติชัย")],
        gender: Some(AdministrativeGender::Male),
        ..R4Patient::default()
    };
    let r5 = r4_to_r5::patient(original.clone());
    let back = r5_to_r4::patient(&r5);
    assert_eq!(original, back);
}
