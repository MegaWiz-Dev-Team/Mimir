//! FHIR R5 administrative resources: `Organization`, `Practitioner`,
//! `PractitionerRole` (who works where, in which role — the basis for
//! "who may sign" in a clinical workflow).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    CodeableConcept, ContactPoint, Date, HumanName, Id, Identifier, Meta, Narrative, Period,
    Reference,
};
use crate::resources::AdministrativeGender;

resource_type_marker!(OrganizationResourceType, "Organization");
resource_type_marker!(PractitionerResourceType, "Practitioner");
resource_type_marker!(PractitionerRoleResourceType, "PractitionerRole");

/// FHIR R5 `Organization` (<http://hl7.org/fhir/R5/organization.html>).
/// Scaffold subset: identity, type, name, hierarchy. No required fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Organization {
    /// Always `"Organization"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: OrganizationResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated (stored per ADR-006 Amendment 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Business identifiers (e.g. hospital code 5 digits).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Whether the record is in active use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Kind of organization.
    #[serde(rename = "type", default, skip_serializing_if = "Vec::is_empty")]
    pub type_: Vec<CodeableConcept>,
    /// Name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Parent organization.
    #[serde(rename = "partOf", skip_serializing_if = "Option::is_none")]
    pub part_of: Option<Reference>,
}

impl Organization {
    /// Named organization.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            active: Some(true),
            ..Self::default()
        }
    }
}

/// FHIR R5 `Practitioner.qualification` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PractitionerQualification {
    /// Licence / certificate numbers (e.g. Thai Medical Council licence).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// What the qualification is (required `1..1`).
    pub code: CodeableConcept,
    /// Validity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Issuing body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<Reference>,
}

/// FHIR R5 `Practitioner` (<http://hl7.org/fhir/R5/practitioner.html>).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Practitioner {
    /// Always `"Practitioner"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: PractitionerResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Business identifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Whether the record is in active use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Names (Thai + Latin).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<HumanName>,
    /// Contact details.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,
    /// Administrative gender.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<AdministrativeGender>,
    /// Date of birth.
    #[serde(rename = "birthDate", skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<Date>,
    /// Licences / certifications.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualification: Vec<PractitionerQualification>,
}

/// Lenient ingest counterpart of [`Practitioner`] (ADR-006 Decision 4) — HIS exports.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExternalPractitioner {
    /// Logical id.
    pub id: Option<Id>,
    /// Identifiers.
    pub identifier: Vec<Identifier>,
    /// Active flag.
    pub active: Option<bool>,
    /// Names.
    pub name: Vec<HumanName>,
    /// Contacts.
    pub telecom: Vec<ContactPoint>,
    /// Gender.
    pub gender: Option<AdministrativeGender>,
    /// Birth date.
    #[serde(rename = "birthDate")]
    pub birth_date: Option<Date>,
    /// Qualifications.
    pub qualification: Vec<PractitionerQualification>,
}

impl From<ExternalPractitioner> for Practitioner {
    fn from(e: ExternalPractitioner) -> Self {
        Self {
            id: e.id,
            identifier: e.identifier,
            active: e.active,
            name: e.name,
            telecom: e.telecom,
            gender: e.gender,
            birth_date: e.birth_date,
            qualification: e.qualification,
            ..Self::default()
        }
    }
}

/// Lenient ingest counterpart of [`Organization`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExternalOrganization {
    /// Logical id.
    pub id: Option<Id>,
    /// Identifiers.
    pub identifier: Vec<Identifier>,
    /// Active flag.
    pub active: Option<bool>,
    /// Kind.
    #[serde(rename = "type")]
    pub type_: Vec<CodeableConcept>,
    /// Name.
    pub name: Option<String>,
    /// Parent.
    #[serde(rename = "partOf")]
    pub part_of: Option<Reference>,
}

impl From<ExternalOrganization> for Organization {
    fn from(e: ExternalOrganization) -> Self {
        Self {
            id: e.id,
            identifier: e.identifier,
            active: e.active,
            type_: e.type_,
            name: e.name,
            part_of: e.part_of,
            ..Self::default()
        }
    }
}

/// FHIR R5 `PractitionerRole` (<http://hl7.org/fhir/R5/practitionerrole.html>).
///
/// The `code` carries the workflow role used for authorization in Nótt Local
/// (physician may sign; technician may import/edit; admin configures).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct PractitionerRole {
    /// Always `"PractitionerRole"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: PractitionerRoleResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Business identifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Whether the role is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// Period the role is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// The practitioner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub practitioner: Option<Reference>,
    /// Where the role is held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<Reference>,
    /// Roles (e.g. physician, sleep technologist, administrator).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code: Vec<CodeableConcept>,
    /// Specialties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub specialty: Vec<CodeableConcept>,
}
