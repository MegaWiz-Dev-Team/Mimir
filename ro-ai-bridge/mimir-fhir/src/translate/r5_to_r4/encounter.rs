//! R5 → R4 `Encounter` — inverts the renames / widening of the R4 → R5 lift.

use crate::resources::{Encounter, EncounterStatus};
use crate::translate::r4::{R4Encounter, R4Hospitalization};

/// Lower a canonical R5 [`Encounter`] to an R4 `Encounter`.
///
/// Inverse renames: `actualPeriod` → `period`, `admission` →
/// `hospitalization`. `class` takes the first `Coding` of the first
/// `CodeableConcept` (R4 holds a single `Coding`).
#[must_use]
pub fn encounter(r5: &Encounter) -> R4Encounter {
    R4Encounter {
        id: r5.id.clone(),
        identifier: r5.identifier.clone(),
        status: Some(r5_status_to_r4(r5.status).to_string()),
        class: r5.class.first().and_then(|cc| cc.coding.first()).cloned(),
        subject: r5.subject.clone(),
        period: r5.actual_period.clone(), // R4 name for `actualPeriod`
        hospitalization: r5.admission.as_ref().map(|a| R4Hospitalization {
            discharge_disposition: a.discharge_disposition.clone(),
        }),
    }
}

/// Map an R5 [`EncounterStatus`] back to an R4 `Encounter.status` string.
///
/// Inverse of the R4 → R5 status remap on the lossless subset. R5-only states
/// (`discharged`, `discontinued`) have no exact R4 equivalent and map best-effort.
#[must_use]
// The R5-only arms intentionally share a body with a lossless arm but are kept
// separate to document the best-effort mapping explicitly (Sprint 7 may revise).
#[allow(clippy::match_same_arms)]
pub fn r5_status_to_r4(status: EncounterStatus) -> &'static str {
    match status {
        EncounterStatus::Planned => "planned",
        EncounterStatus::InProgress => "in-progress",
        EncounterStatus::OnHold => "onleave",
        EncounterStatus::Completed => "finished",
        EncounterStatus::Cancelled => "cancelled",
        EncounterStatus::EnteredInError => "entered-in-error",
        EncounterStatus::Unknown => "unknown",
        EncounterStatus::Discharged => "in-progress",
        // R5-only "discontinued" (stopped before completion) has no exact R4
        // code; "cancelled" is the closest (vs "entered-in-error", which means
        // the record should never have existed).
        EncounterStatus::Discontinued => "cancelled",
    }
}
