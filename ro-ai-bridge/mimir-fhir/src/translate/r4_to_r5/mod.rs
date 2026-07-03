//! Lift FHIR R4 wire types ([`crate::translate::r4`]) to the canonical R5
//! [`crate::resources`] types.

mod encounter;
mod patient;

pub use encounter::{encounter, r4_status_to_r5};
pub use patient::patient;
