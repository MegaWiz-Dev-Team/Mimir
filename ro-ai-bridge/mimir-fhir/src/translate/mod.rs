//! R4 ↔ R5 adapter boundary (Sprint 2 scaffold).
//!
//! Asgard is **R5-canonical** ([ADR-013]); R4 exists only at the edge. A legacy
//! R4 client's JSON is parsed into the R4 wire types in [`r4`], lifted to the
//! canonical R5 [`crate::resources`] types via [`r4_to_r5`], and (for outbound
//! R4 responses and round-trip tests) lowered back via [`r5_to_r4`].
//!
//! Sprint 2 covers `Patient` (lossless pass-through) and `Encounter` (the two
//! R5 renames `period` ↔ `actualPeriod` and `hospitalization` ↔ `admission`,
//! plus the `class` cardinality widening and the `status` value-set remap).
//!
//! [ADR-013]: ../../../../Asgard/docs/decisions/ADR-013-fhir-r5-canonical-version.md

pub mod r4;
pub mod r4_to_r5;
pub mod r5_to_r4;
