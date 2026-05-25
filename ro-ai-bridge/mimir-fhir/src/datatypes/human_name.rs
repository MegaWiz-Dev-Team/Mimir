//! FHIR R5 `HumanName` datatype (Sprint 1 Day 4).
//!
//! Per ADR-006 Decision 5, Thai bilingual names are represented as TWO
//! `HumanName` entries (one Thai, one Latin) per Patient, each tagged with
//! a `language` field. This is friendlier than wrapping every `String`
//! field in a `LangString` newtype.
//!
//! Construction helpers `HumanName::thai()` and `HumanName::english()`
//! make the bilingual pattern ergonomic at the call site.

use serde::{Deserialize, Serialize};

use crate::datatypes::{Code, Period};

/// FHIR R5 `HumanName.use` enum.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-name-use.html>):
/// Distinguishes the purpose of a name (e.g., the official legal name vs
/// a nickname vs the maiden name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NameUse {
    /// Known as / conventional / the one usually used.
    Usual,
    /// The formal name as registered in an official (government) registry.
    Official,
    /// A temporary name.
    Temp,
    /// A name that is used to address the person in an informal manner,
    /// but is not part of their formal or usual name.
    Nickname,
    /// Anonymous assigned name (e.g. trauma patient before identification).
    Anonymous,
    /// This name is no longer in use (but it useful to keep on record).
    Old,
    /// A name used prior to changing name because of marriage.
    Maiden,
}

/// FHIR R5 `HumanName` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#HumanName>):
/// A human's name with the ability to identify parts and usage.
///
/// **Bilingual convention (Asgard / Thai medical):** create two `HumanName`
/// entries on `Patient.name` — one with `language = "th"` carrying Thai
/// script, one with `language = "en"` carrying the Latin transliteration.
/// Per [ADR-006 Decision 5], the `language` field is exposed as a regular
/// `Option<Code>` for ergonomics. On-wire it serializes as a plain JSON
/// field; full FHIR `_language` extension wrapping is deferred until a real
/// `HumanName` field needs it inside a real Resource (Sprint 2+).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct HumanName {
    /// The purpose / context for this name.
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_: Option<NameUse>,

    /// Specifies the entire name as it should be displayed
    /// e.g., on an application UI. May contain `family`/`given` content
    /// already rendered together.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Family name (often called surname / last name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,

    /// Given names (often called first / middle).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub given: Vec<String>,

    /// Honorific prefixes (e.g. "Dr.", "นาย", "นาง", "นางสาว").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix: Vec<String>,

    /// Suffix appended (e.g. "MD", "Jr.").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suffix: Vec<String>,

    /// Time period when the name was/is in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,

    /// Locale of the name (Asgard convention — see module-level docs).
    /// Typical values: `"th"`, `"en"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Code>,
}

impl HumanName {
    /// Construct a Thai-script `HumanName` with `language = "th"`.
    ///
    /// Use this for the canonical Thai name on a Patient. Pair with
    /// [`HumanName::english`] for the Latin transliteration.
    ///
    /// # Panics
    ///
    /// Does not panic — `Code::new("th")` is always valid grammar.
    #[must_use]
    pub fn thai(family: impl Into<String>, given: impl Into<String>) -> Self {
        Self {
            use_: Some(NameUse::Official),
            family: Some(family.into()),
            given: vec![given.into()],
            language: Some(Code::new("th").expect("valid lang code")),
            ..Self::default()
        }
    }

    /// Construct an English/Latin `HumanName` with `language = "en"`.
    ///
    /// Use this for the Latin transliteration that pairs with a Thai
    /// [`HumanName::thai`] entry. Marked as `Usual` (not `Official`) per
    /// MOPH convention — the Thai-script name is the official one for
    /// Thai citizens.
    ///
    /// # Panics
    ///
    /// Does not panic — `Code::new("en")` is always valid grammar.
    #[must_use]
    pub fn english(family: impl Into<String>, given: impl Into<String>) -> Self {
        Self {
            use_: Some(NameUse::Usual),
            family: Some(family.into()),
            given: vec![given.into()],
            language: Some(Code::new("en").expect("valid lang code")),
            ..Self::default()
        }
    }

    /// Set the `use` field (e.g. mark as `Nickname` or `Maiden`).
    #[must_use]
    pub fn with_use(mut self, use_: NameUse) -> Self {
        self.use_ = Some(use_);
        self
    }

    /// Add an honorific prefix (e.g. `"Dr."`, `"นพ."`).
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix.push(prefix.into());
        self
    }

    /// Add an additional given name.
    #[must_use]
    pub fn with_given(mut self, given: impl Into<String>) -> Self {
        self.given.push(given.into());
        self
    }
}
