//! FHIR R5 `Address` datatype with Thai sub-district extension support
//! (Sprint 1 Day 5).
//!
//! ## Thai address mapping convention
//!
//! Thai addresses have four administrative levels:
//!
//! | Thai level     | FHIR R5 field | Example       |
//! |----------------|---------------|---------------|
//! | จังหวัด (province)  | `state`         | "กรุงเทพมหานคร" |
//! | เขต / อำเภอ        | `district`      | "บางรัก"        |
//! | แขวง / ตำบล        | extension       | "สีลม"          |
//! | บ้านเลขที่ + ถนน      | `line[]`        | "123 ถ.สีลม"    |
//!
//! FHIR R5 has no native sub-district field — Thailand IG uses an
//! extension. The constant [`TH_SUB_DISTRICT_EXTENSION_URL`] defines
//! the canonical URL; helper [`Address::thai`] populates it correctly.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{Code, Extension, Period, Uri};

/// Canonical extension URL for Thai sub-district (แขวง / ตำบล).
///
/// Used on `Address.extension[]` per Thailand FHIR IG convention.
/// This is an Asgard-stable URL — if Thailand IG publishes an official
/// extension URL we will migrate adapters; the public-facing FHIR
/// resource will continue to honor both URLs during a transition window.
pub const TH_SUB_DISTRICT_EXTENSION_URL: &str =
    "https://fhir.moph.go.th/StructureDefinition/sub-district";

/// FHIR R5 `Address.use` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AddressUse {
    Home,
    Work,
    Temp,
    Old,
    Billing,
}

/// FHIR R5 `Address.type` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AddressType {
    Postal,
    Physical,
    Both,
}

/// FHIR R5 `Address` datatype.
///
/// Per FHIR R5 spec (<http://hl7.org/fhir/R5/datatypes.html#Address>):
/// An address expressed using postal conventions, with structured fields
/// for international interop. Thai addresses use the convention
/// documented at the module level above.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default)]
pub struct Address {
    /// home | work | temp | old | billing.
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_: Option<AddressUse>,

    /// postal | physical | both.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<AddressType>,

    /// Display text of the address as it would appear on an envelope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Street name, house number, apartment number, etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line: Vec<String>,

    /// City / town / village (or `เขต` / `อำเภอ` if not using `district`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,

    /// District / county — for Thailand: `เขต` / `อำเภอ`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,

    /// State / province — for Thailand: `จังหวัด`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Postal code (`รหัสไปรษณีย์` for Thailand, 5 digits).
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,

    /// ISO 3166 country code. Use `"TH"` for Thailand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<Code>,

    /// Time period when the address was/is in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,

    /// Extensions — used by Thailand IG for sub-district
    /// (`แขวง` / `ตำบล`). See [`TH_SUB_DISTRICT_EXTENSION_URL`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extension: Vec<Extension>,
}

impl Address {
    /// Construct a Thai home address with the canonical 4-level mapping.
    ///
    /// `line` is the street address (house number + road). `sub_district`,
    /// `district`, `province` map to ตำบล/แขวง, เขต/อำเภอ, จังหวัด
    /// respectively. The sub-district populates an `extension[]` entry
    /// (no native FHIR field exists for it).
    ///
    /// # Panics
    ///
    /// Does not panic — `Code::new("TH")` and the extension URL are both
    /// always-valid grammar.
    #[must_use]
    pub fn thai(
        line: impl Into<String>,
        sub_district: impl Into<String>,
        district: impl Into<String>,
        province: impl Into<String>,
        postal_code: impl Into<String>,
    ) -> Self {
        let sub_district_ext = Extension::string(
            Uri::new(TH_SUB_DISTRICT_EXTENSION_URL).expect("valid extension URL"),
            sub_district.into(),
        );
        Self {
            use_: Some(AddressUse::Home),
            line: vec![line.into()],
            district: Some(district.into()),
            state: Some(province.into()),
            postal_code: Some(postal_code.into()),
            country: Some(Code::new("TH").expect("valid country code")),
            extension: vec![sub_district_ext],
            ..Self::default()
        }
    }

    /// Helper: extract the Thai sub-district value from extensions, if present.
    #[must_use]
    pub fn sub_district(&self) -> Option<&str> {
        self.extension
            .iter()
            .find(|ext| ext.url.as_str() == TH_SUB_DISTRICT_EXTENSION_URL)
            .and_then(|ext| ext.value_string.as_deref())
    }

    /// Set the `use` field.
    #[must_use]
    pub fn with_use(mut self, use_: AddressUse) -> Self {
        self.use_ = Some(use_);
        self
    }

    /// Add another address line (e.g. apartment number).
    #[must_use]
    pub fn with_line(mut self, line: impl Into<String>) -> Self {
        self.line.push(line.into());
        self
    }
}
