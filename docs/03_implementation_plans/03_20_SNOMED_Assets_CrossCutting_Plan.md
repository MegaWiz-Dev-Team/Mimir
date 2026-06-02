# Cross-cutting Applications of Sprint 58 KB Assets — Plan

**Sprint 59 (proposed)** · 2026-06-02 · builds on [03_19](03_19_SNOMED_Refsets_EDQM_IPS_GPFP_Ingest_Plan.md)

## Assets in hand (Sprint 58, live in mimir-api v2.3.66)
| Asset | Table | What it gives |
|---|---|---|
| IPS refset | `snomed_refset_members` (ips, 12,353) | interoperable minimal clinical concept set |
| GP/FP refset | `snomed_refset_members` (gpfp, 4,260) | primary-care reason-for-encounter set |
| EDQM dose map | `snomed_edqm_dose_map` (324) | SNOMED dose form ↔ EDQM code |
| TMT dose-link | `snomed_tmt_dose_link` (8,555 trusted) | TMT med → SNOMED+EDQM dose form |

Exposed today: `/knowledge/snomed/search?refset=ips|gpfp` (boost/filter), catalog counts.
**NOT yet exposed:** dose-form resolution (B3 `fhir_dose_form.py` is a *script*, callable only inside Mimir — see P0).

---

## P0 — Shared prerequisite: expose resolution as mimir-api endpoints
Cross-repo consumers (Iris, Underwriter, Syn) can't call a Python script. Promote to HTTP:
- `POST /api/v1/knowledge/snomed/dose-form` `{tmt_id}` → `{doseForm: CodeableConcept | null, trusted: bool}` (port `fhir_dose_form.resolve_dose_form` logic into `knowledge_snomed.rs`, needs_review=0 only).
- (already have) `search?refset=` for IPS/GP-FP membership; optionally add `POST /knowledge/snomed/in-refset {concept_ids[], refset}` for batch membership checks.
- Effort: ~0.5d Rust (mirror existing handlers). **Unblocks X1.**

---

## X2 — Eval gold sets (DO FIRST — fastest, enables measurement)
**Goal:** turn the refsets/maps into regression benchmarks so every later change (X1, dose-link curation, extraction model swap) is measured, not assumed. Extends B4.

**Inputs:** all 4 assets + existing labeled fixtures (UC2 4 fixtures, claims-pipeline 7 docs).

**Plug-in:** Mimir eval tables (`eval_benchmark_datasets/runs/scores/summary`, tenant `asgard_platform`), pattern = `persist_snomed_refset_eval.py` / `persist_primekg_resolver_eval.py`.

**Datasets to build:**
1. **dose-form precision** — sample N TMT meds → resolver output vs curated truth; track curated/exact vs token_subset error rate (token_subset is the risk tier).
2. **coding validity** — run the 7 claim-pipeline docs through extraction → assert each picked SNOMED/ICD-10-TM/dose-form code is *valid & in-scope* (in IPS / resolves to ICD-10-TM / dose-form trusted).
3. **IPS coverage** — % of UC2 summary concepts that are IPS members (interoperability score over time).

**Honest scope:** refsets give a *validity/coverage* check (is the code real & in-scope), not human-label accuracy — full accuracy needs the labeled fixtures. Combine both.
**Effort:** ~1–1.5d Python. **Value:** high (regression safety for everything downstream).

---

## X1 — Insurance enrichment (Iris / Underwriter)
**Goal:** richer, standardized claim/underwriting FHIR → more reliable risk assessment & cleaner reimbursement.

**Inputs:** dose-form (via P0 endpoint) + IPS gate.

**Plug-in (spans repos — Mimir serves, Iris/Underwriter consume):**
- Claim FHIR builder (`extraction_to_fhir_r5.py` medical + the Iris/Underwriter equivalents): call P0 dose-form endpoint → fill `Medication.doseForm` on claim meds (route/form → chronic-vs-acute, topical-vs-systemic signal for risk).
- Structure the claim clinical summary as an **IPS-conformant Composition** (reviewer-friendly, interoperable, reusable for cross-insurer).
- Underwriting risk scoring: coded meds + IPS-structured problem list feed scoring instead of free text.

**Dependencies:** P0; tenant `asgard_insurance` (exists); coordination with `asgard-iris` / `asgard-underwriter` repos (Iris coding is deterministic — fits a deterministic resolver call).
**Effort:** ~2–3d (P0 + insurance-side wiring, per repo). **Value:** high (business/revenue).
**Risk:** cross-repo; NHSO drug-claim spec may or may not require dose form — confirm before claiming reimbursement benefit.

---

## X3 — Tyr/Skuggi minimum-necessary dataset (DESIGN SPIKE — do last)
**Goal:** use IPS as the "minimum necessary" clinical set when data leaves the box (referral, insurer, cross-border) → data-minimization (PDPA/HIPAA principle).

**Honest framing:** this is a **different axis from Skuggi's current scope.** Skuggi redacts *PII* (names, IDs) in text/images. IPS-minimization filters *clinical concepts* (drop non-IPS detail on external export). They're complementary, not the same layer — so this is likely a **new sharing-gate / Tyr detection rule**, not a Skuggi text-tier feature.

**Possible shapes (pick in spike):**
- Export gate: when a bundle is destined external, flag/strip concepts not in IPS (over-sharing detector).
- Tyr rule: alert when an outbound payload carries rich non-IPS clinical detail.

**Dependencies:** Skuggi W2+ (NER still gated per `asgard_skuggi_state`); export path must exist. **Most design-heavy, least-defined.**
**Effort:** ~1d spike before any build. **Value:** compliance/trust (strategic, not immediate revenue).

---

## Recommended sequence
1. **X2 eval gold sets** — fast, pure Mimir, gives the measurement harness. *(start here)*
2. **P0 dose-form endpoint** — small Rust, unblocks X1. *(can parallel X2)*
3. **X1 insurance enrichment** — highest business value, needs P0 + repo coordination.
4. **X3 Tyr/Skuggi** — design spike first; build only if the spike justifies it.

**Open the work with X2 + P0** (both Mimir-owned, low risk, unblock the rest).
