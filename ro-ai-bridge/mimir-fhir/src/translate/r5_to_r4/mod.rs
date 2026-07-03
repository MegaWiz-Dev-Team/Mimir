//! Lower canonical R5 [`crate::resources`] types back to FHIR R4 wire types
//! ([`crate::translate::r4`]) — for outbound R4 responses and round-trip tests.

mod encounter;
mod patient;

pub use encounter::{encounter, r5_status_to_r4};
pub use patient::patient;
