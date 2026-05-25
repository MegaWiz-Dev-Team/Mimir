//! FHIR R5 complex datatypes (Sprint 1 Day 3-5).
//!
//! Implemented: `Coding`, `CodeableConcept`, `Identifier`, `IdentifierUse`,
//! `Reference`, `Period`, `ContactPoint`, `ContactPointSystem`,
//! `ContactPointUse`, `Extension` (minimal — `valueString` variant only;
//! full `value[x]` polymorphism deferred to Sprint 1 Day 7).

use serde::{Deserialize, Serialize};

use crate::datatypes::{Code, DateTime, Uri};

// =============================================================================
// Coding — FHIR R5 Coding type
// =============================================================================

/// FHIR R5 `Coding` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Coding>):
/// A reference to a code defined by a terminology system. Every field is
/// optional at the wire level, but in practice `system` + `code` is the
/// minimum useful payload.
///
/// Example LOINC vital-sign code:
///
/// ```ignore
/// Coding {
///     system: Some(Uri::new("http://loinc.org").unwrap()),
///     code: Some(Code::new("8480-6").unwrap()),
///     display: Some("Systolic blood pressure".into()),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Coding {
    /// Identity of the terminology system — typically a canonical URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Uri>,

    /// Version of the system — if relevant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Symbol in syntax defined by the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Code>,

    /// Representation defined by the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,

    /// True if this coding was chosen directly by the user.
    #[serde(rename = "userSelected", skip_serializing_if = "Option::is_none")]
    pub user_selected: Option<bool>,
}

impl Coding {
    /// Convenience constructor for the most common shape:
    /// system + code, no display, no version.
    #[must_use]
    pub fn new(system: Uri, code: Code) -> Self {
        Self {
            system: Some(system),
            code: Some(code),
            ..Self::default()
        }
    }

    /// Add a human-readable display to an existing coding.
    #[must_use]
    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = Some(display.into());
        self
    }
}

// =============================================================================
// CodeableConcept — FHIR R5 CodeableConcept type
// =============================================================================

/// FHIR R5 `CodeableConcept` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#CodeableConcept>):
/// A concept that may be defined by a formal reference to a terminology
/// (one or more `Coding`s) AND/OR by a plain text string. The two are
/// not mutually exclusive — a concept can be both coded and described
/// in free text for human readability.
///
/// Example single-coding concept:
///
/// ```ignore
/// CodeableConcept {
///     coding: vec![Coding::new(loinc_url, vital_sign_code)],
///     text: Some("Systolic BP".into()),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct CodeableConcept {
    /// Code(s) defined by terminology systems. Multiple codings here
    /// represent translations of the SAME concept across systems
    /// (e.g. ICD-10 and SNOMED for the same diagnosis).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coding: Vec<Coding>,

    /// Plain-text representation — for human reading; never used for
    /// computable comparisons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl CodeableConcept {
    /// Construct a `CodeableConcept` from a single `Coding`.
    #[must_use]
    pub fn from_coding(coding: Coding) -> Self {
        Self {
            coding: vec![coding],
            text: None,
        }
    }

    /// Construct a text-only `CodeableConcept` (no coded representation).
    #[must_use]
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            coding: Vec::new(),
            text: Some(text.into()),
        }
    }

    /// Add free-text alongside existing codings.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
}

// =============================================================================
// IdentifierUse — enum for Identifier.use field
// =============================================================================

/// FHIR R5 `IdentifierUse` value set (`Identifier.use` field).
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-identifier-use.html>):
/// Distinguishes the purpose of an identifier (e.g., whether it's the
/// primary "official" id, a temporary one, or an old retained value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentifierUse {
    /// Recommended for display to user (e.g., the id the patient gives
    /// when asked).
    Usual,
    /// Considered to be the "main" identifier (e.g., national citizen ID).
    Official,
    /// A temporary identifier (e.g., issued at registration before a
    /// permanent one assigned).
    Temp,
    /// An identifier used for secondary purposes (e.g., a number that
    /// goes on the door of the patient's chart in the hospital).
    Secondary,
    /// The identifier id no longer considered valid but might be relevant
    /// for searching historical records.
    Old,
}

// =============================================================================
// Identifier — FHIR R5 Identifier type
// =============================================================================

/// FHIR R5 `Identifier` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Identifier>):
/// A numeric or alphanumeric string associated with a single object or
/// entity within a defined system. Distinct from FHIR `Id` (which is the
/// internal logical id of a resource); `Identifier` represents
/// business identifiers like national IDs, hospital MRNs, insurance
/// policy numbers, etc.
///
/// Thai citizen ID convention (used by `Patient.identifier` per MOPH-PC1):
///
/// ```ignore
/// Identifier {
///     use_: Some(IdentifierUse::Official),
///     system: Some(Uri::new("https://fhir.moph.go.th/identifier/citizen-id").unwrap()),
///     value: Some("1234567890123".into()),
///     ..Default::default()
/// }
/// ```
///
/// Fields `period` and `assigner` are deferred to Day 4/5 of Sprint 1
/// (they depend on `Period` and `Reference` types). Both are `0..1`
/// optional in the spec, so absence is FHIR-conformant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Identifier {
    /// The purpose of this identifier.
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_: Option<IdentifierUse>,

    /// Description of identifier — e.g. "Medical Record Number".
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<CodeableConcept>,

    /// The namespace for the identifier value (canonical URI).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Uri>,

    /// The value that is unique within the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Time period when id is/was valid for use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,

    /// Organization (or other entity) that issued the identifier.
    ///
    /// `Box`ed because `Reference` can contain `Identifier`, which would
    /// otherwise create an infinitely-sized struct.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigner: Option<Box<Reference>>,
}

impl Identifier {
    /// Convenience constructor for the most common shape:
    /// system + value (e.g. national ID, MRN).
    #[must_use]
    pub fn new(system: Uri, value: impl Into<String>) -> Self {
        Self {
            system: Some(system),
            value: Some(value.into()),
            ..Self::default()
        }
    }

    /// Mark this identifier as the official one.
    #[must_use]
    pub fn official(mut self) -> Self {
        self.use_ = Some(IdentifierUse::Official);
        self
    }

    /// Mark this identifier as usual/displayable.
    #[must_use]
    pub fn usual(mut self) -> Self {
        self.use_ = Some(IdentifierUse::Usual);
        self
    }

    /// Attach a type `CodeableConcept` (e.g. "MR" Medical Record Number).
    #[must_use]
    pub fn with_type(mut self, type_: CodeableConcept) -> Self {
        self.type_ = Some(type_);
        self
    }

    /// Attach the issuing organization (or other entity) as a Reference.
    #[must_use]
    pub fn with_assigner(mut self, assigner: Reference) -> Self {
        self.assigner = Some(Box::new(assigner));
        self
    }

    /// Attach a validity period.
    #[must_use]
    pub fn with_period(mut self, period: Period) -> Self {
        self.period = Some(period);
        self
    }
}

// =============================================================================
// Period — FHIR R5 Period type
// =============================================================================

/// FHIR R5 `Period` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Period>):
/// A time period defined by start and/or end `dateTime`. Either bound may
/// be absent (open-ended period). Used for validity windows on Identifier,
/// `HumanName`, `Address`, `ContactPoint`, Encounter periods, etc.
///
/// FHIR rule: if both `start` and `end` are present, `end` SHALL be after
/// `start`. We do not enforce this at construction (the spec also allows
/// equality for "instant in time" periods); validation is best done at
/// the profile/resource layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Period {
    /// Starting time with inclusive boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<DateTime>,

    /// End time with inclusive boundary, if not ongoing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime>,
}

impl Period {
    /// Construct a period with a known start, open-ended (no end).
    #[must_use]
    pub fn starting(start: DateTime) -> Self {
        Self {
            start: Some(start),
            end: None,
        }
    }

    /// Construct a closed period with both start and end set.
    #[must_use]
    pub fn between(start: DateTime, end: DateTime) -> Self {
        Self {
            start: Some(start),
            end: Some(end),
        }
    }
}

// =============================================================================
// ContactPoint — FHIR R5 ContactPoint type
// =============================================================================

/// FHIR R5 `ContactPoint.system` enum.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-contact-point-system.html>):
/// What kind of contact channel this is (phone, email, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContactPointSystem {
    Phone,
    Fax,
    Email,
    Pager,
    Url,
    Sms,
    Other,
}

/// FHIR R5 `ContactPoint.use` enum.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-contact-point-use.html>):
/// The purpose of a contact point (home, work, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContactPointUse {
    Home,
    Work,
    Temp,
    Old,
    Mobile,
}

/// FHIR R5 `ContactPoint` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#ContactPoint>):
/// Details for technology-mediated contact — phone, email, etc. Used on
/// Patient (telecom), Practitioner (telecom), Organization (telecom).
///
/// Example Thai mobile phone:
///
/// ```ignore
/// ContactPoint {
///     system: Some(ContactPointSystem::Phone),
///     value: Some("+66-2-123-4567".into()),
///     use_: Some(ContactPointUse::Mobile),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ContactPoint {
    /// Telecommunications form: phone | fax | email | ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<ContactPointSystem>,

    /// The actual contact point details (phone number, email address, URL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// home | work | temp | old | mobile.
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_: Option<ContactPointUse>,

    /// Specifies preferred order (1 = highest priority).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,

    /// Time period when the contact point was/is in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
}

impl ContactPoint {
    /// Construct a phone contact point.
    #[must_use]
    pub fn phone(value: impl Into<String>) -> Self {
        Self {
            system: Some(ContactPointSystem::Phone),
            value: Some(value.into()),
            ..Self::default()
        }
    }

    /// Construct an email contact point.
    #[must_use]
    pub fn email(value: impl Into<String>) -> Self {
        Self {
            system: Some(ContactPointSystem::Email),
            value: Some(value.into()),
            ..Self::default()
        }
    }

    /// Set the use (home/work/mobile/etc.).
    #[must_use]
    pub fn with_use(mut self, use_: ContactPointUse) -> Self {
        self.use_ = Some(use_);
        self
    }

    /// Set preferred-order rank.
    #[must_use]
    pub fn with_rank(mut self, rank: u32) -> Self {
        self.rank = Some(rank);
        self
    }
}

// =============================================================================
// Extension — FHIR R5 Extension (minimal — Day 5 scope)
// =============================================================================

/// FHIR R5 `Extension` datatype — Day 5 minimal subset.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/extensibility.html#Extension>):
/// Extension carries a `url` (canonical extension definition) plus exactly
/// one `value[x]` (50+ polymorphic variants) OR nested sub-extensions.
///
/// **Day 5 scope:** support `valueString` only — sufficient for the Thai
/// address sub-district extension. Full `value[x]` polymorphism is
/// deferred to Sprint 1 Day 7 when more datatypes (Quantity, Date, ...)
/// are available.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Extension {
    /// Canonical URL identifying the extension definition.
    pub url: Uri,

    /// The string value of the extension (Day 5 subset of `value[x]`).
    #[serde(rename = "valueString", skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
}

impl Extension {
    /// Construct an extension with a string value.
    #[must_use]
    pub fn string(url: Uri, value: impl Into<String>) -> Self {
        Self {
            url,
            value_string: Some(value.into()),
        }
    }
}

// =============================================================================
// Reference — FHIR R5 Reference type
// =============================================================================

/// FHIR R5 `Reference` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/references.html#Reference>):
/// A reference from one resource to another. Three forms:
///
/// 1. **Literal** — `reference: "Patient/A12345"` or absolute URL
/// 2. **Logical** — `identifier: <business id of the target resource>`
/// 3. **Contained** — `reference: "#contained-1"` (within Bundle/resource)
///
/// All fields are optional; in practice a usable Reference needs either
/// `reference` (literal) or `identifier` (logical).
///
/// Note on Rust cycles: `Reference` contains `Identifier` (logical form),
/// and `Identifier` contains `Reference` (assigner). The cycle is broken
/// by `Box`ing `Identifier.assigner` (not `Reference.identifier`) — the
/// boxing happens on the rarer field to minimize heap allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Reference {
    /// Literal reference, relative or absolute URL.
    /// Examples: `"Patient/A12345"`, `"https://other.example.com/Patient/X"`,
    /// `"#contained-org-1"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,

    /// Resource type the reference refers to (canonical URL of the type).
    /// Useful when reference is logical-only (identifier-based) so consumer
    /// knows what kind of resource to look up.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<Uri>,

    /// Logical reference via business identifier (not Rust `Box` — the
    /// cycle is broken on the `Identifier.assigner` side).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<Identifier>,

    /// Text alternative for the resource (display name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

impl Reference {
    /// Construct a literal reference (e.g. `Patient/A12345`).
    #[must_use]
    pub fn literal(reference: impl Into<String>) -> Self {
        Self {
            reference: Some(reference.into()),
            ..Self::default()
        }
    }

    /// Construct a logical reference from a business identifier.
    #[must_use]
    pub fn logical(identifier: Identifier) -> Self {
        Self {
            identifier: Some(identifier),
            ..Self::default()
        }
    }

    /// Attach a display string to a reference.
    #[must_use]
    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = Some(display.into());
        self
    }

    /// Attach a resource type URI to a reference (useful for logical-only refs).
    #[must_use]
    pub fn with_type(mut self, type_: Uri) -> Self {
        self.type_ = Some(type_);
        self
    }
}
