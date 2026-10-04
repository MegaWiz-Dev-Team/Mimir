//! FHIR R5 document resources: `Binary` (raw bytes), `DocumentReference`
//! (metadata + versioning of a document), `Device` (recording hardware or the
//! analysis software itself).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{
    Annotation, Attachment, Code, CodeableConcept, Id, Identifier, Instant, Markdown, Meta,
    Narrative, Reference,
};

resource_type_marker!(BinaryResourceType, "Binary");
resource_type_marker!(DocumentReferenceResourceType, "DocumentReference");
resource_type_marker!(DeviceResourceType, "Device");

/// FHIR R5 `Binary` (<http://hl7.org/fhir/R5/binary.html>).
///
/// A plain `Resource` (no narrative). `contentType` is required. Large payloads
/// (EDF, PDF) keep `data` empty and the bytes in the store's blob area; the
/// store serves them by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Binary {
    /// Always `"Binary"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: BinaryResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// MIME type (required).
    #[serde(rename = "contentType")]
    pub content_type: Code,
    /// Resource whose access rules apply to these bytes.
    #[serde(rename = "securityContext", skip_serializing_if = "Option::is_none")]
    pub security_context: Option<Reference>,
    /// Inline content, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl Binary {
    /// Binary metadata with bytes held outside the resource.
    #[must_use]
    pub fn external(content_type: Code) -> Self {
        Self {
            resource_type: BinaryResourceType,
            id: None,
            meta: None,
            content_type,
            security_context: None,
            data: None,
        }
    }
}

/// `DocumentReference.status` value set (R5 document-reference-status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentReferenceStatus {
    Current,
    Superseded,
    EnteredInError,
}

/// `DocumentReference.relatesTo` backbone — R5 types `code` as `CodeableConcept`
/// (`replaces`, `transforms`, `signs`, `appends`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentReferenceRelatesTo {
    /// Relationship (required).
    pub code: CodeableConcept,
    /// Target document (required).
    pub target: Reference,
}

/// `DocumentReference.content` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentReferenceContent {
    /// Where to access the document (required).
    pub attachment: Attachment,
}

/// FHIR R5 `DocumentReference` (<http://hl7.org/fhir/R5/documentreference.html>).
///
/// Required: `status`, `content` (`1..*`). In Nótt Local one `DocumentReference`
/// per version of a scored-event set; an edit creates a new one with
/// `relatesTo: replaces` → the previous version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentReference {
    /// Always `"DocumentReference"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: DocumentReferenceResourceType,
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
    /// Reference status (required).
    pub status: DocumentReferenceStatus,
    /// Kind of document (recording, scored-events, report).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<CodeableConcept>,
    /// Categorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    /// Who/what the document is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    /// Clinical context (encounter / request).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<Reference>,
    /// When this reference was created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<Instant>,
    /// Who authored the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub author: Vec<Reference>,
    /// Relationships to other documents.
    #[serde(rename = "relatesTo", default, skip_serializing_if = "Vec::is_empty")]
    pub relates_to: Vec<DocumentReferenceRelatesTo>,
    /// Human description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Markdown>,
    /// The document content (required, `1..*`).
    pub content: Vec<DocumentReferenceContent>,
}

impl DocumentReference {
    /// A current document with one attachment.
    #[must_use]
    pub fn current(attachment: Attachment) -> Self {
        Self {
            resource_type: DocumentReferenceResourceType,
            id: None,
            meta: None,
            text: None,
            identifier: Vec::new(),
            based_on: Vec::new(),
            status: DocumentReferenceStatus::Current,
            type_: None,
            category: Vec::new(),
            subject: None,
            context: Vec::new(),
            date: None,
            author: Vec::new(),
            relates_to: Vec::new(),
            description: None,
            content: vec![DocumentReferenceContent { attachment }],
        }
    }
}

/// `Device.status` value set (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceStatus {
    Active,
    Inactive,
    EnteredInError,
}

/// `Device.name.type` value set (R5 device-nametype).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceNameType {
    RegisteredName,
    UserFriendlyName,
    PatientReportedName,
}

/// `Device.name` backbone (R5 rename of R4 `deviceName`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceName {
    /// The name (required).
    pub value: String,
    /// Name type (required).
    #[serde(rename = "type")]
    pub type_: DeviceNameType,
}

/// `Device.version` backbone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceVersion {
    /// Which version this is (e.g. `engine`, `build`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<CodeableConcept>,
    /// The version text (required).
    pub value: String,
}

/// FHIR R5 `Device` (<http://hl7.org/fhir/R5/device.html>) — no required fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Device {
    /// Always `"Device"`.
    #[serde(rename = "resourceType", default)]
    pub resource_type: DeviceResourceType,
    /// Logical id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    /// Version / last-updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Narrative.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Narrative>,
    /// Identifiers (serial, UDI-DI …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    /// Display name.
    #[serde(rename = "displayName", skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Record status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DeviceStatus>,
    /// Manufacturer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Model number.
    #[serde(rename = "modelNumber", skip_serializing_if = "Option::is_none")]
    pub model_number: Option<String>,
    /// Serial number.
    #[serde(rename = "serialNumber", skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    /// Names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<DeviceName>,
    /// Versions (software engine / build).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub version: Vec<DeviceVersion>,
    /// Kind of device.
    #[serde(rename = "type", default, skip_serializing_if = "Vec::is_empty")]
    pub type_: Vec<CodeableConcept>,
    /// Responsible organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Reference>,
    /// Comments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
}
