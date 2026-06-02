# SNOMED Refset + EDQM Ingest & Usage Plan (IPS / GP-FP / EDQM)

**Sprint 58** · status: PLANNED · author: platform · date: 2026-06-01

## Source / Provenance

| Field | Value |
|---|---|
| Distributor | SNOMED International — **Member Licensing & Distribution Service (MLDS)** |
| Portal | <https://mlds.ihtsdotools.org/#/userDashboard> |
| Member | Thailand |
| License | IHTSDO Affiliate License 2023 (PDFs at `$MIMIR_KB/SnomedCT/*.pdf`); commercial-cleared per `DATA_LICENSE.md` |
| Obligation | SNOMED **≤180-day upgrade** — applies to all three packages |

All three packages are **already downloaded & verified** on disk:

```
$MIMIR_KB = /Volumes/T7 Shield/asgard-data/mimir-kb
$MIMIR_KB/SnomedCT/
  SnomedCT_InternationalRF2_PRODUCTION_20260501  ← prerequisite (already ingested)
  SnomedCT_IPS_PRODUCTION_20250930               ← IPS  simple refset, 12,679 members
  SnomedCT_GPFP_PRODUCTION_20260331              ← GP/FP simple refset + ExtendedMap
  SnomedCT_SNOMEDEDQMMapPackage_20250930         ← EDQM SimpleMap, 328 dose-form rows
```

## Why these three (recap)

| Package | Gap it closes | Downstream consumer |
|---|---|---|
| **EDQM** dose-form map | TMT dose forms are free-text in `tmt_codes.fsn` — no coded target | FHIR `Medication.doseForm` coded concept |
| **IPS** refset | patient summaries built from raw FHIR Bundles, no standard concept subset | UC2 patient-summary / `Composition` skill |
| **GP/FP** refset | no primary-care "reason for encounter" concept subset | MOPH-PC1 scope, Eir primary-care agents |

> ⚠️ **Correction recorded:** earlier in planning, EDQM + GP/FP were proposed as "to download" — they were in fact already on disk; only IPS needed downloading (since done). Verified against folder, not memory.

---

## Part A — INGEST

### A0. Backup (MANDATORY — backup-first rule)
Before any write to MariaDB:
```bash
./scripts/backup-shared-kbs.sh          # or backup-full-k8s.sh for full
# verify MANIFEST + gzip integrity before proceeding
```
Tables touched are all **new** (CREATE IF NOT EXISTS) — no existing data is mutated — but the rule stands: snapshot first, verify, then proceed.

### A1. Schema
`ro-ai-bridge/migrations/sprint58_snomed_refsets_edqm.sql` (written). Four tables, all `tenant_id=NULL` shared master, mirroring `icd10_codes` / `tmt_codes` conventions:

| Table | Holds | Rows (expected) |
|---|---|---|
| `snomed_refset_members` | generic simple-refset membership, `refset_key` discriminator (`ips`,`gpfp`) | ~12.7k + GP/FP |
| `snomed_edqm_dose_map` | SNOMED dose-form concept → EDQM code | 328 |
| `snomed_tmt_dose_link` | resolved TMT GP/GPU → SNOMED dose-form (text-match, confidence-gated) | ~TMT GP count |
| `snomed_refset_ingest_runs` | audit trail (+`source_url` provenance) | per-run |

**Design note:** IPS and GP/FP are both *simple refsets* (flat membership) → one table with a `refset_key`, the same idiom that collapsed 10 TMT relationship tables into `tmt_relationships`. Future simple refsets (NCPT/ICNP) add a key, not a table.

**Prerequisite:** `snomed_descriptions` must already hold International FSNs (the IPS Concept file is intentionally 2 rows — concepts live in International). If empty, run `snomed_icd10_map_ingest.py --desc-file <International Description Snapshot>` first.

### A2. Loader — `scripts/snomed_refset_ingest.py` (NEW, reuses RF2 helpers)
Mirrors `snomed_icd10_map_ingest.py` (same `mariadb_exec` / `sql_quote` / `batched_insert`, same `--dry-run`, same `*_ingest_runs` audit pattern). Three independent sub-commands:

- `--ips <der2_Refset_IPSSimpleSnapshot...txt>` → `snomed_refset_members` (`refset_key='ips'`)
- `--gpfp <der2_Refset_GPFPSimpleSnapshot...txt>` → `snomed_refset_members` (`refset_key='gpfp'`)
- `--edqm <der2_ssRefset_EDQMSimpleMapSnapshot...txt>` → `snomed_edqm_dose_map`
- `--tmt-dose-link` → derives `snomed_tmt_dose_link` (text-match pass, see A3)

RF2 parsing rules (all share the simple-refset column layout
`id  effectiveTime  active  moduleId  refsetId  referencedComponentId [  mapTarget  correlationId]`):
- **skip `active != 1`**
- **skip files / lines from `._*`** (macOS AppleDouble on exFAT T7 — the existing loader does NOT guard this; new loader MUST, or RF2 parse breaks)
- Snapshot (not Full) is the ingest source — current-state only.

### A3. The one hard part — TMT → SNOMED dose-form link (`--tmt-dose-link`)
EDQM gives SNOMED-dose-form ↔ EDQM. The missing half is **TMT-dose-form → SNOMED-dose-form**, because TMT stores dose form only as a fragment inside the GP/GPU `fsn`. Resolution pass:
1. Pull SNOMED dose-form concepts: `snomed_descriptions WHERE semantic_tag='(dose form)' OR concept_id IN (SELECT snomed_concept_id FROM snomed_edqm_dose_map)`.
2. For each TMT GP/GPU, extract dose-form fragment from `fsn`, normalize (lowercase, strip strength/qty), match: `exact → normalized → fuzzy`.
3. Write `snomed_tmt_dose_link` with `match_method` + `confidence`; `confidence < 0.85 → needs_review=1` (excluded from automated FHIR coding).
   - **No silent caps:** loader logs counts of exact / normalized / fuzzy / unmatched so coverage is visible, not assumed.

### A4. Wire into bootstrap
Add idempotent steps to `scripts/bootstrap-shared-kbs.sh` (after SNOMED International, before/after TMT) with `--skip-snomed-refsets` flag, so a fresh Mac mini reproduces it.

### A5. Verification
```sql
SELECT refset_key, COUNT(*) FROM snomed_refset_members GROUP BY refset_key;     -- ips ≈ 12679
SELECT COUNT(*) FROM snomed_edqm_dose_map;                                        -- 328
SELECT match_method, COUNT(*) FROM snomed_tmt_dose_link GROUP BY match_method;    -- coverage
SELECT * FROM snomed_refset_ingest_runs ORDER BY started_at DESC LIMIT 4;         -- all status=done
```

---

## Part B — USAGE (นำไปใช้)

### B1. IPS — patient-summary concept gate
The text→concept search (`snomed_descriptions` FULLTEXT, already powering the
SNOMED→ICD-10-TM pipeline) gains an **IPS membership boost/filter**:
```sql
SELECT d.concept_id, d.term,
       EXISTS(SELECT 1 FROM snomed_refset_members m
              WHERE m.refset_key='ips' AND m.concept_id=d.concept_id AND m.active=1) AS in_ips
FROM snomed_descriptions d
WHERE MATCH(d.term) AGAINST (? IN NATURAL LANGUAGE MODE)
ORDER BY in_ips DESC, ...;
```
→ UC2 patient-summary / `Composition` skill prefers IPS concepts → internationally
interoperable summaries instead of arbitrary SNOMED picks. Tag `Composition` sections
with IPS membership for downstream IPS-conformant export.

### B2. GP/FP — primary-care narrowing
Same membership join with `refset_key='gpfp'` → restrict reason-for-encounter / problem
coding to the curated primary-care subset for **MOPH-PC1** flows and Eir primary-care
agents. Reduces over-broad SNOMED matches in the ambulatory setting.

### B3. EDQM — FHIR Medication.doseForm coding
In `extraction_to_fhir_r5.py` medication path:
```
TMT GP/GPU  ──snomed_tmt_dose_link──▶  SNOMED dose-form concept
            ──snomed_edqm_dose_map──▶  EDQM code
→ Medication.doseForm.coding = [ {system: SNOMED, code: <concept>},
                                 {system: EDQM,   code: <edqm_code>} ]
```
Only `needs_review=0` links auto-code; the rest go to the manual review queue.

### B4. Eval / regression (persist to Mimir eval, per project convention)
- IPS coverage: % of UC2 summary concepts that are IPS members (target ↑ over baseline).
- doseForm coverage: % of TMT-coded medications that resolve to an EDQM code (report, don't cap).
- GP/FP precision: spot-check primary-care reason-for-encounter coding vs full International.

---

## Execution checklist
- [x] A0 backup + verify MANIFEST — `~/asgard-backups/shared-kbs/2026-06-02-0009` (MariaDB 33M, gzip OK)
- [x] A1 apply `sprint58_snomed_refsets_edqm.sql` — 4 tables created
- [x] A2 write `snomed_refset_ingest.py` (+ `._` guard)
- [x] A2 dry-run each sub-command — counts matched awk
- [x] A2 ingest IPS (12,353), GP/FP (4,260), EDQM (324)
- [x] A3 TMT→SNOMED dose-link pass — **partial, by design** (see Outcome)
- [x] A4 bootstrap wiring — Phase 8 `--skip-snomed-refsets`
- [x] A5 verification SQL green — `snomed_refset_ingest_runs` all `done`
- [x] B3 EDQM → FHIR `Medication.doseForm` — resolver `scripts/fhir_dose_form.py` + wired into `extraction_to_fhir_r5.py` (2026-06-02)
- [x] B1 IPS → patient-summary concept gate — `knowledge_snomed.rs` search `?refset=ips` boost/filter + `in_refset` flag (2026-06-02)
- [x] B2 GP/FP → primary-care narrowing — same param `?refset=gpfp` (`refset_only` for hard filter)
- [x] B4 persist eval baseline — `scripts/persist_snomed_refset_eval.py` → Mimir eval (tenant asgard_platform), coverage 4/4 floors green
- [x] #6 catalog — `shared_knowledge.rs` SNOMED entry now reports ips/gpfp/edqm/dose-link counts
- [x] #7 TPC — verified populated (3,077 codes via ICD-9-CM baseline, Phase 7); memory consistent, no action

### B1/B2/B4 done (2026-06-02)
`GET/POST /api/v1/knowledge/snomed/search` gains `refset` (`ips`|`gpfp`, allowlisted →
injection-safe) + `refset_only`: members boosted to the top via an EXISTS subquery on
`snomed_refset_members`, each result tagged `in_refset`. `cargo check` clean; boost SQL
verified against DB (e.g. "Asthma (disorder)" flagged in IPS, sorted first). Catalog
(`/api/v1/knowledge/shared`) SNOMED entry now surfaces the Sprint 58 counts. Coverage
eval persisted (`persist_snomed_refset_eval.py`): re-run after any dose-link re-ingest;
fails if trusted links drop below the 8,000 floor. **Note:** Rust changes are
compile-verified — a mimir-api rebuild/redeploy is needed before the live endpoint
serves them.

### B3 done (2026-06-02)
`scripts/fhir_dose_form.py` — `resolve_dose_form(tmt_id, query)` pure/testable + CLI.
Chain `tmt_id → snomed_tmt_dose_link(needs_review=0) → SNOMED concept + snomed_edqm_dose_map`.
Returns a FHIR CodeableConcept `{coding:[SNOMED, EDQM], text}` or `None` (token_subset
links refused — no guessing). Wired into `extraction_to_fhir_r5.py` behind
`FHIR_RESOLVE_DOSEFORM=1` (opt-in, graceful, offline default unchanged): TMT-coded meds
emit a **contained `Medication`** (code=TMT + `doseForm`) referenced by
`MedicationRequest.medication` (R5 CodeableReference). Verified: levothyroxine `1154071`
→ SNOMED `385024007` + EDQM `10106000`; colistin (needs_review) → no doseForm. Bundles
4/5/6 regenerated with doseForm.

## Outcome (2026-06-02)

Ingested under SNOMED Affiliate License, all `tenant_id=NULL` shared:

| Table | Rows | Note |
|---|---|---|
| `snomed_refset_members` (ips) | 12,353 | active IPS members (26 inactive dropped) |
| `snomed_refset_members` (gpfp) | 4,260 | primary-care reasons-for-encounter |
| `snomed_edqm_dose_map` | 324 | active EDQM dose-form maps |
| `snomed_tmt_dose_link` | 8,613 | TMT GP/GPU → SNOMED dose form |

**TMT dose-link coverage (19,855 GP/GPU total) — reported, not silently capped (post-curation 2026-06-02):**
- curated = **4,388** + exact **3,777** + normalized **390** = **8,555 trusted** (`needs_review=0`, auto-codable)
- token_subset = **3,907** proposed, `needs_review=1` (ambiguous ties dropped, not guessed; mostly safe oral-route insertion — a human glance confirms)
- containers (bottle/vial/tube…) = **6,195** excluded — GPU packaging, not dose forms
- unmatched = **1,198** → curation backlog (complex: powder-and-solvent-for-injection, multi-dose inhalers, transdermal patches)

Of the **13,660 real dose forms** (excl. containers): **62.6% auto-codable**, **91.2% have a link** (incl. review). Curated tier (`CURATED_DOSE_ALIASES`, 8 entries) resolves the ambiguous route-omitted forms TMT abbreviates (`tablet`→Oral tablet, `cream`→Cutaneous cream…) that token-matching correctly refused to guess — lifting trusted from 4,167 → 8,555. All curated targets verified EDQM-mapped.

**Key finding:** TMT dose forms follow **EDQM Standard Term** naming
("Film-coated tablet") while SNOMED uses its own ("Film-coated *oral* tablet"), so
there is no exact automatic bridge. Token-subset matching recovers the close variants;
the ~5k under-specified remainder needs a small curated alias map (top ~30 forms cover
most). Only `needs_review=0` links should auto-populate FHIR `Medication.doseForm`.

## Rollback
All tables are new + `CREATE IF NOT EXISTS`; rollback = `DROP TABLE snomed_refset_members, snomed_edqm_dose_map, snomed_tmt_dose_link, snomed_refset_ingest_runs;`. No existing data altered.
