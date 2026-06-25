//! R5 → R4 `Patient` — pure pass-through (inverse of the R4 → R5 lift).

use crate::resources::Patient;
use crate::translate::r4::R4Patient;

/// Lower a canonical R5 [`Patient`] to an R4 `Patient` (lossless for the
/// modeled subset).
#[must_use]
pub fn patient(r5: &Patient) -> R4Patient {
    R4Patient {
        id: r5.id.clone(),
        identifier: r5.identifier.clone(),
        active: r5.active,
        name: r5.name.clone(),
        telecom: r5.telecom.clone(),
        gender: r5.gender,
        birth_date: r5.birth_date.clone(),
        address: r5.address.clone(),
    }
}
