# mimir-fhir — Specification

> **Spec status:** Living · **Version:** 0.1 · **Last updated:** 2026-06-26
> **Part of:** [Asgard platform](../../../Asgard/docs/SPEC.md) — this is **Layer 1** (FHIR data plane).
> Consumed by: [asgard-iris](../../../asgard-iris/docs/SPEC.md) (extraction-service), and planned for asgard-underwriter, Eir, Mimir REST.

---

## 1. Identity

| | |
|---|---|
| **What** | A from-scratch **Rust FHIR R5 type system** — Layer 1 data plane for the Asgard medical/insurance stack |
| **Crate version** | **0.0.1** |
| **Design** | R5 canonical; R4 supported **only** at an adapter boundary (R4 client → R4↔R5 translator → R5 internal) |
| **Rule** | Never re-implement FHIR types elsewhere — **depend on this crate** |

## 2. Reality check — implemented vs planned

> This is the single most important section. Treat anything not in "implemented" as **not existing yet**.

**Implemented today (Sprint 1 + Sprint 2):**
- `datatypes` — primitives + complex + numeric + metadata + name/address (Sprint 2 added the `Date` primitive)
- `resources` — **Sprint 2:** `Patient` + `Encounter` (R5 canonical) each with an `External*` lenient-ingest pair (ADR-006 D4); backbone elements `PatientContact` / `Encounter{Diagnosis,Admission,Location}`; `AdministrativeGender` / `EncounterStatus` value sets
- `translate::{r4_to_r5, r5_to_r4}` — **Sprint 2:** Patient pass-through + Encounter renames (`period`↔`actualPeriod`, `hospitalization`↔`admission`), `class` widening, status remap; round-trip identity on the lossless subset
- `validators` — **Sprint 2:** TH Core / MoPH-PC Patient profile stubs (return Ok; real logic Sprint 7)
- `schema_export` — `all_datatype_schemas()` + **`all_resource_schemas()`** (Sprint 2)
- `terminology` — canonical system/identifier/extension/profile URIs (added 2026-05-30, ahead of resources, to converge consumers now)
- **20 test files** (Sprint 2 added `resources_patient`, `resources_encounter`, `translate_r4_r5`, `datatypes_date`)

**NOT implemented (commented out in `src/lib.rs`):**
`profiles` · `adapters` · `rest`

⇒ **2 of the 21 resources exist** (`Patient`, `Encounter`). The remaining 19 (`Condition`/`Observation`/`MedicationRequest`/`Composition`/`Coverage`/`Claim`/...) are gated on Sprint 3+.

## 3. Datatypes implemented

- **Primitives:** `Id`, `Code`, `Uri`, `Url`, `Markdown`, `Date` (partial precision; Sprint 2), `DateTime` (partial precision), `Instant`, `Decimal` (via `rust_decimal`, 28–29 sig digits — no IEEE-754 drift). *Deferred:* `Canonical`, `Time`, `Base64Binary`, `PositiveInt`, `UnsignedInt`.
- **Complex:** `Coding`, `CodeableConcept`, `Identifier`, `Period`, `ContactPoint`, `Reference`, `Extension` (9 common `value[x]` variants + nested).
- **Numeric:** `Quantity` (UCUM), `Money` (ISO 4217 + Thai Baht helper), `Range`, `Ratio`.
- **Metadata:** `Annotation`, `Meta` (versionId/lastUpdated derived from **Tyr** audit), `Narrative`.
- **Name/address:** `HumanName`, `Address`.

## 4. Thai localization

**In code now:**
- **Bilingual names** — `HumanName.language` (`th`/`en`) + `HumanName::thai()` / `::english()` (ADR-006 D5: two entries per Patient).
- **Thai 4-level address** — `line[]` / `district` (อำเภอ) / `state` (จังหวัด) / `postal_code`, with **sub-district** (ตำบล/แขวง) via extension `https://fhir.moph.go.th/StructureDefinition/sub-district`; `Address::thai(...)` helper.
- **Thai citizen-ID identifier** — convention: `Identifier{ system: "https://fhir.moph.go.th/identifier/citizen-id", value: "<13 digits>" }`.
- Thai phone/contact examples.

**Planned, NOT in code:** TH Core + MoPH-PC **profiles** & validators (Sprint 7; `profiles/th-core/` is empty `.gitkeep`), **ICD-10-TM**, **TMT** code systems.
> ⚠️ Because ICD-10-TM is not yet here, downstream repos invented their own system URIs (Iris `hl7.org/fhir/sid/icd-10-tm` vs Mimir scripts `terminology.fhir.moph.go.th/CodeSystem/icd-10-tm`). **mimir-fhir must declare the canonical URI** and everyone adopts it.

## 5. Scope (the 20 resources, when built)

Patient, Practitioner, Organization, Location, Encounter, Observation (+8 sub-profiles), Condition, Procedure, AllergyIntolerance, MedicationRequest, MedicationStatement, DiagnosticReport, ImagingStudy, Specimen, DocumentReference, Coverage, Claim, ClaimResponse, Immunization, Device. **Composition** added for UC2 patient-summary (ADR-015) → effectively 21. Bounded by MOPH-PC1 (not all ~145 R5 resources).

## 6. R5-specific design choices (when resources land)

- `MedicationRequest.medication` as **CodeableReference** (R5, not R4 `medicationReference`).
- `Encounter.actualPeriod` (R5 rename), `MedicationStatement.adherence` (R5-only). These are *why* R5 was chosen over R4 for MOPH-PC1.

## 7. How to consume

1. **Crate dependency:** `use mimir_fhir::datatypes::{...}` (today); resources later.
2. **Schema export:** `schema_export::all_datatype_schemas()` → JSON Schema for MCP/SDK/frontend codegen.
3. **REST (Sprint 6+):** `/fhir/r5/{ResourceType}/...` via axum.
4. **R4 adapter (Sprint 2+):** `translate::r4_to_r5::*`.
5. **Validators (Sprint 7+):** TH Core + MoPH-PC, tightest-binding-wins.

## 8. Sprint roadmap

S0 scaffold ✅ · **S1 datatypes ✅** · **S2 Patient + Encounter + R4↔R5 scaffold ✅** · S3–5 remaining resources · S6 REST · S7 profiles + validators · S8 **43Files/HOSxP adapter** (largest) · S9 SMART-on-FHIR launch + OpenEMR · S10 polish/deploy.

## 9. Tech

Pure Rust, no external FHIR library. Deps: `serde`, `serde_json`, `schemars` (0.8), `chrono`, `thiserror`, `rust_decimal`.

## 10. Reference docs

`README.md` · ADR-006 (canonical design) · ADR-012 (no-EHR data plane) · ADR-013 (R5 canonical) · `Asgard/docs/architecture/moph_pc1_fhir_mapping.md` (78-element list) · `Asgard/docs/technical/mimir-fhir-phase-1-plan.md`.

## 11. Open questions / alignment points

- ✅ **Done (2026-05-30):** canonical ICD-10-TM / TMT and other system URIs now in `mimir_fhir::terminology` (e.g. `terminology::ICD10_TM`). Remaining: consumers adopt — **Iris migrate** from `http://hl7.org/fhir/sid/icd-10-tm`; Python claim scripts already use the MOPH URL.
- Sprint 2 resource priority should be driven by the **claims + patient-summary** use cases actually in flight: Patient, Encounter, Condition, Observation, MedicationRequest, Coverage, Claim, Composition, DocumentReference.
- Confirm TH Core / MoPH-PC **profile canonical URLs** against the real published IG (current `https://fhir.moph.go.th/StructureDefinition/...` strings in prototype output are assumed).
