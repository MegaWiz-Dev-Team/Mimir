//! FHIR R5 accountability resources: `Provenance` (who changed/signed what),
//! `AuditEvent` (who accessed what), `Consent` (what the patient permitted).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    CodeableConcept, Coding, Date, DateTime, Id, Identifier, Instant, Meta, Narrative, Period,
    Reference, Signature, Uri,
};

resource_type_marker!(ProvenanceResourceType, "Provenance");
resource_type_marker!(AuditEventResourceType, "AuditEvent");
resource_type_marker!(ConsentResourceType, "Consent");

/// `Provenance.agent` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProvenanceAgent {
    /// How the agent participated (author, verifier, …).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<CodeableConcept>,
    /// Roles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role: Vec<CodeableConcept>,
    /// The agent (required).
    pub who: Reference,
    /// On whose behalf.
    #[serde(rename = "onBehalfOf", skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<Reference>,
}

/// `Provenance.entity.role` value set (R5 provenance-entity-role).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceEntityRole {
    Revision,
    Quotation,
    Source,
    Instantiates,
    Removal,
}

/// `Provenance.entity` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProvenanceEntity {
    /// How the entity was used (required).
    pub role: ProvenanceEntityRole,
    /// Identity of the entity, typically a versioned reference (required).
    pub what: Reference,
}

/// FHIR R5 `Provenance` (<http://hl7.org/fhir/R5/provenance.html>).
///
/// Required: `target` (`1..*`), `agent` (`1..*`). Point `target` at a versioned
/// reference (`DiagnosticReport/x/_history/3`) so the signed version is exact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Always `"Provenance"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: ProvenanceResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// What was produced / changed / signed (required).
    pub target: Vec<Reference>,
    /// When the activity occurred.
    #[serde(rename = "occurredDateTime", skip_serializing_if = "Option::is_none")]
    pub occurred_date_time: Option<DateTime>,
    /// When recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded: Option<Instant>,
    /// Policies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy: Vec<Uri>,
    /// Activity (e.g. `revise`, `attest`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<CodeableConcept>,
    /// Patient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient: Option<Reference>,
    /// Who participated (required).
    pub agent: Vec<ProvenanceAgent>,
    /// Inputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity: Vec<ProvenanceEntity>,
    /// Signatures (physician sign-off).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signature: Vec<Signature>,
}

/// HL7 v3 `DataOperation` code system — `Provenance.activity` codes.
pub const DATA_OPERATION_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-DataOperation";
/// HL7 v3 `ParticipationType` — `attest` / verifier activity.
pub const PARTICIPATION_TYPE_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/v3-ParticipationType";

impl Provenance {
    /// Provenance of `target` by `who`.
    #[must_use]
    pub fn of(target: Reference, who: Reference) -> Self {
        Self {
            resource_type: ProvenanceResourceType,
            id: None,
            meta: None,
            text: None,
            target: vec![target],
            occurred_date_time: None,
            recorded: None,
            policy: Vec::new(),
            activity: None,
            patient: None,
            agent: vec![ProvenanceAgent {
                type_: None,
                role: Vec::new(),
                who,
                on_behalf_of: None,
            }],
            entity: Vec::new(),
            signature: Vec::new(),
        }
    }

    /// Set the activity (e.g. `v3-DataOperation#UPDATE`, `v3-ParticipationType#VRF`).
    #[must_use]
    pub fn with_activity(mut self, activity: Coding) -> Self {
        self.activity = Some(CodeableConcept::from_coding(activity));
        self
    }
}

/// `AuditEvent.action` value set (R5 audit-event-action).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum AuditEventAction {
    /// Create.
    C,
    /// Read / view.
    R,
    /// Update.
    U,
    /// Delete.
    D,
    /// Execute.
    E,
}

/// `AuditEvent.outcome` backbone (R5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuditEventOutcome {
    /// Outcome code (required).
    pub code: Coding,
    /// Detail.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detail: Vec<CodeableConcept>,
}

/// `AuditEvent.agent` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuditEventAgent {
    /// Agent type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<CodeableConcept>,
    /// Roles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub role: Vec<CodeableConcept>,
    /// Identifier of who (required).
    pub who: Reference,
    /// Whether the agent initiated the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestor: Option<bool>,
}

/// `AuditEvent.source` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuditEventSource {
    /// The system that detected and recorded the event (required).
    pub observer: Reference,
}

/// `AuditEvent.entity` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct AuditEventEntity {
    /// Specific instance of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub what: Option<Reference>,
    /// Role of the entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<CodeableConcept>,
}

/// FHIR R5 `AuditEvent` (<http://hl7.org/fhir/R5/auditevent.html>).
///
/// Required: `code`, `recorded`, `agent` (`1..*`), `source`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditEvent {
    /// Always `"AuditEvent"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: AuditEventResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Category.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    /// Specific event (required).
    pub code: CodeableConcept,
    /// C/R/U/D/E.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<AuditEventAction>,
    /// When the event occurred.
    #[serde(rename = "occurredDateTime", skip_serializing_if = "Option::is_none")]
    pub occurred_date_time: Option<DateTime>,
    /// When recorded (required).
    pub recorded: Instant,
    /// Outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<AuditEventOutcome>,
    /// Patient involved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient: Option<Reference>,
    /// Actors (required).
    pub agent: Vec<AuditEventAgent>,
    /// Recording system (required).
    pub source: AuditEventSource,
    /// Data involved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity: Vec<AuditEventEntity>,
}

/// `Consent.status` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConsentStatus {
    Draft,
    Active,
    Inactive,
    NotDone,
    EnteredInError,
    Unknown,
}

/// `Consent.decision` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConsentDecision {
    Deny,
    Permit,
}

/// `Consent.provision` backbone (scaffold subset).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct ConsentProvision {
    /// Timeframe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Actions controlled (e.g. access, use, disclose).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action: Vec<CodeableConcept>,
    /// Purposes of use (e.g. treatment, research/analytics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub purpose: Vec<Coding>,
}

/// FHIR R5 `Consent` (<http://hl7.org/fhir/R5/consent.html>). Required: `status`.
///
/// Replaces the free-text consent string of the Nótt API: clinical vs
/// analytics scope become `provision.purpose` codings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Consent {
    /// Always `"Consent"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: ConsentResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Identifiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// State (required).
    pub status: ConsentStatus,
    /// Kind of consent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    /// Patient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    /// When consent was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
    /// Effective period.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Who is granted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grantee: Vec<Reference>,
    /// Base decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<ConsentDecision>,
    /// Rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provision: Vec<ConsentProvision>,
}
