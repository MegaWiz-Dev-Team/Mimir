//! FHIR profile validators (stubs — Sprint 2).
//!
//! Real validation logic (slice cardinality, required extensions, fixed
//! systems, Thai citizen-id check digit, ...) ships in **Sprint 7**. For now
//! every validator is a total function returning `Ok(())`, so callers can wire
//! the validation call sites today and have them tighten later without an API
//! change.
//!
//! Binding precedence when multiple profiles apply is *tightest wins*
//! (MoPH-PC over TH Core) — see `SPEC.md` §7.

use crate::resources::Patient;

/// Error raised by a profile validator.
///
/// Carries the machine-stable `profile` canonical that rejected the resource
/// plus a human `message`. No instances are produced yet (Sprint 2 validators
/// are stubs); the type is defined now so the `Result`-returning signatures are
/// stable from the first call site.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("profile {profile} validation failed: {message}")]
pub struct ProfileError {
    /// Canonical URL of the profile that rejected the resource.
    pub profile: String,
    /// Human-readable reason.
    pub message: String,
}

/// Validate a [`Patient`] against the **TH Core Patient** profile.
///
/// # Errors
///
/// Returns [`ProfileError`] once real validation lands (Sprint 7). The Sprint 2
/// stub always returns `Ok(())`.
#[allow(clippy::unnecessary_wraps)] // Result kept for forward-compat (Sprint 7).
pub fn validate_th_core_patient(_patient: &Patient) -> Result<(), ProfileError> {
    Ok(())
}

/// Validate a [`Patient`] against the **MoPH-PC Patient** profile.
///
/// # Errors
///
/// Returns [`ProfileError`] once real validation lands (Sprint 7). The Sprint 2
/// stub always returns `Ok(())`.
#[allow(clippy::unnecessary_wraps)] // Result kept for forward-compat (Sprint 7).
pub fn validate_moph_pc_patient(_patient: &Patient) -> Result<(), ProfileError> {
    Ok(())
}
