//! FHIR R5 `ExtendedContactDetail` (<http://hl7.org/fhir/R5/metadatatypes.html#ExtendedContactDetail>):
//! a contact for a purpose — the names, telecoms and address to reach it. R5
//! `Organization.contact` uses it in place of R4's top-level address and telecom.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::datatypes::{Address, CodeableConcept, ContactPoint, HumanName, Period, Reference};

/// FHIR R5 `ExtendedContactDetail`. Every element is optional.
///
/// ```
/// use mimir_fhir::datatypes::{Address, ContactPoint, ExtendedContactDetail};
/// let letterhead = ExtendedContactDetail {
///     address: Some(Address { text: Some("123 ถนนพระราม 6".into()), ..Address::default() }),
///     telecom: vec![ContactPoint::phone("02-000-0000")],
///     ..ExtendedContactDetail::default()
/// };
/// assert_eq!(letterhead.telecom.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct ExtendedContactDetail {
    /// The type of contact (e.g. billing, press).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<CodeableConcept>,
    /// Names of the people or departments to contact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<HumanName>,
    /// Phone, email and other telecoms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,
    /// Address for the contact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,
    /// The organization this contact is for, if not the containing one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<Reference>,
    /// When this contact is valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
}
