//! Minimal FHIR **R4** wire types for the fields that differ from R5.
//!
//! Only the subset Asgard ingests is modeled. These exist purely so the
//! adapters in [`super::r4_to_r5`] / [`super::r5_to_r4`] have a concrete R4
//! shape to translate; the rest of the codebase only ever sees the R5
//! [`crate::resources`] types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    Address, CodeableConcept, Coding, ContactPoint, Date, HumanName, Id, Identifier, Period,
    Reference,
};
use crate::resources::AdministrativeGender;

/// FHIR **R4** `Patient` — structurally identical to R5 for the modeled subset
/// (the R4 → R5 `Patient` translation is pure pass-through).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct R4Patient {
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Business identifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Active flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<HumanName>,
    /// Contact channels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,
    /// Administrative gender.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<AdministrativeGender>,
    /// Date of birth.
    #[serde(rename = "birthDate", skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<Date>,
    /// Addresses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<Address>,
}

/// FHIR **R4** `Encounter.hospitalization` backbone (R5 renamed it `admission`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct R4Hospitalization {
    /// Where the patient goes after discharge.
    #[serde(
        rename = "dischargeDisposition",
        skip_serializing_if = "Option::is_none"
    )]
    pub discharge_disposition: Option<CodeableConcept>,
}

/// FHIR **R4** `Encounter` — differs from R5 in two field spellings handled by
/// the adapter: `period` (R5 `actualPeriod`) and `hospitalization`
/// (R5 `admission`). R4 `class` is a single `Coding` (R5 widened it to
/// `0..*` `CodeableConcept`); R4 `status` uses a different value set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct R4Encounter {
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Business identifiers (VN / AN).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// R4 status code (kept as a raw string at the R4 boundary — R4 and R5
    /// status value sets differ; known values are remapped by the adapter).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// R4 `class` — a single `Coding` (`0..1`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<Coding>,
    /// Subject (patient) reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    /// R4 `period` — R5 renames this to `actualPeriod`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// R4 `hospitalization` — R5 renames this to `admission`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hospitalization: Option<R4Hospitalization>,
}
