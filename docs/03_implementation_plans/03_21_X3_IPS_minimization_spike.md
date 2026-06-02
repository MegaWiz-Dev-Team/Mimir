# X3 — IPS "minimum-necessary" minimization: design spike

**Status:** spike complete, 2026-06-02 · **Verdict: DO NOT BUILD YET (defer with a named trigger)** · design-only, no code changed.

**Question:** can the IPS refset (12,353 SNOMED concepts in Mimir, `snomed_refset_members refset_key='ips'`) act as a "minimum necessary" clinical set — filtering data that leaves the box (referral / insurer / cross-border) to enforce PDPA/HIPAA data-minimization? Reference plan: `03_20_SNOMED_Assets_CrossCutting_Plan.md` (X3).

---

## 1. Where does it belong — Skuggi, Tyr, or a new component?

**NOT Skuggi.** Confirmed it is a different axis. Skuggi (Heimdall in-process Rust middleware, W1 text Tier-1 shipped, NER gated — see `asgard_skuggi_state`) redacts **PII tokens** (names, IDs) in text/images. IPS-minimization filters **clinical concepts** (drop non-IPS *detail*) on a whole payload. Putting it in Skuggi's text tier would be the wrong layer — it operates on coded concepts, not PII spans. They are complementary.

**If anything, Tyr — and as an *audit/over-sharing detector*, not a stripping gate.** Reasoning below (§3): the one real rich-clinical export path *legitimately needs* full detail, so a *stripping* export-gate would harm the consumer. An *observability* rule (flag when an outbound clinical payload carries a high non-IPS-concept ratio) is the only shape that doesn't break a real workflow — and that is Tyr's job (Wazuh SIEM detection), not Skuggi's.

**A new export-gate component is premature** — no workflow needs concept-stripping today (§2).

## 2. What actually emits clinical data off-box today (verified 2026-06-02)

| Path | Off-box to | Clinical-concept richness | Minimization applicable? |
|---|---|---|---|
| **Underwriter APS report** (`asgard-underwriter/.../report.rs`) | the **insurer** (3rd party) | **High** — numbered problem list + clinical detail + timeline + labs + meds + risk, printable HTML | ✗ counter-productive: underwriting *needs* full history; dropping non-IPS detail removes the signal it exists to convey |
| Iris claim — NHSO XML / EDI 837-I (`asgard-iris` generation-service) | NHSO / สปสช | **Already minimal** — ICD-10-TM diagnoses + service lines, no narrative (verified: NHSO XML has no medication/narrative block) | n/a — nothing to strip |
| Iris claim summary (`summary_html`/`md`) | hospital staff via download | Moderate — structured diagnoses + meds | low: hospital's own data, attached to its own submission |
| tyr-archive → `gs://asgard-archive` | GCS (paid cloud archive) | **Not clinical** — SIEM/security telemetry | out of scope (not a clinical dataset) |
| Cloud LLM (Mimir/Syn → gemini) | Google | text/image | **PII-gated already** by Skuggi + `phi_strict`; asgard_medical/insurance are local-only. Different control axis. |

**No `referral` or `cross-border` clinical-sharing path exists in the codebase** (grep found none in iris/underwriter/mimir) — those are the plan's *hypothetical* future use-cases, not current emitters.

**Conclusion:** the only rich clinical concepts that leave the box to a third party go via the **Underwriter APS report → insurer**, and that consumer legitimately requires the detail. Every other path is either already concept-minimal, non-clinical, or PII-gated on a different axis.

## 3. Minimal shape (if/when built)

Because the one rich path needs its detail, the *only* non-breaking shape is **observability, not enforcement**:

- **Tyr over-sharing detector (audit, not strip).** When an outbound clinical payload is emitted (e.g. an APS report / exported bundle), compute the **non-IPS-concept ratio**: for each coded concept, call `POST /api/v1/knowledge/snomed/search?refset=ips&refset_only=false` (or batch membership) and read `in_refset`; ratio = (concepts not in IPS) / total. Emit a Tyr/Wazuh event with the ratio + concept list. Alert only above a threshold. This makes over-sharing *visible* without breaking the legitimate underwriting need.
- **Stripping export-gate** (drop non-IPS concepts before send) — defer entirely; it has no valid consumer today and would degrade the APS report.

Membership check reuses the live X1 endpoint pattern (`?refset=ips`, `in_refset` flag) — no new Mimir work needed.

## 4. Effort + is it worth building?

- **Effort:** Tyr audit-rule shape ≈ 2–3 d (payload tap on the export path + per-concept membership batch + Wazuh rule + threshold tuning). Stripping gate: more, and unjustified.
- **Worth building now? NO.** No off-box path both carries rich clinical concepts *and* would benefit from dropping non-IPS detail without harming its purpose. Existing controls already cover the real exposure: PII redaction (Skuggi), an already-minimal claim format, and local-only/PII-gated LLM. Building an IPS-minimization control now would be a solution looking for a problem.
- **Also blocked on** Skuggi W2+ (NER still gated) per the plan — the dependency is not met either.

### Defer with a named trigger
Revisit X3 **only when a path that sends rich clinical concepts to a party that does *not* need full detail is added** — concretely: a **referral export** (hospital→hospital), a **cross-border / HIE share**, or a **patient-facing data export**. At that point, build the **Tyr over-sharing detector** first (observability), and consider a stripping gate only for the patient-facing / cross-border case where minimization is mandated.

**One-line outcome:** IPS-minimization is real in principle but has no qualifying export path today; it is *not* a Skuggi feature; if built it is a Tyr audit rule; defer until a referral/cross-border/patient-export path exists.
