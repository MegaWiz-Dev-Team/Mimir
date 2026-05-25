//! FHIR R5 numeric complex datatypes (Sprint 1 Day 6).
//!
//! Implemented: `Quantity`, `QuantityComparator`, `Money`, `Range`, `Ratio`.
//!
//! All four wrap [`crate::datatypes::Decimal`] values plus a unit / currency.
//! Used heavily by `Observation` (vital signs, lab values), `Coverage` /
//! `Claim` (monetary amounts), `Observation.referenceRange` (ranges).

use serde::{Deserialize, Serialize};

use crate::datatypes::{Code, Decimal, Uri};

// =============================================================================
// QuantityComparator — enum
// =============================================================================

/// FHIR R5 `Quantity.comparator` value set.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/valueset-quantity-comparator.html>):
/// indicates that the value is bounded, not exact — useful for sensitivity-limit
/// lab results ("less than 0.1") and asymptotic distributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuantityComparator {
    /// `<` — the actual value is less than the given value.
    #[serde(rename = "<")]
    LessThan,
    /// `<=` — the actual value is less than or equal to the given value.
    #[serde(rename = "<=")]
    LessOrEqual,
    /// `>=` — the actual value is greater than or equal to the given value.
    #[serde(rename = ">=")]
    GreaterOrEqual,
    /// `>` — the actual value is greater than the given value.
    #[serde(rename = ">")]
    GreaterThan,
    /// `ad` — the actual value is sufficient for the total quantity to equal
    /// the given value (asymptotic distribution). Rarely used clinically.
    #[serde(rename = "ad")]
    Ad,
}

// =============================================================================
// Quantity — measured amount
// =============================================================================

/// FHIR R5 `Quantity` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Quantity>):
/// A measured amount (or an amount that can potentially be measured).
/// `value` is a `Decimal`; `unit` is a human-readable label; `system` +
/// `code` together identify the unit unambiguously (typically UCUM).
///
/// Example LOINC body weight (29463-7) Quantity:
///
/// ```ignore
/// Quantity {
///     value: Some(Decimal::from_str("65.4").unwrap()),
///     unit: Some("kg".into()),
///     system: Some(Uri::new("http://unitsofmeasure.org").unwrap()),
///     code: Some(Code::new("kg").unwrap()),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Quantity {
    /// Numerical value (with implicit precision).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,

    /// Comparator (`<`, `<=`, `>=`, `>`, `ad`) — only set when value is bounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparator: Option<QuantityComparator>,

    /// Unit representation for display (e.g., `"kg"`, `"mmHg"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// System that defines the unit's coded form — typically the UCUM
    /// canonical URL `http://unitsofmeasure.org`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Uri>,

    /// Coded form of the unit (typically a UCUM code such as `"mm[Hg]"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Code>,
}

impl Quantity {
    /// Canonical UCUM system URL.
    pub const UCUM_SYSTEM: &'static str = "http://unitsofmeasure.org";

    /// Construct a UCUM-coded quantity with full system + unit metadata.
    ///
    /// # Panics
    ///
    /// Does not panic — `UCUM_SYSTEM` is a known-valid URI and the caller
    /// supplies an already-typed `Code`.
    #[must_use]
    pub fn ucum(value: Decimal, unit: impl Into<String>, ucum_code: Code) -> Self {
        Self {
            value: Some(value),
            unit: Some(unit.into()),
            system: Some(Uri::new(Self::UCUM_SYSTEM).expect("UCUM URL is valid")),
            code: Some(ucum_code),
            ..Self::default()
        }
    }

    /// Attach a comparator (e.g., for "less than detection limit" lab results).
    #[must_use]
    pub fn with_comparator(mut self, comparator: QuantityComparator) -> Self {
        self.comparator = Some(comparator);
        self
    }
}

// =============================================================================
// Money — amount of currency
// =============================================================================

/// FHIR R5 `Money` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Money>):
/// An amount of economic utility in some recognised currency. Used in
/// Coverage, Claim, `ChargeItem` for monetary amounts.
///
/// `currency` is an ISO 4217 currency code (`"THB"`, `"USD"`, etc.) —
/// stored as `Code` (already-validated grammar).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Money {
    /// Numerical amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,

    /// ISO 4217 currency code (`"THB"` for Thai Baht, `"USD"` for US Dollar).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Code>,
}

impl Money {
    /// Convenience constructor for Thai Baht amount.
    ///
    /// # Panics
    ///
    /// Does not panic — `"THB"` is a known-valid FHIR Code.
    #[must_use]
    pub fn thb(value: Decimal) -> Self {
        Self {
            value: Some(value),
            currency: Some(Code::new("THB").expect("THB is valid code")),
        }
    }

    /// Convenience constructor for an arbitrary ISO 4217 currency.
    #[must_use]
    pub fn new(value: Decimal, currency: Code) -> Self {
        Self {
            value: Some(value),
            currency: Some(currency),
        }
    }
}

// =============================================================================
// Range — pair of Quantities bounding a numeric range
// =============================================================================

/// FHIR R5 `Range` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Range>):
/// A set of ordered Quantity values bounded by `low` and `high`. Either
/// bound may be absent (open-ended range). Used for `Observation.referenceRange`
/// (normal lab value ranges), `DosageInstruction.doseAndRate` (drug dose ranges).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Range {
    /// Lower bound, inclusive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<Quantity>,

    /// Upper bound, inclusive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<Quantity>,
}

impl Range {
    /// Construct a fully-bounded range.
    #[must_use]
    pub fn between(low: Quantity, high: Quantity) -> Self {
        Self {
            low: Some(low),
            high: Some(high),
        }
    }

    /// Construct an open-ended-above range (`>= low`).
    #[must_use]
    pub fn at_least(low: Quantity) -> Self {
        Self {
            low: Some(low),
            high: None,
        }
    }

    /// Construct an open-ended-below range (`<= high`).
    #[must_use]
    pub fn at_most(high: Quantity) -> Self {
        Self {
            low: None,
            high: Some(high),
        }
    }
}

// =============================================================================
// Ratio — pair of Quantities representing numerator / denominator
// =============================================================================

/// FHIR R5 `Ratio` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Ratio>):
/// A relationship between two Quantity values. Used to express titres
/// (e.g., antibody titer 1:64), dose-rates (5 mg / 1 mL), and other
/// numerator-over-denominator clinical measurements.
///
/// FHIR rule: a `Ratio` with one Quantity present and the other absent
/// is invalid. Both must be present together, or both absent (which makes
/// the Ratio meaningless — typically don't emit it).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Ratio {
    /// Numerator quantity (e.g., `5 mg` for a dose rate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numerator: Option<Quantity>,

    /// Denominator quantity (e.g., `1 mL` for a dose rate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denominator: Option<Quantity>,
}

impl Ratio {
    /// Construct a ratio from numerator and denominator.
    #[must_use]
    pub fn new(numerator: Quantity, denominator: Quantity) -> Self {
        Self {
            numerator: Some(numerator),
            denominator: Some(denominator),
        }
    }
}
