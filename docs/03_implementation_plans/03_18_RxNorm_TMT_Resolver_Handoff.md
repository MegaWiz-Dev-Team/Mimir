# 03_18 — RxNorm/TMT Resolver: status & handoff

**Status:** merged to `main` (PRs #390, #391). Companion to the design doc
[`03_17_RxNorm_TMT_Crosswalk_Resolver_v1.md`](03_17_RxNorm_TMT_Crosswalk_Resolver_v1.md)
and the v1 mechanism doc `ro-ai-domain-medical/docs/NORMALIZER.md`. This is the "what
shipped / what's left" record for whoever picks the resolver up next.

The resolver normalizes a drug/disease name as written (brand, Thai trade, cross-region
generic) to the PrimeKG-canonical form the clinical medication-safety pruner resolves
against — because entity resolution, not the graph query, is the dominant failure mode.

## 1. What's merged on `main`

| PR | what | key commits |
|----|------|-------------|
| **#390** | full RxNorm crosswalk + TMT Thai lane + UNII→PrimeKG bridge + PrimeKG version pin | `7cd5a19` Lane B · `7c15aaa` scaffolding · `68b18fe` PrimeKG :Meta · `c8449c8` UNII fix · `2b48ab6` full table+alias · `373b637` bridge |
| **#391** | RxNorm row in `/api/v1/knowledge/shared` catalog | `3d8a1a9` |

Rebase-merged (repo disallows squash), built on the pruner's #388 (`2637d17`).

## 2. Data

- **MariaDB `mimir`** (migration `sprint55_rxnorm_crosswalk.sql`), `tenant_id IS NULL`:
  - `rxnorm_atoms` 282,622 · `rxnorm_rel` 849,452 · `rxnorm_unii` 10,248 (every row has `drugbank_id`)
  - `rxnorm_primekg_map` — **empty** (offline coverage only computed it — see §5)
  - `rxnorm_ingest_runs` — provenance; `source_version = rxnorm-20260706`, sha256, status DONE
- **Shipped static TSVs** (`include_str!` into the normalizer; force-added past the `data/` ignore):
  - `ro-ai-domain-medical/data/rxnorm_brand_ingredient.tsv` — **9,162** brands (was 57)
  - `ro-ai-domain-medical/data/us_inn_alias.tsv` — **11,264** aliases (was 2)
  - `ro-ai-domain-medical/data/tmt_thai_generic.tsv` — **21,100** Thai brands
- **Licensed source / caches** (on the T7 Shield dev drive, NOT in git):
  - RxNorm Full 07/2026: `asgard-data/terminology/rxnorm/` (zip + `rrf/`)
  - PrimeKG: `asgard-data/mimir-kb/PrimeKG/kg.csv` + derived `primekg_drugs.tsv` (7,957 drug nodes)

## 3. How to regenerate

- **Ingest** (needs a RxNorm Full release, free UMLS/UTS acct): `scripts/rxnorm_ingest.py --rrf-dir <rrf> --source-version rxnorm-YYYYMMDD`. Note UNII is `ATN='FDA_UNII_CODE'` (not `UNII`).
- **Brand→ingredient TSV**: `ro-ai-domain-medical/scripts/build_rxnorm_table.py --from-db --primekg-names primekg_drugs.tsv` — orders ingredients so PrimeKG's preferred name is first; loader takes the first of a `;`-combo.
- **Alias TSV**: `build_rxnorm_table.py --build-alias --primekg-names primekg_drugs.tsv` — every RxNorm synonym → PrimeKG name via `rxcui→UNII/DrugBank-id→primekg_drugs`.
- **Thai TSV**: `ro-ai-domain-medical/scripts/build_tmt_table.py` — climbs TMT dm+d (TP←GP←VTM).
- **Bridge coverage** (offline, no Neo4j): `scripts/rxnorm_primekg_bridge.py --source-version rxnorm-20260706 --primekg-drugs primekg_drugs.tsv --dry-run`.
- **Normalizer pipeline** (`ro-ai-domain-medical::normalizer::DrugDiseaseNormalizer`): `TMT thai→generic (layer-0) → RxNorm brand→ingredient → generalized alias → PrimeKG resolve`; `parse_tsv` takes the first `;`-segment.
- **PrimeKG version pin**: `ro-ai-bridge/migrations/neo4j/primekg_meta.cypher` (idempotent MERGE) + `Neo4jService::primekg_meta_version()`; the catalog reads it (falls back to `"primekg-v2"`).

## 4. Findings + numbers

- **UNII discovery**: in the release, UNII is `ATN='FDA_UNII_CODE'`, 100% `SAB=DRUGBANK`, and its `CODE` field is the **DrugBank id** → RxNorm gives `RXCUI→UNII→DrugBank-id` directly. PrimeKG nodes are keyed by DrugBank id, so the bridge needs **no external DrugBank vocabulary** (closes NORMALIZER.md §4).
- **Non-overfit resolution: 33/35 = 94.3%** on brands NONE of which are in the seed (exact-match vs PrimeKG nodes, offline via kg.csv). Misses = combo `+`, salt naming.
- **Bridge coverage: 4,882/7,957 = 61.4%** of PrimeKG drug nodes — `drugbank_id` 54.8% alone, `name` 6.4%, `inn_syn` 0.1%. Residual = experimental/withdrawn/IUPAC compounds absent from clinical RxNorm.
- 6/6 normalizer tests pass; `mimir-core-ai` + `ro-ai-bridge` cargo-check clean.

## 5. Not done / open

- **`rxnorm_primekg_map` is empty** — offline coverage matched by name/drugbank_id but couldn't fetch `entity_index` (Neo4j-assigned). Populate with a **live Neo4j run** (drop `--primekg-drugs` + `--dry-run`): port-forward `kubectl port-forward -n asgard-infra svc/neo4j 7687:7687`, creds in `ro-ai-bridge/.env` (Neo4j pw is in k8s secret `neo4j-secret`).
- **`primekg_meta.cypher` not yet run** on the graph → the catalog shows the fallback `"primekg-v2"` until it is.
- **`resolution_bench`** not re-run against live PrimeKG with the full table (RAW-vs-NORMALIZED regression). Offline non-overfit already = 94.3%.
- **Combination drugs**: loader takes the first ingredient; RxNorm combos use `;`, TMT combos use ` + ` (not split). Multi-ingredient safety check is open.
- **Disease axis** ~55% — separate workstream; use `POST /knowledge/primekg/resolve` (full chain), not the naive `entity` lookup.

## 6. License lanes

| source | rule |
|--------|------|
| RxNorm (SAB=RXNORM core) | **public domain → ships** |
| UNII (FDA) | public domain |
| DrugBank | **IDs / crosswalk keys only** (used); curated **content** barred; DDInter eval-only |
| TMT | MoPH / THIS-Center — free in TH; **confirm redistribution terms before shipping to a commercial customer** |

## 7. Gotchas

- `primekg_drugs.tsv` was extracted with `awk -F','`, which doesn't handle quoted commas in kg.csv → a few malformed drug-name rows slightly inflate the residual (the `drugbank_id` tier keys on the clean id column and is unaffected). A proper CSV parse would tidy the name/inn tiers.
- The dev checkout `Mimir-normalizer` was an ephemeral clone that got wiped mid-work; the work was recovered via a dedicated worktree and is now plain history on `main`. Work in a fresh worktree off `main`; never bare-commit the shared live checkout.
