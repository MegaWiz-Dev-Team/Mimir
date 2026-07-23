# 03_17 — RxNorm + TMT Crosswalk Resolver (v1)

**Status:** design → implementation (branch `feat/rxnorm-normalizer`)
**Supersedes the mechanism in:** `ro-ai-domain-medical/docs/NORMALIZER.md` (v1 static
seed-map). This closes its named follow-ups: §4 (RxNorm→DrugBank crosswalk), §5 (full
RRF table), §8 (Thai/TMT lane, combos).
**Owner:** medical / clinical-safety-pruner.

## 1. Why v2

The v1 normalizer proved the mechanism — a 57-brand static TSV took drug resolution
`73.3% → 100%` on the `drug-disease-resolution-v1` probe. But it does not generalize
and does not ship well as-is:

| v1 limitation | consequence |
|---|---|
| 57 hand-built brands (`build_rxnorm_table.py` over a hardcoded list) | any brand outside the list → miss; "100%" is partly by construction (§6 non-overfit caveat) |
| hand-curated US↔INN alias table (§4) | every divergent name (albuterol↔salbutamol) is manual; unbounded tail |
| resolves against PrimeKG names directly | identity = DrugBank names → **DrugBank-license-tainted** for commercial ship |
| Thai input unhandled (§8) | Thai trade names (ซารา, ทัยลินอล) all miss; probe is 65× **en-only** |
| disease at 55% | brand→generic can't fix disease; needs its own ontology path |

**Design principle (unchanged):** *resolution — not the graph query — is the dominant
failure mode.* v2 gives "drug" the same normalization depth "disease" already has
(`SNOMED → MONDO → PrimeKG`), built from **public-domain data** so it ships.

## 2. Architecture — two lanes into one ingredient identity

```
   input: brand / generic / Thai / misspelled
        │  lower + trim
   ┌────┴──────────────────────────────┐
   ▼ Lane A  (EN / international)        ▼ Lane B  (Thai)
  RxNorm atoms (full RRF, MariaDB)      TMT dm+d ladder (already loaded)
  BN/SBD/SCD ──rxnorm_rel──▶ IN         TP → TPU → GPU → GP → VTM → SUBS
  SY atoms carry INN (salbutamol)       (recursive over tmt_relationships)
   │  → RxNorm IN (RXCUI)                │  → VTM/SUBS FSN (generic, EN+TH)
   └───────────────────┬─────────────────┘
                       ▼
          ★ CANONICAL HUB = active-ingredient identity
            RXCUI(IN)  ⇄  UNII (FDA, public-domain, language-neutral)
                       │  rxnorm_primekg_map (name | inn_syn | unii)
                       ▼
             PrimeKG DrugBank node (entity_index)
                       │  ← now ONLY the relationship layer
                       ▼
        /drug_interactions · /disease_drugs · /neighbors
```

The identity of a drug moves from *DrugBank-name-match* to *RXCUI/UNII*. PrimeKG becomes
purely the **edge** layer (interaction/indication); resolution no longer depends on
DrugBank → the commercial-license blocker is removed.

## 3. Options compared (against the measured failure modes)

| dimension (probe failure) | naive PrimeKG | v1 seed-map | RxNorm-only (full) | TMT-only | **RxNorm+TMT (v2)** |
|---|:--:|:--:|:--:|:--:|:--:|
| Brand EN (coumadin→warfarin) | ✗ 0% | ~ (57 only) | ✓✓ | ~ (TH-registered) | ✓✓ |
| Cross-region generic (paracetamol, salbutamol) | ✗ | ~ (alias table) | ✓ (SY/INN atoms) | ✓ (SUBS↔INN) | ✓✓ |
| Thai trade names | ✗ | ✗ | ✗ | ✓✓ | ✓✓ |
| Misspelling / lay | ✗ | ✗ | ✓ (fulltext) | ~ | ✓ |
| Dose/form (SCD/SBD) | ✗ | ✗ | ✓✓ | ✓ (GPU/TPU) | ✓✓ |
| **Ships commercial** | ✗ DrugBank | ✓ | ✓ (SAB=RXNORM) | ✓ (MOPH) | ✓✓ |
| Offline / air-gapped | ✓ | ✓ | ✓ (RRF→MariaDB) | ✓ | ✓ |

RxNorm-only loses Thai trade names; TMT-only lacks EN generic breadth + fuzzy match.
Hybrid closes both gaps and both halves are public-domain / MOPH — no DrugBank.

## 4. Data model (migration `sprint55_rxnorm_crosswalk.sql`)

| table | from | role |
|---|---|---|
| `rxnorm_atoms` | RXNCONSO.RRF (ENG, kept TTY, SAB=RXNORM core) | every name string → `str_norm` index for exact/prefix/fulltext; TTY IN/PIN/BN/SBD/SCD/SY |
| `rxnorm_rel` | RXNREL.RRF (ingredient-bearing RELA) | brand/drug → IN closure (`has_ingredient`, `tradename_of`, `consists_of`, …) |
| `rxnorm_unii` | RXNSAT.RRF (ATN='UNII') | RXCUI(IN) → UNII — the chemical-identity join key |
| `rxnorm_primekg_map` | built by `rxnorm_primekg_bridge.py` | ingredient → PrimeKG node, with `match_method` provenance |
| `rxnorm_ingest_runs` | ingest | provenance + `bridge_coverage` |

All `tenant_id = NULL` (shared master, mirrors `tmt_codes`/`icd10_codes`). **Ships WITH a
`GET /api/v1/knowledge/shared` catalog row for `rxnorm` in the same PR** (shared-knowledge
rule — never a silently-invisible master table). TMT tables already exist — Lane B needs
no new schema, just a recursive `tmt_relationships` traversal view.

## 5. Resolution algorithm

```
resolve_drug(term, locale) -> {primekg_index, ingredient, rxcui|tmt_id, unii, tier, lane}:
  # Lane A — RxNorm (fires whenever term has ascii)
  rxcui = rxnorm_atoms.str_norm == lower(term)             # tier0 exact (any TTY)
       or rxnorm_atoms FULLTEXT(term) top-1                # tier2 fuzzy
  if rxcui and tty != IN: rxcui = climb rxnorm_rel → IN    # brand/SCD → ingredient
  hit = rxnorm_primekg_map[rxcui]                          # → PrimeKG node
  # Lane B — TMT (fires on Thai text / locale=th / Lane A miss)
  if thai(term) or locale=='th' or not hit:
     subs = tmt: match FSN(TP/TPU/GP) then climb → VTM/SUBS
     rxcui = rxnorm_atoms.str_norm == subs.english_name    # converge into Lane A hub
          or subs.unii → rxnorm_unii → rxcui
     hit = rxnorm_primekg_map[rxcui]
  return rank_by_tier(hit)   # tier0 ingredient-exact > tier1 fuzzy/inn > tier2 substring
```

Every result carries provenance (`match_method`, `lane`, `tier`) — a clinical guardrail
must be auditable, and a low tier is surfaced (`SUSPECT`), never silently trusted. The
naive `primekg_lookup_entity` stays as the tier-2 fallback only.

## 6. The UNII bridge (the risk) — `rxnorm_primekg_bridge.py`

PrimeKG drug nodes carry only a DrugBank id (`DB00682`) + a name. Mapping RxNorm IN →
that node is the highest-risk step (§4 was left open here). The bridge is **tiered,
most-license-clean first**, and **measures coverage** so the dependency decision is
data-driven, not assumed:

1. **`name`** — RxNorm IN/PIN `str_norm` == PrimeKG node name. No external data. Expected
   to cover the majority (both standardized generic names).
2. **`inn_syn`** — a RxNorm `SY` atom of that ingredient == PrimeKG name. Catches US↔INN
   (albuterol→salbutamol) using **RxNorm's own synonymy** — still zero DrugBank, and it
   *retires the hand-curated alias table*.
3. **`unii`** — RxNorm UNII == DrugBank UNII. Runs **only if** `--drugbank-vocab` (DrugBank
   open `vocabulary.csv`, cols `DrugBank ID`,`UNII`) is supplied. **License must be verified
   before shipping that file**; without it the tier is skipped and the residual it *would*
   rescue is reported. UNII itself (FDA) is public-domain.

The script prints `bridged / residual` with a per-tier breakdown and a residual sample.
**Open question the coverage run answers:** how many of PrimeKG's drug nodes need tier 3
at all? If `name`+`inn_syn` reach ≥ ~95%, the DrugBank-vocab dependency can be dropped
entirely and the resolver is 100% public-domain. Record `bridge_coverage` in
`rxnorm_ingest_runs`.

## 7. License lanes (the commercial unblock)

| source | gives | license | ships? |
|---|---|---|---|
| RxNorm (RRF, SAB='RXNORM' atoms) | brand/generic/synonym/INN, RXCUI | public domain | ✅ primary |
| UNII (FDA / via RXNSAT) | chemical identity | public domain | ✅ bridge key |
| TMT | Thai brand→generic | MOPH/THIS-Center, free in TH | ✅ Thai lane |
| DrugBank `vocabulary.csv` | DB id→UNII | **verify** (tightened terms) | ⚠️ tier-3 only, optional |
| DrugBank full / DDInter | edges, severity | commercial / CC-BY-NC-SA | ❌ research/benchmark tenant only |

Keep the ingest to `SAB='RXNORM'` atoms for the clean core (other source atoms carry
per-source flags). PrimeKG/DrugBank edges stay behind the research/benchmark firewall.

## 8. Rollout + eval

1. Apply `sprint55_rxnorm_crosswalk.sql`.
2. `rxnorm_ingest.py --rrf-dir … --source-version rxnorm-YYYYMMDD` (full atoms/rel/unii).
3. `rxnorm_primekg_bridge.py --source-version …` → build map + **read the coverage report**;
   decide on the DrugBank-vocab dependency from the residual.
4. **Lane B — DONE.** `build_tmt_table.py` climbs the TMT dm+d ladder (TP←GP←VTM; the
   general form is the recursive `tmt_relationships` CTE, in the script header) and dumps a
   static `data/tmt_thai_generic.tsv` — **21,100 Thai-brand→generic** pairs (325 ambiguous
   brands dropped, never guessed; single-ingredient salt/hydrate stripped to the moiety so it
   matches PrimeKG; generic-name-as-brand keys skipped). `DrugDiseaseNormalizer` loads it as
   layer 0 (`include_str!`, offline) ahead of the RxNorm/alias layers. Verified in-crate:
   sara→paracetamol, brufen→ibuprofen, ponstan→mefenamic acid (5/5 tests pass).
5. Point the rest of `DrugDiseaseNormalizer` at the full RxNorm tables (replace the 57-brand
   TSV loader, §7 of NORMALIZER.md); keep the naive PrimeKG match as tier-2 fallback.
6. Add a `/api/v1/knowledge/shared` catalog row for `rxnorm`.
7. **Extend the probe**: `drug-disease-resolution-v1` is 65× en-only → add Thai-locale drug
   probes so Lane B (TMT) is actually exercised; keep RAW-vs-NORMALIZED bucket deltas as the
   regression gate.

**Targets:** brand EN exact `0%→~95%`, drug exact `73%→90%+`, Thai trade names resolved,
paracetamol/salbutamol via SY/INN (no alias table), `bridge_coverage` reported. Disease is
**out of scope** here — its 45% is an eval artifact (naive `entity` vs full `resolve`);
switch that probe to `POST /knowledge/primekg/resolve` separately.

## 9. Files

- `ro-ai-bridge/migrations/sprint55_rxnorm_crosswalk.sql` — schema
- `scripts/rxnorm_ingest.py` — full RRF ingest (replaces `build_rxnorm_table.py`)
- `scripts/rxnorm_primekg_bridge.py` — UNII bridge + coverage probe
- `ro-ai-domain-medical/scripts/build_tmt_table.py` — Lane B TMT climb → `tmt_thai_generic.tsv`
- `ro-ai-domain-medical/data/tmt_thai_generic.tsv` — 21,100 Thai-brand→generic (compiled in)
- `ro-ai-domain-medical/src/normalizer.rs` — layer-0 Thai lane wired + tests
- `ro-ai-domain-medical/docs/NORMALIZER.md` — v1 mechanism + measured baseline (context)
