//! FHIR R5 primitive datatypes.
//!
//! Sprint 1 Day 1-2 scope: `Id` (this file), `Code`, `Canonical`, `Uri`, `Url`,
//! `Markdown`, `DateTime`, `Date`, `Time`, `Instant`, `Base64Binary`, `Decimal`,
//! `PositiveInt`, `UnsignedInt`.
//!
//! Currently implemented: `Id`, `Code`, `Uri`, `Url`, `Markdown`, `DateTime`,
//! `Decimal` (re-export of [`rust_decimal::Decimal`]).
//!
//! ## Note on `Decimal`
//!
//! FHIR R5 `decimal` allows arbitrary precision. We re-export
//! [`rust_decimal::Decimal`] (28-29 significant digits, base-10 — preserves
//! trailing zeros and avoids IEEE-754 float drift). For healthcare numeric
//! ranges (vital signs, lab values, drug doses) this is comfortably above
//! the precision actually used by any clinical instrument.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// FHIR R5 `decimal` primitive — re-export of [`rust_decimal::Decimal`].
pub use rust_decimal::Decimal;

// =============================================================================
// Id — FHIR R5 logical resource id
// =============================================================================

/// FHIR R5 `id` primitive type.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#id>):
/// regex `[A-Za-z0-9\-\.]{1,64}`. Used as the logical id of resources.
///
/// Validation happens at construction and at deserialize time — an invalid
/// id cannot exist as an `Id` value, which prevents downstream
/// FHIR-non-conformance bugs at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Id(String);

/// Errors raised when constructing an invalid [`Id`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdError {
    #[error("FHIR Id must be 1-64 characters, got {0}")]
    InvalidLength(usize),

    #[error("FHIR Id contains invalid character {0:?} (only A-Z a-z 0-9 - . allowed)")]
    InvalidCharacter(char),
}

impl Id {
    /// Construct an `Id`, validating against the FHIR R5 grammar.
    ///
    /// # Errors
    ///
    /// Returns [`IdError::InvalidLength`] if the input is empty or longer
    /// than 64 characters. Returns [`IdError::InvalidCharacter`] if the
    /// input contains a character outside `[A-Za-z0-9\-\.]`.
    pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        let len = s.len();
        if !(1..=64).contains(&len) {
            return Err(IdError::InvalidLength(len));
        }
        if let Some(c) = s.chars().find(|c| !is_valid_id_char(*c)) {
            return Err(IdError::InvalidCharacter(c));
        }
        Ok(Self(s))
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

const fn is_valid_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '.'
}

// =============================================================================
// Code — FHIR R5 coded value
// =============================================================================

/// FHIR R5 `code` primitive type.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#code>):
/// regex `[^\s]+(\s[^\s]+)*`. Non-empty, no leading/trailing whitespace,
/// internal whitespace allowed only as single spaces (never tabs or newlines).
///
/// Used wherever values come from a controlled vocabulary — e.g.
/// `Observation.status = "final"`, `Patient.gender = "male"`,
/// `MedicationRequest.intent = "order"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Code(String);

/// Errors raised when constructing an invalid [`Code`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodeError {
    #[error("FHIR Code must be non-empty")]
    Empty,

    #[error("FHIR Code has leading or trailing whitespace")]
    EdgeWhitespace,

    #[error(
        "FHIR Code contains forbidden whitespace ({0:?}); only single spaces allowed internally"
    )]
    InvalidWhitespace(char),

    #[error("FHIR Code contains consecutive whitespace")]
    ConsecutiveWhitespace,
}

impl Code {
    /// Construct a `Code`, validating against the FHIR R5 grammar.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::Empty`] if input is empty. Returns
    /// [`CodeError::EdgeWhitespace`] on leading/trailing whitespace.
    /// Returns [`CodeError::InvalidWhitespace`] on tabs/newlines/etc.
    /// Returns [`CodeError::ConsecutiveWhitespace`] on `"  "` or `" \t"`.
    pub fn new(s: impl Into<String>) -> Result<Self, CodeError> {
        let s = s.into();
        if s.is_empty() {
            return Err(CodeError::Empty);
        }
        if s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) {
            return Err(CodeError::EdgeWhitespace);
        }
        let mut prev_was_ws = false;
        for c in s.chars() {
            if c.is_whitespace() {
                if c != ' ' {
                    return Err(CodeError::InvalidWhitespace(c));
                }
                if prev_was_ws {
                    return Err(CodeError::ConsecutiveWhitespace);
                }
                prev_was_ws = true;
            } else {
                prev_was_ws = false;
            }
        }
        Ok(Self(s))
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Code {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Code {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

// =============================================================================
// Uri — FHIR R5 URI per RFC 3986
// =============================================================================

/// FHIR R5 `uri` primitive type.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#uri>): any string
/// that conforms to RFC 3986. Used for canonical URLs, OIDs (`urn:oid:...`),
/// UUIDs (`urn:uuid:...`), and absolute/relative web URLs.
///
/// Validation is intentionally permissive: non-empty and no whitespace.
/// Full RFC 3986 validation is out of scope for Phase 1 (FHIR servers
/// generally trust caller-provided URIs; structural validation is
/// expensive and rarely productive).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Uri(String);

/// Errors raised when constructing an invalid [`Uri`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UriError {
    #[error("FHIR Uri must be non-empty")]
    Empty,

    #[error("FHIR Uri must not contain whitespace (got {0:?})")]
    ContainsWhitespace(char),
}

impl Uri {
    /// Construct a `Uri`, applying minimal validation.
    ///
    /// # Errors
    ///
    /// Returns [`UriError::Empty`] if input is empty.
    /// Returns [`UriError::ContainsWhitespace`] if any whitespace present.
    pub fn new(s: impl Into<String>) -> Result<Self, UriError> {
        let s = s.into();
        if s.is_empty() {
            return Err(UriError::Empty);
        }
        if let Some(c) = s.chars().find(|c| c.is_whitespace()) {
            return Err(UriError::ContainsWhitespace(c));
        }
        Ok(Self(s))
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Uri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

// =============================================================================
// Url — FHIR R5 URL (specialization of Uri)
// =============================================================================

/// FHIR R5 `url` primitive type.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#url>): a URI
/// Reference (RFC 3986 §4.1). In practice indistinguishable from `Uri` at
/// the validation layer — kept as a separate type for FHIR semantic
/// clarity (e.g. `Endpoint.address` is `url`, `Identifier.system` is `uri`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Url(String);

impl Url {
    /// Construct a `Url`, applying minimal validation.
    ///
    /// # Errors
    ///
    /// Same as [`Uri::new`].
    pub fn new(s: impl Into<String>) -> Result<Self, UriError> {
        let s = s.into();
        if s.is_empty() {
            return Err(UriError::Empty);
        }
        if let Some(c) = s.chars().find(|c| c.is_whitespace()) {
            return Err(UriError::ContainsWhitespace(c));
        }
        Ok(Self(s))
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Url {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Url {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

// =============================================================================
// Markdown — FHIR R5 markdown text
// =============================================================================

/// FHIR R5 `markdown` primitive type.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#markdown>):
/// commonmark-formatted string. No syntactic validation at construction —
/// markdown is intentionally permissive ("just text with possible markdown
/// markers"). Use for `Annotation.text`, `Condition.note.text`, etc.
///
/// Empty markdown is disallowed by the FHIR spec — must contain at least
/// one non-whitespace character.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Markdown(String);

/// Errors raised when constructing invalid [`Markdown`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MarkdownError {
    #[error("FHIR Markdown must contain at least one non-whitespace character")]
    EmptyOrWhitespaceOnly,
}

impl Markdown {
    /// Construct a `Markdown` value.
    ///
    /// # Errors
    ///
    /// Returns [`MarkdownError::EmptyOrWhitespaceOnly`] if the input is
    /// empty or whitespace-only.
    pub fn new(s: impl Into<String>) -> Result<Self, MarkdownError> {
        let s = s.into();
        if s.chars().all(char::is_whitespace) {
            return Err(MarkdownError::EmptyOrWhitespaceOnly);
        }
        Ok(Self(s))
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Markdown {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Markdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Markdown {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

// =============================================================================
// DateTime — FHIR R5 dateTime (partial-precision OK)
// =============================================================================

/// FHIR R5 `dateTime` primitive type.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#dateTime>):
/// A date, date-time, or partial date in one of four formats:
///
/// - `YYYY` — year only (e.g. `"2026"`)
/// - `YYYY-MM` — year + month (e.g. `"2026-05"`)
/// - `YYYY-MM-DD` — full date (e.g. `"2026-05-24"`)
/// - `YYYY-MM-DDThh:mm:ss[.sss](Z|±hh:mm)` — full datetime with timezone
///
/// Implementation strategy: store as `String` (preserves the original
/// precision the source provided — we do not auto-promote `"2026-05"` to
/// `"2026-05-01T00:00:00Z"`). Validation uses `chrono` for full datetimes
/// and structural check for partial dates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DateTime(String);

/// Errors raised when constructing an invalid [`DateTime`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DateTimeError {
    #[error("FHIR dateTime has invalid format: {0:?}")]
    InvalidFormat(String),
}

impl DateTime {
    /// Construct a `DateTime`, validating against the FHIR R5 grammar.
    ///
    /// # Errors
    ///
    /// Returns [`DateTimeError::InvalidFormat`] if the input does not match
    /// any of the four FHIR R5 dateTime formats.
    pub fn new(s: impl Into<String>) -> Result<Self, DateTimeError> {
        let s = s.into();
        if is_valid_fhir_datetime(&s) {
            Ok(Self(s))
        } else {
            Err(DateTimeError::InvalidFormat(s))
        }
    }

    /// Borrow the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for DateTime {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

fn is_valid_fhir_datetime(s: &str) -> bool {
    match s.len() {
        4 => is_valid_year(s),
        7 => is_valid_year_month(s),
        10 => is_valid_full_date(s),
        // 20+ chars implies full datetime — defer to chrono
        n if n >= 20 => chrono::DateTime::parse_from_rfc3339(s).is_ok(),
        _ => false,
    }
}

fn is_valid_year(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_digit())
}

fn is_valid_year_month(s: &str) -> bool {
    // YYYY-MM
    let bytes = s.as_bytes();
    if bytes[4] != b'-' {
        return false;
    }
    let year = &s[..4];
    let month = &s[5..];
    if !is_valid_year(year) {
        return false;
    }
    month.parse::<u8>().is_ok_and(|m| (1..=12).contains(&m))
}

fn is_valid_full_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}
