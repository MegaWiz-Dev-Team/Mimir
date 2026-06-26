//! Pins the canonical terminology URIs and guards against drift between the
//! `terminology` module and the literals scattered in datatypes.

use mimir_fhir::datatypes::{Quantity, TH_SUB_DISTRICT_EXTENSION_URL};
use mimir_fhir::terminology;

#[test]
fn icd10_tm_is_the_moph_canonical() {
    // Convergence decision (2026-05-30): one ICD-10-TM URI for every Asgard repo.
    assert_eq!(
        terminology::ICD10_TM,
        "https://terminology.fhir.moph.go.th/CodeSystem/icd-10-tm"
    );
    // The US clinical modification is a *different* system and must not collide.
    assert_ne!(terminology::ICD10_TM, terminology::ICD10_CM);
}

#[test]
fn ucum_const_matches_quantity_associated_const() {
    // Single source of truth: if Quantity ever changes its UCUM literal, this fails.
    assert_eq!(terminology::UCUM, Quantity::UCUM_SYSTEM);
}

#[test]
fn sub_district_extension_is_reexported_not_duplicated() {
    assert_eq!(
        terminology::extension::TH_SUB_DISTRICT,
        TH_SUB_DISTRICT_EXTENSION_URL
    );
}

#[test]
fn citizen_id_identifier_system_is_stable() {
    assert_eq!(
        terminology::identifier::CITIZEN_ID,
        "https://fhir.moph.go.th/identifier/citizen-id"
    );
}

#[test]
fn all_systems_are_absolute_urls() {
    let systems = [
        terminology::ICD10_TM,
        terminology::ICD10,
        terminology::ICD9_CM,
        terminology::TMT,
        terminology::LOINC,
        terminology::SNOMED_CT,
        terminology::UCUM,
        terminology::hl7::CONDITION_CLINICAL,
        terminology::hl7::OBSERVATION_CATEGORY,
        terminology::hl7::ACT_CODE,
        terminology::identifier::CITIZEN_ID,
        terminology::extension::TH_SUB_DISTRICT,
        terminology::profile::BASE,
    ];
    for s in systems {
        assert!(
            s.starts_with("http://") || s.starts_with("https://"),
            "system URI is not an absolute URL: {s}"
        );
    }
}
