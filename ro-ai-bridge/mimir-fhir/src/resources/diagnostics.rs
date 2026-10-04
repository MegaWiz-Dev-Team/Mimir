//! FHIR R5 diagnostic results: `Observation` (one index, e.g. AHI) and
//! `DiagnosticReport` (the study report that a physician finalizes).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    Annotation, Attachment, CodeableConcept, DateTime, Id, Identifier, Instant, Meta, Narrative,
    Period, Quantity, Reference,
};

resource_type_marker!(ObservationResourceType, "Observation");
resource_type_marker!(DiagnosticReportResourceType, "DiagnosticReport");

/// `Observation.status` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationStatus {
    Registered,
    Preliminary,
    Final,
    Amended,
    Corrected,
    Cancelled,
    EnteredInError,
    Unknown,
}

/// `Observation.component` backbone — a sub-measurement (e.g. supine AHI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservationComponent {
    /// What was measured (required).
    pub code: CodeableConcept,
    /// Numeric value.
    #[serde(rename = "valueQuantity", skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
    /// Coded value.
    #[serde(
        rename = "valueCodeableConcept",
        skip_serializing_if = "Option::is_none"
    )]
    pub value_codeable_concept: Option<CodeableConcept>,
    /// Text value.
    #[serde(rename = "valueString", skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    /// Boolean value.
    #[serde(rename = "valueBoolean", skip_serializing_if = "Option::is_none")]
    pub value_boolean: Option<bool>,
}

/// FHIR R5 `Observation` (<http://hl7.org/fhir/R5/observation.html>).
///
/// Required: `status`, `code`. `value[x]` is modelled as one optional field per
/// supported type; FHIR allows at most one — enforced by [`Observation::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Always `"Observation"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: ObservationResourceType,
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
    /// Order fulfilled.
    #[serde(rename = "basedOn", default, skip_serializing_if = "Vec::is_empty")]
    pub based_on: Vec<Reference>,
    /// Result state (required).
    pub status: ObservationStatus,
    /// Category.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    /// What was observed (required).
    pub code: CodeableConcept,
    /// Patient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    /// Encounter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    /// Clinically relevant time.
    #[serde(rename = "effectiveDateTime", skip_serializing_if = "Option::is_none")]
    pub effective_date_time: Option<DateTime>,
    /// Clinically relevant period (e.g. the recording night).
    #[serde(rename = "effectivePeriod", skip_serializing_if = "Option::is_none")]
    pub effective_period: Option<Period>,
    /// When this version was made available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued: Option<Instant>,
    /// Who/what is responsible (e.g. the analysis software `Device`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<Reference>,
    /// Numeric result.
    #[serde(rename = "valueQuantity", skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
    /// Coded result.
    #[serde(
        rename = "valueCodeableConcept",
        skip_serializing_if = "Option::is_none"
    )]
    pub value_codeable_concept: Option<CodeableConcept>,
    /// Text result.
    #[serde(rename = "valueString", skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    /// Boolean result.
    #[serde(rename = "valueBoolean", skip_serializing_if = "Option::is_none")]
    pub value_boolean: Option<bool>,
    /// High/low/normal flags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interpretation: Vec<CodeableConcept>,
    /// Comments (e.g. method / denominator note).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// How it was determined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<CodeableConcept>,
    /// Measuring device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<Reference>,
    /// Source data (e.g. the scored-events `DocumentReference`).
    #[serde(rename = "derivedFrom", default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<Reference>,
    /// Sub-measurements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component: Vec<ObservationComponent>,
}

/// More than one `value[x]` was set on an `Observation`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Observation has {0} value[x] fields set; FHIR allows at most one")]
pub struct MultipleValuesError(pub usize);

impl Observation {
    /// Observation with the required fields only.
    #[must_use]
    pub fn new(status: ObservationStatus, code: CodeableConcept) -> Self {
        Self {
            resource_type: ObservationResourceType,
            id: None,
            meta: None,
            text: None,
            identifier: Vec::new(),
            based_on: Vec::new(),
            status,
            category: Vec::new(),
            code,
            subject: None,
            encounter: None,
            effective_date_time: None,
            effective_period: None,
            issued: None,
            performer: Vec::new(),
            value_quantity: None,
            value_codeable_concept: None,
            value_string: None,
            value_boolean: None,
            interpretation: Vec::new(),
            note: Vec::new(),
            method: None,
            device: None,
            derived_from: Vec::new(),
            component: Vec::new(),
        }
    }

    /// Set a numeric value.
    #[must_use]
    pub fn with_quantity(mut self, q: Quantity) -> Self {
        self.value_quantity = Some(q);
        self
    }

    /// Set the subject.
    #[must_use]
    pub fn with_subject(mut self, subject: Reference) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Enforce the FHIR `value[x]` cardinality (`0..1`).
    ///
    /// # Errors
    /// [`MultipleValuesError`] when more than one value field is set.
    pub fn validate(&self) -> Result<(), MultipleValuesError> {
        let n = usize::from(self.value_quantity.is_some())
            + usize::from(self.value_codeable_concept.is_some())
            + usize::from(self.value_string.is_some())
            + usize::from(self.value_boolean.is_some());
        if n > 1 {
            Err(MultipleValuesError(n))
        } else {
            Ok(())
        }
    }
}

/// `DiagnosticReport.status` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticReportStatus {
    Registered,
    Partial,
    Preliminary,
    Modified,
    Final,
    Amended,
    Corrected,
    Appended,
    Cancelled,
    EnteredInError,
    Unknown,
}

/// FHIR R5 `DiagnosticReport` (<http://hl7.org/fhir/R5/diagnosticreport.html>).
///
/// Required: `status`, `code`. Lifecycle used by Nótt Local: automated result =
/// `preliminary` → physician sign-off = `final` (with `resultsInterpreter`) →
/// later change = `amended`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticReport {
    /// Always `"DiagnosticReport"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: DiagnosticReportResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Report numbers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Order fulfilled.
    #[serde(rename = "basedOn", default, skip_serializing_if = "Vec::is_empty")]
    pub based_on: Vec<Reference>,
    /// Report state (required).
    pub status: DiagnosticReportStatus,
    /// Service category.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    /// Report type (required; e.g. LOINC 28633-6).
    pub code: CodeableConcept,
    /// Patient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    /// Encounter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    /// Clinically relevant time.
    #[serde(rename = "effectiveDateTime", skip_serializing_if = "Option::is_none")]
    pub effective_date_time: Option<DateTime>,
    /// Clinically relevant period (the recording night).
    #[serde(rename = "effectivePeriod", skip_serializing_if = "Option::is_none")]
    pub effective_period: Option<Period>,
    /// When this version was released.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued: Option<Instant>,
    /// Responsible for the report (software `Device` for automated results).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<Reference>,
    /// Primary result interpreter — the signing physician.
    #[serde(
        rename = "resultsInterpreter",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub results_interpreter: Vec<Reference>,
    /// Observations in the report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result: Vec<Reference>,
    /// Comments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// Clinical conclusion text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    /// Coded conclusion (e.g. ICD-10-TM G47.3).
    #[serde(
        rename = "conclusionCode",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub conclusion_code: Vec<CodeableConcept>,
    /// Rendered report (e.g. the PDF).
    #[serde(
        rename = "presentedForm",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub presented_form: Vec<Attachment>,
}

impl DiagnosticReport {
    /// Report with the required fields only.
    #[must_use]
    pub fn new(status: DiagnosticReportStatus, code: CodeableConcept) -> Self {
        Self {
            resource_type: DiagnosticReportResourceType,
            id: None,
            meta: None,
            text: None,
            identifier: Vec::new(),
            based_on: Vec::new(),
            status,
            category: Vec::new(),
            code,
            subject: None,
            encounter: None,
            effective_date_time: None,
            effective_period: None,
            issued: None,
            performer: Vec::new(),
            results_interpreter: Vec::new(),
            result: Vec::new(),
            note: Vec::new(),
            conclusion: None,
            conclusion_code: Vec::new(),
            presented_form: Vec::new(),
        }
    }
}
