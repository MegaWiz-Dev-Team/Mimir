//! Canonical terminology & system URIs — the **single source of truth** for Asgard.
//!
//! Per the never-reimplement rule ([ADR-013](../../../../Asgard/docs/decisions/ADR-013-fhir-r5-canonical-version.md)),
//! no consumer (Iris, Underwriter, Eir, Mimir REST, Python claim scripts) may hard-code a
//! `system` / `Identifier.system` / `Extension.url` literal. Reference these constants instead.
//!
//! This module is intentionally available **before** FHIR resources exist (crate is datatypes-only
//! at v0.0.1) so the ecosystem can converge on one set of URIs now and avoid drift later.
//! See `SPEC.md` §"Open questions / alignment points".
//!
//! ```
//! use mimir_fhir::terminology;
//! assert_eq!(terminology::ICD10_TM, "https://terminology.fhir.moph.go.th/CodeSystem/icd-10-tm");
//! ```

// ── Code systems (FHIR `CodeSystem.url`) ───────────────────────────────────────

/// **ICD-10-TM** (Thai Modification) — the canonical diagnosis code system for Thailand.
///
/// CONVERGENCE DECISION (2026-05-30): this is the one URI every Asgard repo must use.
/// Supersedes the divergent `http://hl7.org/fhir/sid/icd-10-tm` (asgard-iris) — migrate to this.
pub const ICD10_TM: &str = "https://terminology.fhir.moph.go.th/CodeSystem/icd-10-tm";

/// ICD-10 (WHO international base release).
pub const ICD10: &str = "http://hl7.org/fhir/sid/icd-10";

/// ICD-9-CM — retained only as an *equivalence* coding alongside ICD-10-TM (legacy/claims).
pub const ICD9_CM: &str = "http://hl7.org/fhir/sid/icd-9-cm";

/// ICD-10-CM (US clinical modification). **NOT for Thai coding** — listed only so importers can
/// detect mis-tagged US codes and re-map them to [`ICD10_TM`].
pub const ICD10_CM: &str = "http://hl7.org/fhir/sid/icd-10-cm";

/// **TMT** — Thai Medicines Terminology (canonical drug code system for Thailand).
pub const TMT: &str = "https://terminology.fhir.moph.go.th/CodeSystem/tmt";

/// LOINC — labs, vital signs, document type codes.
pub const LOINC: &str = "http://loinc.org";

/// SNOMED CT — clinical terms.
pub const SNOMED_CT: &str = "http://snomed.info/sct";

/// UCUM — units of measure. Pin: must equal [`crate::datatypes::Quantity::UCUM_SYSTEM`].
pub const UCUM: &str = "http://unitsofmeasure.org";

/// HL7-published terminology CodeSystems (`terminology.hl7.org`).
pub mod hl7 {
    pub const CONDITION_CLINICAL: &str =
        "http://terminology.hl7.org/CodeSystem/condition-clinical";
    pub const CONDITION_CATEGORY: &str =
        "http://terminology.hl7.org/CodeSystem/condition-category";
    pub const CONDITION_VER_STATUS: &str =
        "http://terminology.hl7.org/CodeSystem/condition-ver-status";
    pub const OBSERVATION_CATEGORY: &str =
        "http://terminology.hl7.org/CodeSystem/observation-category";
    /// HL7 v3 ActCode — used for `Encounter.class` (e.g. `IMP` inpatient, `AMB` ambulatory).
    pub const ACT_CODE: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
}

// ── Identifier systems (FHIR `Identifier.system`) ──────────────────────────────

pub mod identifier {
    //! `Identifier.system` URIs.

    /// Thai national citizen ID (13-digit). Confirmed convention.
    pub const CITIZEN_ID: &str = "https://fhir.moph.go.th/identifier/citizen-id";

    /// Hospital Number. **Provisional** — confirm against the published TH Core IG.
    pub const HN: &str = "https://fhir.moph.go.th/identifier/hn";

    /// Admission Number. **Provisional** — confirm against the published TH Core IG.
    pub const AN: &str = "https://fhir.moph.go.th/identifier/an";
}

// ── Extension URLs (FHIR `Extension.url`) ──────────────────────────────────────

pub mod extension {
    //! `Extension.url` canonicals. Re-exported from their defining module so there is exactly
    //! one literal in the crate (no drift).

    /// Thai sub-district (ตำบล / แขวง) on `Address`.
    pub use crate::datatypes::TH_SUB_DISTRICT_EXTENSION_URL as TH_SUB_DISTRICT;
}

// ── Profile canonicals (FHIR `StructureDefinition.url`) ────────────────────────

pub mod profile {
    //! TH Core + MoPH-PC profile canonicals.
    //!
    //! **PROVISIONAL** — these base/slugs are assumed pending the officially published Thai FHIR
    //! IG. Do not treat as stable until verified (see `SPEC.md` §11). Centralised here so the
    //! eventual correction is a one-line change.

    /// Base for MoPH StructureDefinitions.
    pub const BASE: &str = "https://fhir.moph.go.th/StructureDefinition";
}
