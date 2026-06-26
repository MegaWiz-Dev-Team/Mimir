//! R4 → R5 `Encounter` — applies the two R5 renames plus the
//! `class: Coding` → `class: [CodeableConcept]` widening and the `status`
//! value-set remap.

use crate::datatypes::CodeableConcept;
use crate::resources::{Encounter, EncounterAdmission, EncounterStatus};
use crate::translate::r4::{R4Encounter, R4Hospitalization};

/// Lift an R4 `Encounter` to the canonical R5 [`Encounter`].
///
/// Translations applied:
/// - `period` → `actualPeriod` (R5 rename)
/// - `hospitalization` → `admission` (R5 rename)
/// - `class` single `Coding` → `[CodeableConcept]` (R5 cardinality widening)
/// - `status` string → [`EncounterStatus`] via [`r4_status_to_r5`]
///
/// A missing / unknown R4 `status` maps to [`EncounterStatus::Unknown`] (R5
/// requires `status`, so it cannot be left absent).
#[must_use]
pub fn encounter(r4: R4Encounter) -> Encounter {
    let status = r4
        .status
        .as_deref()
        .map_or(EncounterStatus::Unknown, r4_status_to_r5);

    let class = match r4.class {
        Some(coding) => vec![CodeableConcept::from_coding(coding)],
        None => Vec::new(),
    };

    let mut enc = Encounter::new(status);
    enc.id = r4.id;
    enc.identifier = r4.identifier;
    enc.class = class;
    enc.subject = r4.subject;
    enc.actual_period = r4.period; // R5 rename of `period`
    enc.admission = r4.hospitalization.map(admission_from_r4);
    enc
}

/// Map an R4 `Encounter.hospitalization` to the R5 `Encounter.admission`.
fn admission_from_r4(h: R4Hospitalization) -> EncounterAdmission {
    EncounterAdmission {
        discharge_disposition: h.discharge_disposition,
    }
}

/// Map an R4 `Encounter.status` string to the R5 [`EncounterStatus`] enum.
///
/// R4 and R5 use different value sets. Lossless pairs round-trip through the
/// R5 → R4 inverse; R4-only states (`arrived`, `triaged`) collapse to
/// [`EncounterStatus::InProgress`], and unrecognised strings to
/// [`EncounterStatus::Unknown`].
#[must_use]
// `arrived` / `triaged` share a body with `in-progress` but are kept separate
// to document the R4-only → R5 collapse explicitly.
#[allow(clippy::match_same_arms)]
pub fn r4_status_to_r5(status: &str) -> EncounterStatus {
    match status {
        "planned" => EncounterStatus::Planned,
        "in-progress" => EncounterStatus::InProgress,
        "onleave" => EncounterStatus::OnHold,
        "finished" => EncounterStatus::Completed,
        "cancelled" => EncounterStatus::Cancelled,
        "entered-in-error" => EncounterStatus::EnteredInError,
        "arrived" | "triaged" => EncounterStatus::InProgress,
        _ => EncounterStatus::Unknown,
    }
}
