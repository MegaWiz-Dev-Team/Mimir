//! R4 → R5 `Patient` — pure pass-through (the modeled subset is identical
//! across versions).

use crate::resources::Patient;
use crate::translate::r4::R4Patient;

/// Lift an R4 `Patient` to the canonical R5 [`Patient`].
///
/// `Patient` did not change in the modeled subset between R4 and R5, so this is
/// a field-by-field copy. It exists for symmetry with [`super::encounter`] and
/// to give callers a single, version-explicit entry point.
#[must_use]
pub fn patient(r4: R4Patient) -> Patient {
    Patient {
        id: r4.id,
        identifier: r4.identifier,
        active: r4.active,
        name: r4.name,
        telecom: r4.telecom,
        gender: r4.gender,
        birth_date: r4.birth_date,
        address: r4.address,
        ..Patient::new()
    }
}
