//! FHIR R5 `Patient` resource (Sprint 2) + TH Core / MoPH-PC conventions.
//!
//! `Patient` is the most-referenced resource — `Encounter.subject`,
//! `Observation.subject`, `Condition.subject`, etc. all point at it. Built and
//! tested first per the Phase 1 TDD order.
//!
//! ## Thai conventions baked in
//!
//! - **Citizen id** — the 13-digit national id as an [`Identifier`] whose
//!   `system` is [`terminology::identifier::CITIZEN_ID`]. [`Patient::with_citizen_id`]
//!   is the ergonomic constructor.
//! - **Bilingual names** — two [`HumanName`] entries (Thai + Latin) per patient
//!   (per [ADR-006] D5); see [`HumanName::thai`] / [`HumanName::english`].
//! - **Thai address** — 4-level mapping with sub-district extension via
//!   [`Address::thai`].
//!
//! [ADR-006]: ../../../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    Address, CodeableConcept, ContactPoint, Date, Extension, HumanName, Id, Identifier, Meta,
    Narrative, Uri,
};
use crate::terminology;

resource_type_marker!(PatientResourceType, "Patient");

/// FHIR R5 `AdministrativeGender` value set (`Patient.gender`, `contact.gender`).
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-administrative-gender.html>).
/// Note (MoPH-PC element 4): this is **biological / administrative sex**;
/// gender identity is a separate concept not modeled by PC1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AdministrativeGender {
    Male,
    Female,
    Other,
    Unknown,
}

/// FHIR R5 `Patient.contact` backbone element.
///
/// An emergency / next-of-kin contact carried inline on the patient (MoPH-PC
/// elements 8 + 9 — `contact.name`, `contact.relationship`). A contact that is
/// itself a tracked person uses a separate `RelatedPerson` resource (deferred).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct PatientContact {
    /// The kind of relationship (e.g. emergency contact, parent).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationship: Vec<CodeableConcept>,

    /// Name of the contact person.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<HumanName>,

    /// Contact details (phone, email) for the contact person.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,

    /// Address of the contact person.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,

    /// Administrative gender of the contact person.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<AdministrativeGender>,
}

/// FHIR R5 `Patient` resource.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/patient.html>). Models the subset
/// in MoPH-PC1 (elements 1-9) plus the standard `DomainResource` base
/// (`id`, `meta`, `text`). Unknown fields are rejected on the wire
/// (`deny_unknown_fields`) — to ingest arbitrary EHR exports use
/// [`ExternalPatient`] and convert via [`TryFrom`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Patient {
    /// Resource discriminator — always serializes as `"resourceType": "Patient"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: PatientResourceType,

    /// Logical id of the resource (assigned by Asgard, distinct from citizen id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,

    /// Resource metadata — `versionId` / `lastUpdated` derived from Tyr audit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,

    /// Human-readable narrative summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,

    /// Business identifiers — Thai citizen id, HN, etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,

    /// Whether this patient record is in active use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    /// Patient names — by Thai convention, one Thai-script + one Latin entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<HumanName>,

    /// Contact channels (phone, email).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,

    /// Administrative / biological sex (MoPH-PC element 4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<AdministrativeGender>,

    /// Date of birth — FHIR `date`, so partial precision (year-only) is valid.
    #[serde(rename = "birthDate", skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<Date>,

    /// Addresses — Thai 4-level structure via [`Address::thai`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<Address>,

    /// Emergency / next-of-kin contacts (MoPH-PC elements 8-9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contact: Vec<PatientContact>,

    /// Resource-level extensions (e.g. nationality, race — TH Core).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extension: Vec<Extension>,
}

impl Patient {
    /// Construct an empty `Patient` (just the resource discriminator).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a `Patient` keyed by Thai national citizen id (13 digits).
    ///
    /// Stored as an official [`Identifier`] whose `system` is
    /// [`terminology::identifier::CITIZEN_ID`]. No check-digit validation here
    /// — that is a profile concern (Sprint 7).
    ///
    /// # Panics
    ///
    /// Does not panic — the citizen-id system URI is always valid grammar.
    #[must_use]
    pub fn with_citizen_id(citizen_id: impl Into<String>) -> Self {
        let identifier = Identifier::new(
            Uri::new(terminology::identifier::CITIZEN_ID).expect("valid citizen-id system URI"),
            citizen_id,
        )
        .official();
        Self {
            identifier: vec![identifier],
            ..Self::default()
        }
    }

    /// Extract the Thai citizen id value, if present.
    #[must_use]
    pub fn citizen_id(&self) -> Option<&str> {
        self.identifier
            .iter()
            .find(|i| {
                i.system.as_ref().map(Uri::as_str) == Some(terminology::identifier::CITIZEN_ID)
            })
            .and_then(|i| i.value.as_deref())
    }

    /// Set the logical resource id.
    #[must_use]
    pub fn with_id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Add a name entry (chainable — call twice for the Thai + Latin pair).
    #[must_use]
    pub fn add_name(mut self, name: HumanName) -> Self {
        self.name.push(name);
        self
    }

    /// Set administrative gender.
    #[must_use]
    pub fn with_gender(mut self, gender: AdministrativeGender) -> Self {
        self.gender = Some(gender);
        self
    }

    /// Set date of birth.
    #[must_use]
    pub fn with_birth_date(mut self, birth_date: Date) -> Self {
        self.birth_date = Some(birth_date);
        self
    }

    /// Add a contact channel (phone, email).
    #[must_use]
    pub fn add_telecom(mut self, telecom: ContactPoint) -> Self {
        self.telecom.push(telecom);
        self
    }

    /// Add an address.
    #[must_use]
    pub fn add_address(mut self, address: Address) -> Self {
        self.address.push(address);
        self
    }

    /// Add an emergency / next-of-kin contact.
    #[must_use]
    pub fn add_contact(mut self, contact: PatientContact) -> Self {
        self.contact.push(contact);
        self
    }

    /// Mark the record active / inactive.
    #[must_use]
    pub fn with_active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    /// Add a business identifier (HN, AN, passport, ...).
    #[must_use]
    pub fn add_identifier(mut self, identifier: Identifier) -> Self {
        self.identifier.push(identifier);
        self
    }
}

/// Lenient ingest counterpart of [`Patient`] (per [ADR-006] Decision 4).
///
/// Parses arbitrary external FHIR `Patient` JSON: missing fields default, and
/// fields Asgard does not model (`careTeam`, `photo`, custom EHR extensions,
/// primitive `_birthDate` extensions, ...) are silently dropped. Convert into
/// the strict [`Patient`] via [`TryFrom`].
///
/// [ADR-006]: ../../../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExternalPatient {
    /// Logical id, if the source supplied one.
    pub id: Option<Id>,
    /// Business identifiers.
    pub identifier: Vec<Identifier>,
    /// Active flag.
    pub active: Option<bool>,
    /// Names.
    pub name: Vec<HumanName>,
    /// Contact channels.
    pub telecom: Vec<ContactPoint>,
    /// Administrative gender.
    pub gender: Option<AdministrativeGender>,
    /// Date of birth.
    #[serde(rename = "birthDate")]
    pub birth_date: Option<Date>,
    /// Addresses.
    pub address: Vec<Address>,
    /// Emergency / next-of-kin contacts.
    pub contact: Vec<PatientContact>,
}

impl TryFrom<ExternalPatient> for Patient {
    type Error = super::ConversionError;

    fn try_from(ext: ExternalPatient) -> Result<Self, Self::Error> {
        // `Patient` has no FHIR-required fields, so this conversion is total.
        // Unknown inbound fields were already dropped by the lenient deserialize.
        Ok(Self {
            resource_type: PatientResourceType,
            id: ext.id,
            meta: None,
            text: None,
            identifier: ext.identifier,
            active: ext.active,
            name: ext.name,
            telecom: ext.telecom,
            gender: ext.gender,
            birth_date: ext.birth_date,
            address: ext.address,
            contact: ext.contact,
            extension: Vec::new(),
        })
    }
}
