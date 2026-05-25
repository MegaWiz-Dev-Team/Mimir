//! FHIR R5 datatypes (Sprint 1).
//!
//! Implementation order per Phase 1 plan Sprint 1:
//! 1. Primitives — `Id`, `Code`, `DateTime`, `Decimal`, etc. (Days 1-2)
//! 2. `Identifier`, `Coding`, `CodeableConcept` (Day 3)
//! 3. `Reference`, `HumanName` (Day 4)
//! 4. `Address` (Thai extension), `ContactPoint`, `Period` (Day 5)
//! 5. `Quantity`, `Money`, `Range`, `Ratio` (Day 6)
//! 6. `Annotation`, `Meta`, `Extension`, `Narrative` (Day 7)

mod address;
mod complex;
mod human_name;
mod numeric;
mod primitive;

pub use address::{Address, AddressType, AddressUse, TH_SUB_DISTRICT_EXTENSION_URL};
pub use complex::{
    CodeableConcept, Coding, ContactPoint, ContactPointSystem, ContactPointUse, Extension,
    Identifier, IdentifierUse, Period, Reference,
};
pub use human_name::{HumanName, NameUse};
pub use numeric::{Money, Quantity, QuantityComparator, Range, Ratio};
pub use primitive::{
    Code, CodeError, DateTime, DateTimeError, Decimal, Id, IdError, Markdown, MarkdownError, Uri,
    UriError, Url,
};
