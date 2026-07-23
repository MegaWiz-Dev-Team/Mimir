# Drug/Disease Name Normalizer — design & runbook

**Audience:** engineers extending the clinical safety pruner.
**TL;DR:** The pruner can only check a drug/disease it can *resolve* to a PrimeKG
node. Names as clinicians/LLMs write them (brands, lay terms) don't match PrimeKG
canonical names, so **resolution — not the graph query — is the dominant failure
mode**. The normalizer maps any written name → PrimeKG-canonical before resolving.
Primary source is **RxNorm (public domain, ships)**; DDInter/DrugBank are *not*
usable in the product (license).

---

## 1. The problem (measured, not assumed)

We ran `resolution_bench` against the live PrimeKG (129,375 nodes) with the naive
resolver `Neo4jService::primekg_lookup_entity` (exact > prefix > substring):

| type | exact (trustworthy) | miss | labeled correctness |
|------|--------------------|------|---------------------|
| drug | 73.3% | **24.4%** | **9/13 (69%)** |
| disease | 45% | 10% | — |

**100% of brand names failed** (`coumadin`, `glucophage`, `viagra`, `ventolin`,
`lasix`, `augmentin`, `advil` → *miss*), and `aspirin` mis-resolved to
`Nitroaspirin` (PrimeKG's canonical is `Acetylsalicylic acid`). A missed
resolution is a **silent false-negative**: the pruner never checks the drug, so a
real interaction is not flagged. In a clinical guardrail that is the worst class
of error.

Why brands miss: PrimeKG uses **DrugBank canonical names**. `coumadin` is nowhere
in that vocabulary — the node is `Warfarin`. Substring search on "coumadin" finds
nothing; substring on "aspirin" finds `Nitroaspirin` (wrong drug). No amount of
graph tuning fixes this — the fix is upstream, at name resolution.

## 2. The pipeline

```
  written name  ("Coumadin", "high blood pressure", "ventolin")
      │  lowercase + trim
      ▼
  ┌─────────────────────────────┐
  │ 1. RxNorm brand → ingredient│  static table, built dev-time from RxNav
  │    coumadin → warfarin       │  (RxNorm = public domain → ships)
  │    ventolin → albuterol      │
  └─────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────┐
  │ 2. US ↔ INN alias           │  small static table for the divergent cases
  │    albuterol → salbutamol    │  (RxNorm gives US generic; PrimeKG/DrugBank
  │    (acetaminophen already ok)│   sometimes uses the INN name)
  └─────────────────────────────┘
      │
      ▼
  ┌─────────────────────────────┐
  │ 3. lay/disease overrides    │  hand-curated edge cases
  │    high blood pressure       │  (heart attack → myocardial infarction, …)
  │      → hypertension          │
  └─────────────────────────────┘
      │  canonical candidate
      ▼
  ┌─────────────────────────────┐
  │ 4. PrimeKG resolve          │  primekg_lookup_entity: exact > prefix > substr
  │    → entity_index + name     │  (unchanged; now fed a canonical name)
  └─────────────────────────────┘
```

Each layer is a plain map lookup (O(1)); the only network/DB hit is layer 4
(already there). Layers 1–3 are static tables shipped with the product.

## 3. Data sources & license lanes (this is the crux)

| source | what it gives | license | can it ship in Asgard? |
|--------|--------------|---------|------------------------|
| **RxNorm** (RxNav API / UMLS) | brand→ingredient, synonyms, RXCUI | **public domain** (US gov) | ✅ **yes — primary source** |
| **DDInter 2.0** | DDI + **severity** | CC BY-NC-SA (**non-commercial**) | ❌ **eval-benchmark only, firewalled** |
| **DrugBank / FDB / Lexicomp** | names, severity | **commercial** | ❌ avoid unless licensed |
| **ONC high-priority DDI** (JAMIA 2012) | 15 contraindicated pairs | published consensus | ✅ (used by the *severity* gate, not here) |

**Rule:** the normalizer must be buildable from **public-domain** data only, so it
can ship. RxNorm satisfies this. DDInter's severity is for the *eval* lane
(measuring the severity gate), never baked into product tables.

RxNav proof (live, 8/8 previously-missing brands):
```
coumadin→warfarin  glucophage→metformin  viagra→sildenafil  ventolin→albuterol
lasix→furosemide   tylenol→acetaminophen advil→ibuprofen    augmentin→amoxicillin(+clavulanate)
```

## 4. The US ↔ INN gap (don't skip this)

RxNorm ingredient names are **US generic**. PrimeKG (DrugBank) is **inconsistent**:
some nodes use the US name (`Acetaminophen`), some the INN (`Salbutamol`, not
`Albuterol`). So the chain can be:

```
ventolin →(RxNorm)→ albuterol →(needs INN alias)→ salbutamol →(PrimeKG)→ node
```

Layer 2 is a **small** curated US↔INN alias table (albuterol↔salbutamol,
meperidine↔pethidine, epinephrine↔adrenaline, …). Keep it small and test each
target actually resolves in PrimeKG (the bench catches misses). A fuller fix is
to map RxNorm → DrugBank ID → PrimeKG `entity_id`, but that needs a DrugBank
cross-map; the alias table covers the handful of divergent names today.

## 5. Building the table (dev-time, NOT runtime)

Asgard is local/offline-first — **do not call RxNav at request time.** Build the
static table as a batch job and commit it:

```
python3 build_rxnorm_table.py     # queries RxNav, writes data/rxnorm_brand_ingredient.tsv
```

- The demo table covers common brands. The **full** table (~30k RxNorm brand
  names) is a follow-up: either enumerate `getAllConcepts?tty=BN` via RxNav
  (~30k × 2 calls @ 20 req/s) or download RxNorm Full (free UMLS account) and read
  `RXNCONSO.RRF` / `RXNREL.RRF` (BN→IN relations).
- `log()` what the build covered; never silently ship a partial table as if
  complete.

## 6. Measurement — the regression gate

`resolution_bench` runs each probe term **RAW vs NORMALIZED** and reports the
bucket deltas. The dataset `drug-disease-resolution-v1` is registered in
`eval_benchmark_datasets` (tenant `asgard_medical`) and **is the regression gate**:
any normalizer change re-runs it.

Seed-map result (proves the mechanism):

| | exact raw → norm | miss raw → norm | labeled |
|---|---|---|---|
| drug | 73.3% → **100%** | 24.4% → **0%** | 69% → **100%** |
| disease | 45% → 55% | 10% → 0% | — |

**Non-overfit rule:** the seed map and probe were authored together, so 100% is
partly by construction. Validate on brands **not** used to build the table (RxNorm
is a general source, so this is a real generalization test). Add new brands to the
probe and confirm the exact-rate holds.

## 7. Integration

- Module: `ro-ai-domain-medical::normalizer::DrugDiseaseNormalizer`.
- Consumed by `safety_pruner::PrimeKgPruner::resolve()` (normalize → `primekg_lookup_entity`).
- Exposed via `POST /api/v1/medical/medication-safety/check`.
- `DrugDiseaseNormalizer::seed()` (current) → replace with a loader that reads the
  committed static tables (`rxnorm_brand_ingredient.tsv` + alias + disease overrides).

## 8. Limitations & follow-ups

- **Disease axis is weaker** (55% exact). Disease normalization needs ICD-10TM /
  disease-ontology mapping, not just brand→generic. Separate workstream.
- **Granularity ≠ resolution.** `kidney failure` resolves fine to a node, but the
  metformin contraindication edge is on `chronic kidney disease` / `end stage
  renal failure`. Mapping a term to the node that *carries the edge* is an
  **ontology-expansion** layer, distinct from name normalization. Don't conflate.
- **Combination brands** (percocet = oxycodone + acetaminophen) yield multiple
  ingredients — decide whether to check each component (recommended for safety).
- **Thai names** (TMT, `TMTRF20260518_FULL.xls` on the data drive) map Thai drug
  names → generic; slot in as another layer-1 source for Thai-language input.

## 9. Runbook

- Rebuild table: `python3 build_rxnorm_table.py`
- Run the gate: `NEO4J_PASSWORD=… cargo run --bin resolution_bench` (needs a
  port-forwarded Neo4j)
- Add a mapping: append to the relevant static table, re-run the bench, confirm the
  exact-rate did not regress, commit.
