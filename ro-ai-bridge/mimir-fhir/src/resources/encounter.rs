//! FHIR R5 `Encounter` resource (Sprint 2).
//!
//! Second resource built (after `Patient`). Models the MoPH-PC1 encounter
//! subset (elements 34-39) using **R5** field names — precisely why R5 was
//! chosen over R4 for Asgard (see [ADR-013]):
//!
//! | concept               | R5 (here)                        | R4 (legacy)                            |
//! |-----------------------|----------------------------------|----------------------------------------|
//! | service time          | `actualPeriod`                   | `period`                               |
//! | discharge disposition | `admission.dischargeDisposition` | `hospitalization.dischargeDisposition` |
//!
//! The R4 spellings are handled only at the adapter boundary
//! ([`crate::translate::r4_to_r5`]).
//!
//! [ADR-013]: ../../../../Asgard/docs/decisions/ADR-013-fhir-r5-canonical-version.md

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    Code, CodeableConcept, Coding, Id, Identifier, Meta, Narrative, Period, Reference, Uri,
};
use crate::resources::ConversionError;
use crate::terminology;

resource_type_marker!(EncounterResourceType, "Encounter");

/// FHIR R5 `EncounterStatus` value set (`Encounter.status`, required `1..1`).
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-encounter-status.html>).
/// R5 reworked this set from R4 (e.g. R4 `finished` became R5 `completed`;
/// R4 `arrived` / `triaged` were removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EncounterStatus {
    Planned,
    InProgress,
    OnHold,
    Discharged,
    Completed,
    Cancelled,
    Discarded,
    EnteredInError,
    Unknown,
}

/// FHIR R5 `Encounter.diagnosis` backbone element.
///
/// Links the encounter to its relevant conditions (MoPH-PC element 36 —
/// `DIAGNOSIS_OPD` / `DIAGNOSIS_IPD`, ICD-10-TM coded).
///
/// **Scaffold note:** R5 types `diagnosis.condition` as `CodeableReference`
/// (a concept plus a `Reference`); that datatype is not implemented yet, so
/// this models the reference half as [`Reference`] pointing at a `Condition`.
/// The coded half migrates to `CodeableReference` when that datatype lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct EncounterDiagnosis {
    /// The condition(s) — each a reference to a `Condition` resource.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition: Vec<Reference>,

    /// Role of the diagnosis (admission, discharge, billing, ...). R5 renamed
    /// this from R4 `role` to `use`.
    #[serde(rename = "use", default, skip_serializing_if = "Vec::is_empty")]
    pub use_: Vec<CodeableConcept>,
}

/// FHIR R5 `Encounter.admission` backbone element (R4 name: `hospitalization`).
///
/// Admission / discharge logistics. The scaffold models only
/// `dischargeDisposition` (MoPH-PC element 39); other fields (`origin`,
/// `destination`, `admitSource`, ...) are added on demand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct EncounterAdmission {
    /// Where the patient goes after discharge (referred-out hospital, home, ...).
    #[serde(
        rename = "dischargeDisposition",
        skip_serializing_if = "Option::is_none"
    )]
    pub discharge_disposition: Option<CodeableConcept>,
}

/// FHIR R5 `Encounter.location` backbone element.
///
/// Where the encounter takes place (MoPH-PC element 38 — ward for IPD; OPD
/// typically has none). The `Location` resource is not implemented yet, so
/// `location` is a bare [`Reference`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EncounterLocation {
    /// Reference to the `Location` resource.
    pub location: Reference,
}

/// FHIR R5 `Encounter` resource.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/encounter.html>). `status` is the
/// only FHIR-required field (`1..1`); everything else is optional. Use
/// [`Encounter::outpatient`] / [`Encounter::inpatient`] to stamp the OPD / IPD
/// `class` correctly. Unknown wire fields are rejected — to ingest arbitrary
/// EHR exports use [`ExternalEncounter`] and convert via [`TryFrom`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Encounter {
    /// Resource discriminator — always serializes as `"resourceType": "Encounter"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: EncounterResourceType,

    /// Logical id of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,

    /// Resource metadata — `versionId` / `lastUpdated` derived from Tyr audit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,

    /// Human-readable narrative summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,

    /// Visit number (VN) / admission number (AN) etc. (MoPH-PC element 35).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,

    /// Encounter lifecycle state (required `1..1`).
    pub status: EncounterStatus,

    /// Classification — OPD (`AMB`) vs IPD (`IMP`) etc. (MoPH-PC element 34).
    /// R5 widened this to `0..*` `CodeableConcept` (R4 was a single `0..1`
    /// `Coding`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub class: Vec<CodeableConcept>,

    /// The patient (or group) this encounter is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,

    /// The actual service time. **R5 rename** of R4 `period` (MoPH-PC element 37).
    #[serde(rename = "actualPeriod", skip_serializing_if = "Option::is_none")]
    pub actual_period: Option<Period>,

    /// Diagnoses relevant to this encounter (MoPH-PC element 36).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnosis: Vec<EncounterDiagnosis>,

    /// Location(s) of the encounter (MoPH-PC element 38).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub location: Vec<EncounterLocation>,

    /// Admission / discharge logistics. **R5 rename** of R4 `hospitalization`
    /// (MoPH-PC element 39).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<EncounterAdmission>,
}

impl Encounter {
    /// Construct an `Encounter` with the required status and nothing else.
    #[must_use]
    pub fn new(status: EncounterStatus) -> Self {
        Self {
            resource_type: EncounterResourceType,
            id: None,
            meta: None,
            text: None,
            identifier: Vec::new(),
            status,
            class: Vec::new(),
            subject: None,
            actual_period: None,
            diagnosis: Vec::new(),
            location: Vec::new(),
            admission: None,
        }
    }

    /// `CodeableConcept` for an outpatient (OPD) encounter — HL7 v3 `AMB`.
    #[must_use]
    pub fn outpatient_class() -> CodeableConcept {
        Self::act_code_class("AMB", "ambulatory")
    }

    /// `CodeableConcept` for an inpatient (IPD) encounter — HL7 v3 `IMP`.
    #[must_use]
    pub fn inpatient_class() -> CodeableConcept {
        Self::act_code_class("IMP", "inpatient encounter")
    }

    /// Build an `Encounter.class` `CodeableConcept` from an HL7 v3 `ActCode`.
    ///
    /// Private — `code` / `display` are always valid grammar, so the inner
    /// `expect`s never fire.
    fn act_code_class(code: &str, display: &str) -> CodeableConcept {
        let coding = Coding::new(
            Uri::new(terminology::hl7::ACT_CODE).expect("valid ActCode system URI"),
            Code::new(code).expect("valid ActCode code"),
        )
        .with_display(display);
        CodeableConcept::from_coding(coding)
    }

    /// Mark this as an outpatient (OPD) encounter.
    #[must_use]
    pub fn outpatient(mut self) -> Self {
        self.class.push(Self::outpatient_class());
        self
    }

    /// Mark this as an inpatient (IPD) encounter.
    #[must_use]
    pub fn inpatient(mut self) -> Self {
        self.class.push(Self::inpatient_class());
        self
    }

    /// Set the subject (patient) reference.
    #[must_use]
    pub fn with_subject(mut self, subject: Reference) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Set the actual service period (R5 `actualPeriod`).
    #[must_use]
    pub fn with_actual_period(mut self, period: Period) -> Self {
        self.actual_period = Some(period);
        self
    }

    /// Add a diagnosis.
    #[must_use]
    pub fn add_diagnosis(mut self, diagnosis: EncounterDiagnosis) -> Self {
        self.diagnosis.push(diagnosis);
        self
    }

    /// Set admission / discharge logistics (R5 `admission`).
    #[must_use]
    pub fn with_admission(mut self, admission: EncounterAdmission) -> Self {
        self.admission = Some(admission);
        self
    }

    /// Add a business identifier (VN / AN).
    #[must_use]
    pub fn add_identifier(mut self, identifier: Identifier) -> Self {
        self.identifier.push(identifier);
        self
    }

    /// Set the logical resource id.
    #[must_use]
    pub fn with_id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }
}

/// Lenient ingest counterpart of [`Encounter`] (per [ADR-006] Decision 4).
///
/// Accepts external **R5** `Encounter` JSON. (R4 input goes through
/// [`crate::translate::r4_to_r5::encounter`] first.) `status` is optional here
/// but required by the canonical type, so [`TryFrom`] fails with
/// [`ConversionError::MissingRequiredField`] when it is absent.
///
/// [ADR-006]: ../../../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExternalEncounter {
    /// Logical id.
    pub id: Option<Id>,
    /// Business identifiers (VN / AN).
    pub identifier: Vec<Identifier>,
    /// Lifecycle state — required by the canonical type.
    pub status: Option<EncounterStatus>,
    /// Classification (OPD / IPD).
    pub class: Vec<CodeableConcept>,
    /// Subject (patient) reference.
    pub subject: Option<Reference>,
    /// Actual service time (R5 `actualPeriod`).
    #[serde(rename = "actualPeriod")]
    pub actual_period: Option<Period>,
    /// Diagnoses.
    pub diagnosis: Vec<EncounterDiagnosis>,
    /// Locations.
    pub location: Vec<EncounterLocation>,
    /// Admission / discharge logistics (R5 `admission`).
    pub admission: Option<EncounterAdmission>,
}

impl TryFrom<ExternalEncounter> for Encounter {
    type Error = ConversionError;

    fn try_from(ext: ExternalEncounter) -> Result<Self, Self::Error> {
        let status = ext.status.ok_or(ConversionError::MissingRequiredField {
            resource: "Encounter",
            field: "status",
        })?;
        Ok(Self {
            resource_type: EncounterResourceType,
            id: ext.id,
            meta: None,
            text: None,
            identifier: ext.identifier,
            status,
            class: ext.class,
            subject: ext.subject,
            actual_period: ext.actual_period,
            diagnosis: ext.diagnosis,
            location: ext.location,
            admission: ext.admission,
        })
    }
}
