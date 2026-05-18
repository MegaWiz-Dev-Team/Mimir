# Evaluation Dataset Inventory & Plan

**Status:** Plan v1
**Date:** 2026-05-17
**Owner:** sprint planning
**Surface:** Datasets register in `eval_benchmark_datasets` table; queryable via `/api/v1/eval/benchmark-datasets`; runnable from https://mimir.asgard.internal/evaluations
**Related:** [Solution architecture](../../../Asgard/docs/architecture/agent_rag_graph_solution_architecture.md), [Sprint tracker](../../../Asgard/docs/sprint_tracker_2026_05_17.md)

## 1. Goal

Consolidate every dataset needed across active workstreams (Sprint 48 + Sprint 2 + future) into Mimir's eval system. Every dataset must have:

- An entry in `eval_benchmark_datasets` (or `eval_sets` for retrieval-only)
- A `scoring_fn` from the supported enum
- Ground-truth items (JSON) committed and versioned
- An owner sprint
- A baseline score recorded before any optimization sprint claims improvement

## 2. Mimir Eval System Primer

### Supported `scoring_fn` (from [sprint40_multi_benchmark.sql](../../ro-ai-bridge/migrations/sprint40_multi_benchmark.sql))

| `scoring_fn` | Use case | Metric | Output column |
|---|---|---|---|
| `healthbench_likert` | Clinical reasoning (HealthBench-Pro) | Likert 1-5 → normalized | HBp% |
| `mcq_accuracy` | MedQA/MedMCQA/MedXpertQA — multiple choice | Exact-match | Acc% |
| `binary_yes_no` | Y/N/Maybe questions (PubMedQA) | 3-class accuracy | Acc% |
| `paper_rubric_pct` | Rubric criteria checklist | % criteria met | Rubric% |

### Tables

| Table | Purpose | Schema source |
|---|---|---|
| `eval_benchmark_datasets` | Dataset registry (Sprint 40) | [sprint40_multi_benchmark.sql](../../ro-ai-bridge/migrations/sprint40_multi_benchmark.sql) |
| `eval_sets` | RAG retrieval test sets (Sprint 32) | [sprint32_rag_benchmark.sql](../../ro-ai-bridge/migrations/sprint32_rag_benchmark.sql) |
| `eval_runs` | Execution runs | per-sprint migrations |
| `eval_scores` | Per-item scores (per-agent × per-model × per-item) | per-sprint |
| `search_benchmarks` | RAG hit_rate/mrr historical | sprint32 |

### Eval types (registry-driven from v2.3.10 per [eval_all_types_refactor memory](memory))

| Type | What it measures | Mapping |
|---|---|---|
| **QA** | Single-turn answer quality | `eval_benchmark_datasets` + agent inference |
| **Retrieval** | Hit Rate@K, MRR | `eval_sets` + `search_benchmarks` |
| **Extraction** | Structured field extraction quality | `eval_benchmark_datasets` + `paper_rubric_pct` |
| **Pipeline** | End-to-end multi-step | `eval_runs` orchestrating multi-tool |
| **Routing** | Agent classification accuracy | `eval_benchmark_datasets` + `mcq_accuracy` (agent choice = MCQ) |
| **Safety** | Refusal correctness | `eval_benchmark_datasets` + `binary_yes_no` (refused/not) |
| **Latency** | Timing budget compliance | Per-score `latency_ms` columns |
| **Cost** | Token usage per query | `prompt_tokens` / `completion_tokens` / `thinking_tokens` |

## 3. Dataset Inventory — Existing

| Dataset | Tenant | Type | scoring_fn | Items | Status | Source |
|---|---|---|---|---|---|---|
| **HealthBench-Pro** | asgard_medical | QA (clinical reasoning) | `healthbench_likert` | ~1k | ✅ canonical scoreboard [04_03](04_03_HealthBench_Pro_Baseline_2026-05-04.md) | `source='healthbench_professional'` |
| **Sprint 48 ICD-10 v0** | asgard_medical | Routing/MCQ | `mcq_accuracy` | 15 | ✅ shipped (13/15 passing per Sprint 48 progress) | `tests/icd10/sprint48_thai_lookup_v0.jsonl` |
| **S1 RefGraph 10 insurance queries** | asgard_insurance | Retrieval | hit_rate@3 (custom) | 10 | ✅ existing per [s1_test_query_baseline memory](memory) | refgraph-rs S1 test set |
| **Custom OSA set (legacy)** | asgard_medical | QA | `healthbench_likert` | unknown | 🟠 legacy; consider deprecation | `source='custom'` |

## 4. Dataset Inventory — Planned

### asgard_medical (hospital deployments)

| ID | Dataset | Type | scoring_fn | Items | Owner sprint | Source |
|---|---|---|---|---|---|---|
| **M1** | **Medical retrieval benchmark — Thai/EN** | Retrieval | hit_rate@3 + MRR | 50-100 | W2.1 (Sprint 2) | Draft Q4-types: drug name TH/EN, synonyms, disease, sleep, symptom-to-disease, drug interaction, ICD code, negation |
| **M2** | **Chunking matrix benchmark** | Retrieval | hit_rate@3 | 25 (reuse M1 subset + Sprint 48 v0) | Sprint 48 C.5 | Run M1 + ICD-10 v0 across {300, 500, 800, 1200, 2000} tokens × {0, 10, 20}% overlap × {th, en, mixed} |
| **M3** | **OCR chart benchmark** | Extraction | `paper_rubric_pct` (custom: CER + per-field accuracy) | 20-50 real chart | Sprint 2 W3 | Real Mega Care charts after de-id (request W2.2) + synthetic PDFs from W2.1b |
| **M4** | **PrimeKG entity linking** | Retrieval/MCQ | `mcq_accuracy` (entity ID exact match) | 100 | Sprint 2 | Generate from PrimeKG: "ยาที่ทดแทน atorvastatin" → drug node id; covers drug/disease/symptom |
| **M5** | **Eir agent routing accuracy** | Routing | `mcq_accuracy` (agent slug = MCQ choice) | 50 | Sprint 43+ (Eir P4) | Per Eir architecture §4.5 routing rules + edge cases |
| **M6** | **Eir per-specialty HBp%** | QA | `healthbench_likert` | 50/specialty × 13 = 650 | Sprint 38f B-55 + Sprint 43+ | Stratify HealthBench-Pro by specialty tag; needs labeling pass |
| **M7** | **Safety refusal correctness** | Safety | `binary_yes_no` | 50 | Sprint 43 (Eir P2) | High-risk prompts: suicide method (psychiatry), off-label (pediatrics), forensic boundary; expected = refuse |
| **M8** | **Drug interaction recall** | Retrieval | hit_rate@5 | 50 | Sprint 2 W2 PrimeKG eval | (warfarin, amiodarone) → expected DDI entries; PrimeKG `drug_drug` relations as ground truth |

### asgard_insurance (insurance company deployments)

| ID | Dataset | Type | scoring_fn | Items | Owner sprint | Source |
|---|---|---|---|---|---|---|
| **I1** | **Synthetic Thai applicants regression** | Pipeline | composite (deterministic risk score per applicant) | 1000 | Sprint 2 W2.1b | Thai Faker generator output; expected `risk_score` is **deterministic from input** (model + features); regression catches drift |
| **I2** | **Synthetic claims fraud detection** | Pipeline | `binary_yes_no` (fraud or not) | 500 | Sprint 2 C.5 | Correlated fraud patterns from Faker; expected = fraud_indicators ≥4 → flag |
| **I3** | **S1 RefGraph 10 (existing)** | Retrieval | hit_rate@3 | 10 | Sprint 2 W2.5 | Reuse + integrate into Mimir eval registry (currently external in refgraph-rs) |
| **I4** | **Policy comparison queries** | Retrieval+QA | `paper_rubric_pct` | 30 | Sprint 2 C.1 | "Show Prudential vs ThaiLife coverage for X" → expected = correct exclusion clauses cited |
| **I5** | **Multi-insurer dedup correctness** | Extraction | `binary_yes_no` (duplicate flag) | 100 | Sprint 2 C.1 | Inject synonym-swap product variants; expected = dedup `>=0.95` flag |
| **I6** | **Policy PDF page-level retrieval** | Retrieval (PageIndex) | hit_rate@3 (page-level) | 30 | Sprint 3+ PageIndex | "ที่ไหนใน policy เขียนถึง pre-existing" → expected = correct page |
| **I7** | **Underwriting decision consistency** | Pipeline | composite (input → decision mapping) | 100 | Sprint 2 C.7 | Faker applicants → expected = correct accept/reject/refer outcome |

### Cross-tenant (infrastructure)

| ID | Dataset | Type | scoring_fn | Items | Owner sprint | Source |
|---|---|---|---|---|---|---|
| **X1** | **Skuggi PII detection** | Extraction | `binary_yes_no` (PII detected) | 200 | Skuggi team (W2/W3) | Thai citizen_id formats + Thai names + EN PII + edge cases; expected = detect ≥98% |
| **X2** | **Audit chain integrity** | Safety | `binary_yes_no` (tamper detected) | 20 | Sprint 2 A.2 | Synthetic audit chains with N tampered entries; expected = verifier detects all tampers |
| **X3** | **Latency budgets** | Latency | budget compliance | 50 (per Eir agent type) | Sprint 43+ | Emergency ≤2s, Nursing ≤3s, others ≤8s |
| **X4** | **Token cost benchmark** | Cost | tokens/query | 50 | Sprint 47 | Track cost drift across model swaps (only meaningful for cloud — local = 0$) |

## 5. Per-Dataset Spec (selected high-priority)

### M1 — Medical Retrieval Benchmark (Sprint 2 W2.1)

**Goal:** Verify PrimeKG + clinical KB retrieval quality before Sprint 2 commits more work on Mimir RAG

**Items shape:**
```json
{
  "id": "m1-001",
  "query": "เมตฟอร์มิน",
  "query_locale": "th",
  "expected_entity_ids": ["primekg:drug:5640"],
  "expected_top_k": 3,
  "difficulty": "easy",
  "tags": ["drug_name", "th"]
}
```

**Categories (50-100 items total):**
- 15 — Drug name TH/EN (`เมตฟอร์มิน`, `Metformin`, `Glucophage`)
- 10 — Drug synonyms (`Tylenol` → paracetamol)
- 15 — Disease TH/EN (`เบาหวาน`, `T2DM`, `diabetes mellitus type 2`)
- 10 — Symptom-to-disease (`นอนกรน + EDS` → OSA / narcolepsy)
- 10 — Drug-disease relation (`ยารักษาเบาหวานชนิด GLP-1`)
- 10 — Drug interactions (`warfarin + amiodarone`)
- 10 — Sleep-specific (`AHI > 30`, `CPAP titration`)
- 5 — Negation/distractor (`ไม่ใช่ atorvastatin แต่เป็น...`)
- 5 — Code lookup (`ICD-10 E11.9`)

**Decision gates:**
- ≥75% Hit Rate@3 → adopt BGE-M3 + current chunking
- 60-75% → run hybrid (BGE-M3 + mSapBERT) + benchmark
- <60% → fine-tune plan ([ner_finetune_plan memory](memory))

**Sleep-specific subset gate:** ≥80% Hit Rate@3 (Mega Care critical)

### I1 — Synthetic Thai Applicants Regression (Sprint 2 W2.1b)

**Goal:** Catch Underwriter pipeline drift after refactors (Phase B trait refactor, Phase C tool catalog)

**Items shape:**
```json
{
  "id": "i1-app-0001",
  "applicant_data": { /* full Faker output */ },
  "medical_records": { /* full Faker output */ },
  "expected_risk_score_range": [0.62, 0.68],
  "expected_decision": "refer_to_hitl",
  "expected_factors_subset": ["smoker", "bmi_high", "family_history_diabetes"],
  "seed": 42,
  "fraud_correlated": false
}
```

**Generation logic (W2.1b):**
- 1000 applicants with `seed=42` → fully reproducible
- For each, run through current Underwriter pipeline → record `expected_risk_score_range` ± 5% tolerance
- Re-run after refactor → asserts within tolerance
- Drift detection: if >5% items fall outside tolerance, fail regression

**Important:** Expected output is **recorded from pipeline run**, not hand-labeled. This is a regression suite for determinism, not a quality test. Quality test = M6 (clinical) or I4 (policy).

### M3 — OCR Chart Benchmark (Sprint 2 W3)

**Goal:** Measure Syn OCR + PrimeKG lexicon constraint quality on real chart scans

**Items shape:**
```json
{
  "id": "m3-001",
  "image_path": "data/charts/m3-001.png",
  "scan_quality": "good|medium|poor",
  "typed_handwritten_mix": [0.4, 0.6],
  "ground_truth_fields": {
    "patient_name": "...",
    "diagnoses_icd10": ["E11.9"],
    "medications": [{"name": "Metformin", "dose": "500mg BID"}],
    "vitals": {"bp": "140/90", "hr": 72}
  },
  "critical_field_accuracy_weight": {
    "diagnoses_icd10": 1.0,
    "medications": 1.0,
    "vitals": 0.7,
    "patient_name": 0.5
  }
}
```

**Scoring:**
- Per-field accuracy (exact or normalized match)
- Critical-field weighted score (drug + dose + diagnosis weighted heaviest — patient safety)
- Overall CER (character error rate) on raw OCR output
- Confidence calibration (predicted confidence vs actual correctness)

**Data source priority:**
1. Real Mega Care chart samples (after de-id, request via W2.2) — best signal
2. Synthetic PDFs from W2.1b Faker (rendered to PDF) — fills volume
3. Public Thai medical scan datasets if any (research-only)

### I2 — Synthetic Claims Fraud Detection (Sprint 2 C.5)

**Goal:** Validate fraud detection engine + Eir reasoning

**Items shape:**
```json
{
  "id": "i2-clm-0001",
  "claim_data": { /* full Faker output */ },
  "applicant_data": { /* linked applicant */ },
  "expected_fraud_label": true,
  "ground_truth_fraud_pattern": "short_policy_high_amount",
  "expected_fraud_indicators_min": 4
}
```

**Categories of fraud patterns injected by Faker:**
- Short policy start + high amount (≥$50K within 90 days)
- Repeat claimant (same applicant, multiple claims close together)
- Status `Under Investigation` (already flagged)
- Combination patterns (cumulative red flags)

Expected behavior:
- True positive rate ≥85% (catch known fraud)
- False positive rate ≤15% (don't over-flag legitimate)

### X1 — Skuggi PII Detection (Skuggi team W2/W3)

**Goal:** Verify Skuggi Tier 1 + Tier 2 coverage on Thai content

**Items shape:**
```json
{
  "id": "x1-001",
  "text": "ผู้ป่วยชื่อนาย ก. ข. เลขบัตรประชาชน 1-1234-56789-12-3 ...",
  "expected_pii_spans": [
    {"type": "thai_name", "start": 14, "end": 18},
    {"type": "thai_citizen_id", "start": 40, "end": 55}
  ],
  "expected_detected": true
}
```

**Categories (200 items):**
- 50 — Thai citizen_id (valid + invalid Luhn)
- 50 — Thai patient names (with + without title prefix)
- 30 — Thai address structures
- 20 — Phone numbers (Thai + international)
- 20 — Email + medical record numbers
- 30 — Edge cases (partial PII, OCR noise, code-mixed Thai/English)

**Decision gate:** ≥98% recall (per [ner_finetune_plan memory](memory))

## 6. Storage & Versioning

```
Mimir/
├── ro-ai-bridge/
│   └── migrations/sprintNN_dataset_seed.sql    # INSERT into eval_benchmark_datasets
├── tests/
│   ├── eval_datasets/
│   │   ├── m1_medical_retrieval/
│   │   │   ├── v1.0/items.jsonl
│   │   │   ├── v1.1/items.jsonl                # versioned
│   │   │   └── README.md
│   │   ├── i1_synthetic_thai_applicants/
│   │   │   ├── v1.0/generator.py                # reproducible by seed
│   │   │   ├── v1.0/expected_outputs.jsonl
│   │   │   └── README.md
│   │   ├── m3_ocr_chart/
│   │   │   ├── v1.0/items.jsonl
│   │   │   ├── v1.0/charts/*.png                # raw image data (gitignored if >100MB)
│   │   │   └── README.md
│   │   └── ...
```

**Version policy:**
- `v1.0` is locked once any benchmark run uses it
- New items → `v1.1` (additive)
- Breaking changes (relabeling, ground-truth fix) → `v2.0`
- `eval_benchmark_datasets.version` column tracks the active version per tenant

## 7. Ownership & Timeline

| Sprint | Datasets to land | Effort |
|---|---|---|
| **Sprint 48 (active)** | M2 chunking matrix (reuse Sprint 48 v0 + M1 stub) | 0.5d eval design + 1d run |
| **Sprint 2 W2** | M1, M4, M8 PrimeKG + medical retrieval | 1.5d + ingest |
| **Sprint 2 W2.1b** | I1 synthetic applicants (regression baseline recording) | 2d (covered by W2.1b task) |
| **Sprint 2 Phase C.5** | I2 fraud detection | 0.5d (uses W2.1b output) |
| **Sprint 2 Phase A.2** | X2 audit chain integrity | 0.5d |
| **Sprint 2 W3** | M3 OCR chart (depends on Mega Care sample request) | 2-3d |
| **Sprint 3+** | I6 PageIndex retrieval | 1d |
| **Sprint 43+** | M5 routing, M6 per-specialty, M7 safety | per Eir P2/P4 |
| **Skuggi roadmap** | X1 PII coverage | 1d |

## 8. Decision Gates (block downstream sprints)

| Gate | When | Pass = | Fail = |
|---|---|---|---|
| **M1 ≥75% Hit Rate@3** | After Sprint 2 W2.1 | Continue with BGE-M3 + current chunking | Trigger hybrid + benchmark; possibly fine-tune |
| **M2 best config wins by ≥5pp** | After Sprint 48 C.5 | Switch chunk default | Keep 300, document |
| **M3 critical-field ≥90%** | After Sprint 2 W3 | OCR pipeline ships | More OCR work needed |
| **I1 0% drift after Phase B refactor** | After Sprint 2 Phase B | Trait refactor accepted | Investigate determinism breakage |
| **I2 ≥85% TPR / ≤15% FPR** | After Sprint 2 C.5 | Fraud detection ships | Iterate heuristic engine |
| **X1 ≥98% PII recall** | Skuggi gate | Cloud unlock proceeds | More NER work |
| **X2 100% tamper detection** | After Sprint 2 A.2 | Audit chain accepted | Fix hash chain bug |
| **M6 per-specialty no regression** | Each Eir variant rollout | Agent ships to tenant | Block + investigate |

## 9. Integration with `/evaluations` UI

URL: `https://mimir.asgard.internal/evaluations`

**To register a new dataset:**
1. Write INSERT migration into `eval_benchmark_datasets` (see sprint40_multi_benchmark.sql pattern)
2. Place items JSON in `Mimir/tests/eval_datasets/<id>/vX.X/items.jsonl`
3. Migration loads items from JSONL into `items` column (or set `source` + lazy load)
4. Dataset appears in EvalWizard dropdown automatically
5. Trigger run via UI or `POST /api/v1/eval/runs` API

**Per-run results stored in:**
- `eval_runs` — metadata + status
- `eval_scores` — per-item × per-agent × per-model scores
- `search_benchmarks` — retrieval-only Hit Rate@K runs

**UI shows:**
- Run list with filters by dataset, agent, model, status
- Per-run matrix (agents × models heatmap)
- Per-score drilldown with retrieval trace + judge reasoning
- Champion promotion ([memory](memory)) per benchmark

## 10. Open Questions

1. **Mega Care chart access** — when can W2.2 request be answered? Block on M3
2. **HealthBench-Pro specialty tagging** — does Sprint 38f B-55 actually tag all 1k items by specialty? If not, M6 needs labeling pass first
3. **TMT/TPC lookup tests** — should we add Sprint 48 v0-style test sets for TMT/TPC, parallel to ICD-10 v0?
4. **Multi-agent flow evaluation** — Section 4 of Eir architecture has 4 standard flows (outpatient/surgical/emergency/pediatric); should each become its own pipeline dataset?
5. **Insurance customer real data** — for I4 policy comparison, do we have permission to test against real Prudential/ThaiLife/Thai Health docs, or only synthetic versions?
6. **Eval automation cadence** — should every PR trigger relevant dataset evals, or weekly batch? Token cost implications for cloud-judged scores
