//! FHIR R5 metadata datatypes (Sprint 1 Day 7).
//!
//! Implemented: `Annotation`, `Meta`, `Narrative`, `NarrativeStatus`.
//!
//! ## Note on `Meta.versionId` / `Meta.lastUpdated`
//!
//! Per ADR-006 Decision 2, Asgard does NOT store `meta.versionId` or
//! `meta.lastUpdated` as authoritative state on the resource itself.
//! The resource row in the database is always the current state. Historical
//! state is reconstructible from the Tyr audit hash chain (ADR-002).
//!
//! The struct has these as `Option<...>` fields. On read, they are
//! populated from the latest Tyr audit event for the resource. On write
//! (from external clients), they are IGNORED.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{Coding, DateTime, Instant, Markdown, Reference, Uri};

// =============================================================================
// Annotation — FHIR R5 Annotation
// =============================================================================

/// FHIR R5 `Annotation` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Annotation>):
/// A text note associated with a resource — typically used on `Condition.note`,
/// `MedicationStatement.note`, `Procedure.note`, etc.
///
/// `author[x]` is polymorphic: either a `Reference` to a Practitioner /
/// Patient / `RelatedPerson` (authorReference) or a free-text string when
/// the author is not a registered entity (authorString). At most one of
/// the two should be set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    /// Polymorphic `author[x]`: structured Reference variant.
    #[serde(rename = "authorReference", skip_serializing_if = "Option::is_none")]
    pub author_reference: Option<Reference>,

    /// Polymorphic `author[x]`: free-text string variant.
    #[serde(rename = "authorString", skip_serializing_if = "Option::is_none")]
    pub author_string: Option<String>,

    /// When the annotation was made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<DateTime>,

    /// The annotation text itself (Markdown). Required by FHIR spec —
    /// represented as non-optional here since an empty annotation is
    /// meaningless. (Callers MUST provide text at construction.)
    pub text: Markdown,
}

impl Annotation {
    /// Construct an annotation with text only, no author or time.
    #[must_use]
    pub fn text_only(text: Markdown) -> Self {
        Self {
            author_reference: None,
            author_string: None,
            time: None,
            text,
        }
    }

    /// Construct an annotation authored by a referenced entity (e.g. Practitioner).
    #[must_use]
    pub fn by_reference(author: Reference, text: Markdown) -> Self {
        Self {
            author_reference: Some(author),
            author_string: None,
            time: None,
            text,
        }
    }

    /// Construct an annotation authored by a free-text name (e.g. legacy systems).
    #[must_use]
    pub fn by_string(author: impl Into<String>, text: Markdown) -> Self {
        Self {
            author_reference: None,
            author_string: Some(author.into()),
            time: None,
            text,
        }
    }

    /// Attach a time to an existing annotation.
    #[must_use]
    pub fn with_time(mut self, time: DateTime) -> Self {
        self.time = Some(time);
        self
    }
}

// =============================================================================
// Meta — FHIR R5 resource metadata
// =============================================================================

/// FHIR R5 `Meta` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/resource.html#Meta>):
/// Metadata about a resource — version, last update timestamp, profile
/// conformance claims, security labels, tags.
///
/// **Asgard implementation note** (per ADR-006 D2):
/// - `versionId` and `lastUpdated` are derived from Tyr audit on read,
///   not stored in the FHIR resource row directly.
/// - On write, these fields from external clients are IGNORED.
/// - Single source of truth = Tyr audit hash chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default)]
pub struct Meta {
    /// Version identifier — derived from Tyr audit on emit; ignored on input.
    #[serde(rename = "versionId", skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,

    /// Last-updated timestamp — derived from Tyr audit on emit; ignored on input.
    #[serde(rename = "lastUpdated", skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<Instant>,

    /// Source URL — identifies the system the resource originated from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Uri>,

    /// Profiles this resource claims conformance to.
    /// Each is a canonical URL (FHIR `canonical` type — stored as Uri for
    /// Phase 1 simplification; semantic distinction added later if needed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profile: Vec<Uri>,

    /// Security labels applied to this resource.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<Coding>,

    /// Tags applied for workflow / filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag: Vec<Coding>,
}

impl Meta {
    /// Construct a Meta declaring conformance to one or more profiles.
    #[must_use]
    pub fn with_profiles(profiles: Vec<Uri>) -> Self {
        Self {
            profile: profiles,
            ..Self::default()
        }
    }

    /// Declare conformance to a single profile (convenience).
    #[must_use]
    pub fn conforming_to(profile: Uri) -> Self {
        Self {
            profile: vec![profile],
            ..Self::default()
        }
    }
}

// =============================================================================
// Narrative — FHIR R5 human-readable summary
// =============================================================================

/// FHIR R5 `Narrative.status` value set.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-narrative-status.html>):
/// Indicates how the narrative was produced and whether it represents
/// the full content of the resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NarrativeStatus {
    /// Narrative generated from structured data; represents the full content.
    Generated,
    /// Narrative generated but contains data from extensions not in core spec.
    Extensions,
    /// Narrative contains additional information beyond what's in the data.
    Additional,
    /// No narrative provided (the resource has no human-readable summary).
    Empty,
}

/// FHIR R5 `Narrative` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/narrative.html>):
/// A human-readable summary of the resource, formatted as XHTML. Every
/// FHIR resource SHOULD have a Narrative — though many resources omit it
/// (relying on consumers to render structured data).
///
/// The `div` field carries an XHTML fragment (a single `<div>` element
/// with namespace `xmlns="http://www.w3.org/1999/xhtml"`). For Phase 1
/// we store it as `String` and do not parse / validate the XHTML at
/// construction — that's a downstream concern.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Narrative {
    /// How the narrative was produced.
    pub status: NarrativeStatus,

    /// The XHTML content — must start with `<div xmlns="...">` and end
    /// with `</div>`. Not validated at this layer.
    pub div: String,
}

impl Narrative {
    /// Construct an empty narrative (status=empty, minimal div).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            status: NarrativeStatus::Empty,
            div: r#"<div xmlns="http://www.w3.org/1999/xhtml"></div>"#.into(),
        }
    }

    /// Construct a generated narrative with the given div content.
    #[must_use]
    pub fn generated(div: impl Into<String>) -> Self {
        Self {
            status: NarrativeStatus::Generated,
            div: div.into(),
        }
    }
}
