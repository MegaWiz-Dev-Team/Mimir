//! FHIR R5 workflow resources: `ServiceRequest` (the order) and `Task` (the
//! worklist item a clinician acts on).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    Annotation, CodeableConcept, CodeableReference, DateTime, Id, Identifier, Meta, Narrative,
    Period, Reference,
};
use crate::resources::ConversionError;

resource_type_marker!(ServiceRequestResourceType, "ServiceRequest");
resource_type_marker!(TaskResourceType, "Task");

/// `ServiceRequest.status` / request-status value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RequestStatus {
    Draft,
    Active,
    OnHold,
    Revoked,
    Completed,
    EnteredInError,
    Unknown,
}

/// `ServiceRequest.intent` / request-intent value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RequestIntent {
    Proposal,
    Plan,
    Directive,
    Order,
    OriginalOrder,
    ReflexOrder,
    FillerOrder,
    InstanceOrder,
    Option,
}

/// Request priority value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RequestPriority {
    Routine,
    Urgent,
    Asap,
    Stat,
}

/// FHIR R5 `ServiceRequest` (<http://hl7.org/fhir/R5/servicerequest.html>).
///
/// Required: `status`, `intent`, `subject` (R5 made subject `1..1`). `code` is an
/// R5 `CodeableReference` (R4 was `CodeableConcept`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServiceRequest {
    /// Always `"ServiceRequest"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: ServiceRequestResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Order numbers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Order state (required).
    pub status: RequestStatus,
    /// Proposal / plan / order (required).
    pub intent: RequestIntent,
    /// Classification.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    /// Urgency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<RequestPriority>,
    /// What is requested (e.g. LOINC 28633-6 sleep study).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableReference>,
    /// The patient (required in R5).
    pub subject: Reference,
    /// Encounter the order was placed in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    /// When the service should happen.
    #[serde(rename = "occurrenceDateTime", skip_serializing_if = "Option::is_none")]
    pub occurrence_date_time: Option<DateTime>,
    /// When the order was signed.
    #[serde(rename = "authoredOn", skip_serializing_if = "Option::is_none")]
    pub authored_on: Option<DateTime>,
    /// Who ordered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester: Option<Reference>,
    /// Requested performers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<Reference>,
    /// Indications.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason: Vec<CodeableReference>,
    /// Comments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
}

impl ServiceRequest {
    /// An active order for `subject`.
    #[must_use]
    pub fn order(subject: Reference) -> Self {
        Self {
            resource_type: ServiceRequestResourceType,
            id: None,
            meta: None,
            text: None,
            identifier: Vec::new(),
            status: RequestStatus::Active,
            intent: RequestIntent::Order,
            category: Vec::new(),
            priority: None,
            code: None,
            subject,
            encounter: None,
            occurrence_date_time: None,
            authored_on: None,
            requester: None,
            performer: Vec::new(),
            reason: Vec::new(),
            note: Vec::new(),
        }
    }

    /// Set what is requested.
    #[must_use]
    pub fn with_code(mut self, code: CodeableConcept) -> Self {
        self.code = Some(CodeableReference::concept(code));
        self
    }
}

/// Lenient ingest counterpart of [`ServiceRequest`] — orders arriving from a HIS.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExternalServiceRequest {
    /// Logical id.
    pub id: Option<Id>,
    /// Identifiers.
    pub identifier: Vec<Identifier>,
    /// Status (required by canonical).
    pub status: Option<RequestStatus>,
    /// Intent (required by canonical).
    pub intent: Option<RequestIntent>,
    /// Category.
    pub category: Vec<CodeableConcept>,
    /// Priority.
    pub priority: Option<RequestPriority>,
    /// Code.
    pub code: Option<CodeableReference>,
    /// Subject (required by canonical).
    pub subject: Option<Reference>,
    /// Encounter.
    pub encounter: Option<Reference>,
    /// Occurrence.
    #[serde(rename = "occurrenceDateTime")]
    pub occurrence_date_time: Option<DateTime>,
    /// Authored on.
    #[serde(rename = "authoredOn")]
    pub authored_on: Option<DateTime>,
    /// Requester.
    pub requester: Option<Reference>,
    /// Performers.
    pub performer: Vec<Reference>,
    /// Reasons.
    pub reason: Vec<CodeableReference>,
    /// Notes.
    pub note: Vec<Annotation>,
}

impl TryFrom<ExternalServiceRequest> for ServiceRequest {
    type Error = ConversionError;

    fn try_from(e: ExternalServiceRequest) -> Result<Self, Self::Error> {
        let missing = |field| ConversionError::MissingRequiredField {
            resource: "ServiceRequest",
            field,
        };
        Ok(Self {
            resource_type: ServiceRequestResourceType,
            id: e.id,
            meta: None,
            text: None,
            identifier: e.identifier,
            status: e.status.ok_or_else(|| missing("status"))?,
            intent: e.intent.ok_or_else(|| missing("intent"))?,
            category: e.category,
            priority: e.priority,
            code: e.code,
            subject: e.subject.ok_or_else(|| missing("subject"))?,
            encounter: e.encounter,
            occurrence_date_time: e.occurrence_date_time,
            authored_on: e.authored_on,
            requester: e.requester,
            performer: e.performer,
            reason: e.reason,
            note: e.note,
        })
    }
}

/// `Task.status` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Draft,
    Requested,
    Received,
    Accepted,
    Rejected,
    Ready,
    Cancelled,
    InProgress,
    OnHold,
    Failed,
    Completed,
    EnteredInError,
}

/// `Task.intent` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TaskIntent {
    Unknown,
    Proposal,
    Plan,
    Order,
    OriginalOrder,
    ReflexOrder,
    FillerOrder,
    InstanceOrder,
    Option,
}

/// FHIR R5 `Task` (<http://hl7.org/fhir/R5/task.html>) — one worklist item.
///
/// Required: `status`, `intent`. Worklist query = `Task?owner=…&status=…`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Always `"Task"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: TaskResourceType,
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
    /// Request this task fulfils.
    #[serde(rename = "basedOn", default, skip_serializing_if = "Vec::is_empty")]
    pub based_on: Vec<Reference>,
    /// Task state (required).
    pub status: TaskStatus,
    /// Intent (required).
    pub intent: TaskIntent,
    /// Urgency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<RequestPriority>,
    /// Task type (e.g. "review and sign sleep study").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    /// Human-readable explanation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What the task acts on (e.g. the `DiagnosticReport`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<Reference>,
    /// Beneficiary (the patient) — FHIR element name `for`.
    #[serde(rename = "for", skip_serializing_if = "Option::is_none")]
    pub for_: Option<Reference>,
    /// Encounter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    /// When the task was created.
    #[serde(rename = "authoredOn", skip_serializing_if = "Option::is_none")]
    pub authored_on: Option<DateTime>,
    /// Last modification.
    #[serde(rename = "lastModified", skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<DateTime>,
    /// Start/end of execution.
    #[serde(rename = "executionPeriod", skip_serializing_if = "Option::is_none")]
    pub execution_period: Option<Period>,
    /// Who created it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester: Option<Reference>,
    /// Who is responsible (the reading physician's `PractitionerRole`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Reference>,
    /// Comments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
}

impl Task {
    /// A requested task about `focus` for patient `for_`.
    #[must_use]
    pub fn requested(focus: Reference, for_: Reference) -> Self {
        Self {
            resource_type: TaskResourceType,
            id: None,
            meta: None,
            text: None,
            identifier: Vec::new(),
            based_on: Vec::new(),
            status: TaskStatus::Requested,
            intent: TaskIntent::Order,
            priority: None,
            code: None,
            description: None,
            focus: Some(focus),
            for_: Some(for_),
            encounter: None,
            authored_on: None,
            last_modified: None,
            execution_period: None,
            requester: None,
            owner: None,
            note: Vec::new(),
        }
    }

    /// Assign to an owner.
    #[must_use]
    pub fn owned_by(mut self, owner: Reference) -> Self {
        self.owner = Some(owner);
        self
    }
}
