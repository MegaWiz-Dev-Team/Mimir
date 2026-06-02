//! JSON Schema export pipeline (Sprint 1 Day 8 — per [ADR-006 D3]).
//!
//! Every `mimir-fhir` public datatype derives [`schemars::JsonSchema`].
//! This module provides a single entry point [`all_datatype_schemas`] that
//! returns every datatype's JSON Schema, keyed by FHIR type name. Use:
//!
//! ```
//! let schemas = mimir_fhir::schema_export::all_datatype_schemas();
//! for (name, schema) in &schemas {
//!     // Write schema to target/schemas/{name}.schema.json or feed to Hermodr.
//! }
//! ```
//!
//! Why a runtime helper instead of `build.rs`: keeps cargo builds fast,
//! lets callers (Hermodr MCP tool generator, frontend `quicktype`, CI
//! schema-export job) trigger generation explicitly.

use std::collections::BTreeMap;

use schemars::schema::RootSchema;
use schemars::schema_for;

use crate::datatypes::{
    Address, Annotation, CodeableConcept, Coding, ContactPoint, Decimal, Extension, HumanName,
    Identifier, Meta, Money, Narrative, Period, Quantity, Range, Ratio, Reference,
};

/// Generate JSON Schemas for all `mimir-fhir` complex datatypes.
///
/// Returned map is sorted alphabetically by FHIR type name for stable
/// output (useful for diffs in CI). Primitive datatypes are NOT included
/// in this map — they have trivial schemas (string with optional pattern)
/// and are inlined where they appear inside complex datatypes.
#[must_use]
pub fn all_datatype_schemas() -> BTreeMap<String, RootSchema> {
    let mut out = BTreeMap::new();
    macro_rules! add {
        ($t:ty) => {{
            let schema = schema_for!($t);
            out.insert(stringify!($t).to_string(), schema);
        }};
    }
    add!(Address);
    add!(Annotation);
    add!(CodeableConcept);
    add!(Coding);
    add!(ContactPoint);
    add!(Decimal);
    add!(Extension);
    add!(HumanName);
    add!(Identifier);
    add!(Meta);
    add!(Money);
    add!(Narrative);
    add!(Period);
    add!(Quantity);
    add!(Range);
    add!(Ratio);
    add!(Reference);
    out
}

/// Convenience: serialise all schemas to JSON strings, keyed by type name.
///
/// Use this when writing schemas to disk or shipping to Hermodr MCP.
///
/// # Panics
///
/// Does not panic in practice — `serde_json::to_string_pretty` over a
/// `RootSchema` produced by `schema_for!` is infallible. The `.expect`
/// is a defensive guard, never observed to fire.
#[must_use]
pub fn all_datatype_schemas_json() -> BTreeMap<String, String> {
    all_datatype_schemas()
        .into_iter()
        .map(|(name, schema)| {
            (
                name,
                serde_json::to_string_pretty(&schema).expect("schema should serialise"),
            )
        })
        .collect()
}
