# M1 — Medical Retrieval Benchmark v1.0

Sprint 2 W2.1 deliverable. 75 hand-curated TH/EN medical retrieval queries
for evaluating Mimir RAG over PrimeKG / icd10-th / clinical-wisdom
collections.

## Goal

Measure Hit Rate@3 of Mimir's medical retrieval path across realistic
query patterns a clinician (or patient) would actually issue. Acts as the
canonical baseline for the M1 dataset gate:

```
≥75% Hit Rate@3 → adopt BGE-M3 + current chunking
60-75% → run hybrid (BGE-M3 + sparse exact-match) + benchmark
<60% → fine-tune plan
```

## Item schema

```jsonc
{
  "id": "m1-001",
  "query": "เมตฟอร์มิน",
  "locale": "th",                    // th | en | mixed
  "category": "drug_name",           // 14 categories — see below
  "expected_drug_generics": ["Metformin"],
  "expected_drug_classes": ["..."],      // optional
  "expected_icd_codes": ["E11"],         // optional
  "expected_concepts": ["..."],          // optional, for non-code retrievals
  "expected_NOT_drug_generics": ["..."], // negation queries
  "expected_NOT_drug_classes": ["..."],
  "expected_interaction": true,          // drug_interaction category
  "expected_severity": "major",          // drug_interaction category
  "difficulty": "easy|medium|hard",
  "tags": ["..."],
  "notes": "..."                          // optional human notes
}
```

A query is a **hit** at top-K when the retrieved entities include at
least one of the expected entities (codes, drug generics, etc.) AND
no expected_NOT entries appear in the top-K.

## Categories (75 queries)

| Category | Count | What it tests |
|---|---|---|
| drug_name (TH+EN) | 11 | Direct lookup of generic drug names |
| disease (TH+EN) | 11 | Disease term → ICD-10 code |
| symptom_to_disease | 10 | Symptom cluster → diagnosis |
| drug_synonym | 9 | Brand name → generic mapping |
| drug_disease_relation | 7 | "Which drug for X disease" |
| drug_interaction | 6 | DDI lookup with severity |
| acronym | 4 | T2DM/HTN/OSA → expansion + code |
| sleep_procedure | 4 | CPAP/PSG (Mega Care domain) |
| negation | 4 | "NOT X but Y" — distractor test |
| code_lookup | 3 | ICD-10 code → label (reverse) |
| clinical_scenario | 3 | Multi-condition reasoning |
| sleep_metric | 1 | AHI ≥30 → severe OSA |
| drug_class | 1 | "SGLT2 inhibitor" → drug list |
| clinical_concept | 1 | Free-text concept retrieval |

## Locale distribution

| Locale | Count | % |
|---|---|---|
| EN | 48 | 64% |
| TH | 25 | 33% |
| Mixed | 2 | 3% |

Thai is well-represented (33%) — important because Asgard's Thai-first
positioning depends on Thai retrieval quality matching English. The
baseline cascade showed Thai 100% vs EN 100% on ICD-10 lookup queries
(C.5 benchmark, [2026-05-18]); M1 extends to drug names + clinical
scenarios where Thai performance is less proven.

## Difficulty distribution

| Difficulty | Count |
|---|---|
| Easy | 19 |
| Medium | 36 |
| Hard | 20 |

## Out of scope (deferred to v1.1+)

- **M4 PrimeKG entity linking** — separate dataset; each item = exact
  PrimeKG node ID ground truth. Currently M1 expects drug-name/disease-
  string matches; M4 will pin to graph node IDs.
- **Multi-turn / conversational** — M1 is single-query only.
- **Patient-context-aware** — no patient history fed; queries are
  isolated. Phase D chat panel can extend this.
- **Image-based queries** — text only.

## Usage

The dataset is registered in Mimir's `eval_benchmark_datasets` via the
W2.1c-style registration script. Run M1 against any retrieval path:

```bash
# Example: against Mimir API /api/v1/search (when API up)
curl -X POST http://mimir/api/v1/search \
    -H "Authorization: Bearer $TOKEN" \
    -d '{"query": "...", "collection": "primekg-entities", "top_k": 3}'

# Or direct Qdrant search (per scripts/c5_baseline_retrieval.py pattern)
```

## Refs

- Sprint 2 W2.1 tracker: `Asgard/docs/sprint_tracker_2026_05_17.md`
- Dataset plan: `docs/04_evaluation_and_testing/04_10_dataset_inventory_plan_2026-05-17.md`
- C.5 baseline cascade results: `docs/04_evaluation_and_testing/results/c5_cascade_2026_05_18_k3.json`
- Origin queries (Sprint 48 v0): `tests/icd10/sprint48_thai_lookup_v0.jsonl`
