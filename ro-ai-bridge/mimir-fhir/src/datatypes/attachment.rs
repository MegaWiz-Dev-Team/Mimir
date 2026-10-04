//! FHIR R5 `Attachment`, `Signature`, `CodeableReference` (added for the Nótt
//! Local clinical-document workflow: report PDFs, physician sign-off, R5
//! reference-or-concept fields).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{CodeableConcept, Coding, DateTime, Instant, Reference, Uri};
use crate::datatypes::Code;

/// FHIR R5 `Attachment` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Attachment>).
/// Either `data` (inline base64) or `url` (where the bytes live) is used; large
/// payloads such as EDF recordings or report PDFs are referenced by `url`
/// (e.g. `Binary/{id}`) rather than inlined.
///
/// `size` is FHIR `integer64`, which R5 JSON serializes **as a string**.
/// `hash` is defined by FHIR as base64 SHA-1 of `data`; stores that need a
/// stronger digest record SHA-256 in their own integrity log (see
/// ADR-006 Amendment 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default)]
pub struct Attachment {
    /// MIME type of the content (e.g. `application/pdf`).
    #[serde(rename = "contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Code>,
    /// Human language of the content (BCP-47, e.g. `th`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Code>,
    /// Inline data, base64-encoded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Where the data can be accessed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Number of bytes (FHIR `integer64` → JSON string).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "integer64_as_string"
    )]
    #[schemars(with = "Option<String>")]
    pub size: Option<u64>,
    /// Hash of the data (FHIR: base64 SHA-1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Label to display in place of the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Date the attachment was first created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation: Option<DateTime>,
}

impl Attachment {
    /// Attachment pointing at bytes stored elsewhere (e.g. `Binary/abc`).
    #[must_use]
    pub fn by_url(content_type: Code, url: impl Into<String>, size: u64) -> Self {
        Self {
            content_type: Some(content_type),
            url: Some(url.into()),
            size: Some(size),
            ..Self::default()
        }
    }

    /// Set a display title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// `integer64` (de)serializer: FHIR R5 JSON carries 64-bit integers as strings.
mod integer64_as_string {
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option)] // serde `with` requires `&Option<T>`
    pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(n) => s.serialize_str(&n.to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StrOrNum {
            S(String),
            N(u64),
        }
        match Option::<StrOrNum>::deserialize(d)? {
            None => Ok(None),
            Some(StrOrNum::N(n)) => Ok(Some(n)),
            Some(StrOrNum::S(s)) => s.parse().map(Some).map_err(serde::de::Error::custom),
        }
    }
}

/// FHIR R5 `Signature` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Signature>). All
/// elements are optional in R5. Used on `Provenance.signature` to record a
/// physician's sign-off: who, when, and over which content (`data` / `sigFormat`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default)]
pub struct Signature {
    /// Indication of the reason the entity signed (e.g. author, verification).
    #[serde(rename = "type", default, skip_serializing_if = "Vec::is_empty")]
    pub type_: Vec<Coding>,
    /// When the signature was created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<Instant>,
    /// Who signed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub who: Option<Reference>,
    /// The party represented.
    #[serde(rename = "onBehalfOf", skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<Reference>,
    /// MIME type of the signed content.
    #[serde(rename = "targetFormat", skip_serializing_if = "Option::is_none")]
    pub target_format: Option<Code>,
    /// MIME type of the signature itself.
    #[serde(rename = "sigFormat", skip_serializing_if = "Option::is_none")]
    pub sig_format: Option<Code>,
    /// The signature value, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// ASTM E1762-95 signature-type code system (used for `Signature.type`).
pub const SIGNATURE_TYPE_SYSTEM: &str = "urn:iso-astm:E1762-95:2013";

impl Signature {
    /// A verification signature (ASTM `1.2.840.10065.1.12.1.5`) by `who` at `when`.
    ///
    /// # Panics
    /// Never: the system URI and code are compile-time constants that satisfy
    /// the `Uri` / `Code` grammars.
    #[must_use]
    pub fn verification(who: Reference, when: Instant) -> Self {
        Self {
            type_: vec![Coding::new(
                Uri::new(SIGNATURE_TYPE_SYSTEM).expect("valid system uri"),
                Code::new("1.2.840.10065.1.12.1.5").expect("valid code"),
            )
            .with_display("Verification Signature")],
            when: Some(when),
            who: Some(who),
            ..Self::default()
        }
    }
}

/// FHIR R5 `CodeableReference` — a concept, a reference, or both.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/references.html#CodeableReference>).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default)]
pub struct CodeableReference {
    /// Coded concept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept: Option<CodeableConcept>,
    /// Reference to a resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<Reference>,
}

impl CodeableReference {
    /// From a concept only.
    #[must_use]
    pub fn concept(concept: CodeableConcept) -> Self {
        Self {
            concept: Some(concept),
            reference: None,
        }
    }
    /// From a reference only.
    #[must_use]
    pub fn reference(reference: Reference) -> Self {
        Self {
            concept: None,
            reference: Some(reference),
        }
    }
}
