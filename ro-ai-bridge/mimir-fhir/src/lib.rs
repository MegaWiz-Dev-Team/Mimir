//! mimir-fhir — FHIR R5 canonical type system for Asgard.
//!
//! See README.md for scope. See [ADR-006](../../../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md)
//! and [ADR-013](../../../../Asgard/docs/decisions/ADR-013-fhir-r5-canonical-version.md) for design.

pub mod datatypes;
pub mod resources;
pub mod schema_export;
pub mod terminology;
pub mod translate;
pub mod validators;

// Sprint 3-10 modules — declared but not yet implemented.
// Uncommenting before sprint kickoff is a planning error.
//
// pub mod profiles;
// pub mod adapters;
// pub mod rest;
