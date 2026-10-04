//! FHIR R5 Resources (Sprint 2+).
//!
//! Sprint 2 implements the two least-dependent resources first (per the
//! Phase 1 plan's TDD order):
//!
//! 1. [`Patient`] — referenced by every other resource (`Encounter.subject`,
//!    `Observation.subject`, `Condition.subject`, ...). If `Patient` is wrong,
//!    everything downstream is wrong, so it is built and tested first.
//! 2. [`Encounter`] — second-most-referenced; depends only on `Patient` plus a
//!    stubbed `Practitioner` reference.
//!
//! ## Strict-out / lenient-in (per [ADR-006] Decision 4)
//!
//! Each resource ships as a pair:
//!
//! - The canonical strict type (e.g. [`Patient`]) — `deny_unknown_fields`,
//!   used for everything Asgard writes. Field drift fails to deserialize.
//! - An `External*` newtype (e.g. [`ExternalPatient`]) — lenient
//!   (`#[serde(default)]`, silently ignores fields we do not model), used only
//!   at the ingest boundary. Convert into the canonical type via [`TryFrom`].
//!
//! [ADR-006]: ../../../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md

/// Generates a zero-sized `resourceType` discriminator marker type.
///
/// FHIR JSON tags every resource with a `"resourceType"` string. We model it
/// as a zero-sized type that (de)serializes to/from exactly one literal and
/// rejects any other value on the wire — so a `Patient` can never silently
/// deserialize from an `Encounter` payload. One marker per resource keeps the
/// discriminator type-safe without a runtime `String` field that could drift.
macro_rules! resource_type_marker {
    ($name:ident, $lit:literal) => {
        #[doc = concat!("Zero-sized `resourceType: \"", $lit, "\"` discriminator marker.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str($lit)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                if s == $lit {
                    Ok($name)
                } else {
                    Err(serde::de::Error::custom(format!(
                        concat!("expected resourceType \"", $lit, "\", got {:?}"),
                        s
                    )))
                }
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> String {
                concat!($lit, "ResourceType").to_string()
            }

            fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
                schemars::schema::SchemaObject {
                    instance_type: Some(schemars::schema::SingleOrVec::Single(Box::new(
                        schemars::schema::InstanceType::String,
                    ))),
                    enum_values: Some(vec![serde_json::Value::String($lit.to_string())]),
                    ..Default::default()
                }
                .into()
            }
        }
    };
}

// `resource_type_marker!` is in textual macro scope for the submodules below
// (declared after this definition), so each can invoke it directly.

mod admin;
mod diagnostics;
mod documents;
mod encounter;
mod patient;
mod provenance;
mod workflow;

pub use admin::{
    ExternalOrganization, ExternalPractitioner, Organization, Practitioner,
    PractitionerQualification, PractitionerRole,
};
pub use diagnostics::{
    DiagnosticReport, DiagnosticReportStatus, MultipleValuesError, Observation,
    ObservationComponent, ObservationStatus,
};
pub use documents::{
    Binary, Device, DeviceName, DeviceNameType, DeviceStatus, DeviceVersion, DocumentReference,
    DocumentReferenceContent, DocumentReferenceRelatesTo, DocumentReferenceStatus,
};
pub use provenance::{
    AuditEvent, AuditEventAction, AuditEventAgent, AuditEventEntity, AuditEventOutcome,
    AuditEventSource, Consent, ConsentDecision, ConsentProvision, ConsentStatus, Provenance,
    ProvenanceAgent, ProvenanceEntity, ProvenanceEntityRole, DATA_OPERATION_SYSTEM,
    PARTICIPATION_TYPE_SYSTEM,
};
pub use workflow::{
    ExternalServiceRequest, RequestIntent, RequestPriority, RequestStatus, ServiceRequest, Task,
    TaskIntent, TaskStatus,
};

pub use encounter::{
    Encounter, EncounterAdmission, EncounterDiagnosis, EncounterLocation, EncounterStatus,
    ExternalEncounter,
};
pub use patient::{AdministrativeGender, ExternalPatient, Patient, PatientContact};

/// Error raised when converting an `External*` ingest type into its canonical
/// resource (the lenient-in boundary, per [ADR-006] Decision 4).
///
/// `Patient` conversion is total (FHIR `Patient` has no required fields), so it
/// never produces this. `Encounter` conversion can fail because FHIR R5
/// `Encounter.status` is `1..1` (required) — a missing status yields
/// [`ConversionError::MissingRequiredField`].
///
/// [ADR-006]: ../../../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversionError {
    /// A field the canonical type requires was absent in the external input.
    #[error("{resource}.{field} is required by FHIR R5 but was missing in external input")]
    MissingRequiredField {
        /// The resource type being constructed (e.g. `"Encounter"`).
        resource: &'static str,
        /// The required field that was missing (e.g. `"status"`).
        field: &'static str,
    },
}

/// Common surface every canonical resource exposes, so generic code (the
/// store, REST layer, Bundle handling) can work over any resource type.
pub trait FhirResource:
    serde::Serialize + serde::de::DeserializeOwned + Clone + std::fmt::Debug
{
    /// The FHIR `resourceType` literal.
    const RESOURCE_TYPE: &'static str;
    /// Logical id, if assigned.
    fn id(&self) -> Option<&crate::datatypes::Id>;
    /// Assign the logical id.
    fn set_id(&mut self, id: crate::datatypes::Id);
    /// Resource metadata, if any.
    fn meta(&self) -> Option<&crate::datatypes::Meta>;
    /// Mutable resource metadata (created on demand by callers).
    fn meta_mut(&mut self) -> &mut Option<crate::datatypes::Meta>;
}

macro_rules! impl_fhir_resource {
    ($($t:ty => $lit:literal),+ $(,)?) => {$(
        impl FhirResource for $t {
            const RESOURCE_TYPE: &'static str = $lit;
            fn id(&self) -> Option<&crate::datatypes::Id> { self.id.as_ref() }
            fn set_id(&mut self, id: crate::datatypes::Id) { self.id = Some(id); }
            fn meta(&self) -> Option<&crate::datatypes::Meta> { self.meta.as_ref() }
            fn meta_mut(&mut self) -> &mut Option<crate::datatypes::Meta> { &mut self.meta }
        }
    )+};
}

impl_fhir_resource!(
    Patient => "Patient",
    Encounter => "Encounter",
    Organization => "Organization",
    Practitioner => "Practitioner",
    PractitionerRole => "PractitionerRole",
    ServiceRequest => "ServiceRequest",
    Task => "Task",
    Observation => "Observation",
    DiagnosticReport => "DiagnosticReport",
    Binary => "Binary",
    DocumentReference => "DocumentReference",
    Device => "Device",
    Provenance => "Provenance",
    AuditEvent => "AuditEvent",
    Consent => "Consent",
);
