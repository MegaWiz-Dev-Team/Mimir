# drug-disease-resolution-v1

Measures how often the **naive PrimeKG resolver** (`Neo4jService::primekg_lookup_entity`)
finds the **intended** node for a drug/disease name as an LLM/clinician actually
writes it (generic, brand, lay phrasing). Regression gate for the drug/disease
**normalizer** that the clinical safety pruner depends on.

- **tenant:** `asgard_medical`
- **scoring_fn:** `resolution_recall` — bucket the top-1 match by tier:
  `exact (rank 0)` > `prefix (rank 1)` > `substr (rank 2 = SUSPECT)` > `miss`.
  Headline = **exact_rate per type** + **labeled correctness** (vs `expected`).
  "returned anything" overstates recall — always report the exact floor.

## Files
- `probe.jsonl` — 65 terms. Each: `{id, term, type(drug|disease), category, locale, gold, expected?}`.
  `expected` = verified canonical PrimeKG node (13 gold-labeled).
- `results_baseline.json` — naive resolver baseline, **prototype** (2026-07-23),
  NOT a validated production-pruner run.

## Baseline (naive resolver, live vs PrimeKG 129,375 nodes)
drug exact 73.3% / miss 24.4% · disease exact 45% / prefix 35% · labeled 9/13 = 69%.
100% of brand names fail (coumadin/glucophage/viagra → miss); cross-region generics
miss (paracetamol→Acetaminophen, albuterol→Salbutamol); aspirin→Nitroaspirin.

## Provenance / license
Terms are authored realistic clinical vocabulary — **not** sampled from PrimeKG
(that would be exact by construction). This dataset holds only names + expected
canonical nodes. **DDInter/DrugBank interaction ground truth is benchmark-only and
must never be stored here or in the product KG.**

## Reproduce
`resolution_bench` (scratchpad `ddi-safety-bench`) against a port-forwarded Neo4j.
