# MOPH-PC1 Conformance Test Corpus

**Purpose:** Test data for FHIR R5 round-trip + profile validation against the
[78-element MOPH-PC1 mapping](../../../../../Asgard/docs/architecture/moph_pc1_fhir_mapping.md).

**Status (2026-05-24):** Placeholder. Sprint 0 Step 0.4 produces the actual corpus.

## Expected contents (Sprint 0 Step 0.4 deliverable)

5 synthetic patient bundles in `fixtures/`, each covering a slice of the 78 elements:

- `patient_01_complete.json` — every element populated (canary)
- `patient_02_outpatient.json` — OPD HT/DM follow-up scenario (UC1)
- `patient_03_inpatient.json` — IPD with discharge (UC4)
- `patient_04_paediatric.json` — well-child + immunization (UC3)
- `patient_05_lab_specimen.json` — full lab workflow + Specimen

Bundles must:
- Use FHIR R5 resource shapes
- Use TH Core profile bindings where applicable
- Cover all 78 MOPH-PC1 element IDs across the 5 bundles
- Pass `validator::validate()` once Sprint 7 lands
- Use synthetic Thai data (no PII, no real patient identifiers)

## Reference per element

See [MOPH-PC1 mapping doc](../../../../../Asgard/docs/architecture/moph_pc1_fhir_mapping.md)
for the canonical R5 path per element ID.

## Loader (Sprint 0)

A test corpus loader in `tests/moph_pc1/corpus_loader.rs` will be added in
Sprint 0 Day 3 to read these fixtures and run round-trip tests against them.
