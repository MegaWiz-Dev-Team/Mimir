# Terminology Mapping — Reference for Iris

> Reference doc for Iris (and any downstream consumer) that needs to translate
> between clinical terminologies — ICD-10-TM, SNOMED CT, TMT, LOINC, MONDO,
> HPO, DrugBank — and choose the right Mimir endpoint for each lookup.
>
> Every claim below is verified against live data on `mimir-api:v2.3.50`. Run
> the commands at the bottom of each section to confirm in your own env.

## 1. Vocabularies in use

| Name | Where | Size (live count) | Identifier | What it represents | Tenant |
|---|---|---|---|---|---|
| **ICD-10-TM** (Thai-localized ICD-10) | MariaDB `icd10_codes` `source_version='anamai-moph-2010'` | 15,376 codes · 15,002 have Thai labels (97.6%) | `code` (e.g. `E11`, `J18.9`) | Disease/diagnosis classification | NULL (master) |
| **SNOMED CT** | MariaDB `snomed_descriptions` (FULLTEXT on `term`) | 1,397,752 description rows | `concept_id` (e.g. `360455002`) | Granular clinical concepts: diseases, findings, procedures, body sites | NULL (master) |
| **MONDO** | Neo4j PrimeKG `n.source='MONDO'` or `'MONDO_grouped'` | (graph node) | `entity_id` (numeric, e.g. `10269`) | Unified disease ontology — cross-reference layer to PrimeKG | NULL (master) |
| **TMT** (Thai Medicines Terminology) | MariaDB `tmt_codes` | 111,255 codes + `tmt_relationships` hierarchy | `tmt_id` (e.g. `100000001`) | Thai dm+d-style 8-layer drug ontology (TP, TPU, GP, VTM, …) — brand→generic | NULL (master) |
| **TMLT** (Thai Medical Lab Terminology) | MariaDB `tmlt_*` | Separate ingest | `tmlt_id` | Lab observation codes (Thai) | NULL (master) |
| **LOINC** | MariaDB `loinc_codes` | (FULLTEXT on `long_common_name`, `short_name`) | `loinc_code` | Lab observations, vital signs, panels | NULL (master) |
| **HPO** | Neo4j PrimeKG `n.type='phenotype'` (or `effect/phenotype`) | (graph node) | `entity_id` + `name` (PascalCase, e.g. `Fever`) | Human Phenotype Ontology — symptoms/clinical findings | NULL (master) |
| **DrugBank** | Neo4j PrimeKG `n.type='drug'` | (graph node) | `entity_id` starts with `DB` (e.g. `DB00564`) | Drug knowledgebase — used by PrimeKG drug nodes | NULL (master) |
| **PrimeKG** (Harvard) | Neo4j graph (umbrella) | (graph node + edge) | `entity_index` (i64) | The graph itself — nodes are MONDO/HPO/DrugBank/gene/pathway/etc., edges are clinical relations | NULL (master) |

All vocabularies are **shared master data** (`tenant_id IS NULL`) — every tenant sees the same content.

## 2. Crosswalks (the actual mappings)

These are the **bridge tables / edges** between vocabularies. Iris uses these,
directly or transitively, every time it asks "given X in vocab A, what is the
equivalent in vocab B?".

| From → To | Where | Rows | Notes |
|---|---|---|---|
| **SNOMED → ICD-10** | MariaDB `snomed_icd10_map` (`snomed_id`, `icd10_code`, `predicate`) | 154,805 | `predicate='skos:exactMatch'` is the strongest tier; others are `closeMatch`/`broadMatch`. |
| **SNOMED → MONDO** | MariaDB `snomed_mondo_map` (`snomed_id`, `mondo_id`, `predicate`) | 9,120 | Bridge to PrimeKG. Sparse — many SNOMED concepts have no MONDO equivalent. |
| **MONDO → PrimeKG entity** | Neo4j `MATCH (n:PrimeKG) WHERE n.entity_id = $mondo_id AND n.source IN ['MONDO','MONDO_grouped']` | (graph) | `MONDO_grouped` nodes carry multiple underscore-joined MONDO ids inside one cluster. |
| **TMT trade product → generic** | MariaDB `tmt_relationships` (`from_id` TP → `to_id` GP/VTM, `rel_type`) | (varies per ingest) | 8-layer dm+d: TP (Trade Product) → TPU → GP (Generic Product) → VTM (Virtual Therapeutic Moiety). |
| **Thai term → English** (lay) | Heimdall LLM (gemma-4-26b) via `THAI_NORMALIZER_MODEL` | n/a | NOT a static table — runtime LLM normalize step that activates only on Thai text in `/resolve`. |

### Visual: full resolver chain (the canonical path)

```
   User input (Thai OR English)
              │
              ▼
   Step 0  Heimdall LLM (gemma-4-26b)
           "Thai → canonical English" (only fires on Thai)
              │
              ▼
   Step 1  SNOMED FULLTEXT match on snomed_descriptions
           (apostrophe/hyphen-tolerant ORDER BY after v2.3.46)
              │
              ▼
   Step 2  snomed_mondo_map crosswalk (predicate-aware FIELD() rank)
              │
              ▼
   Step 3  Neo4j primekg_lookup_by_mondo
           — direct MONDO node OR MONDO_grouped cluster
              │
              ▼ (Step 3b name-fallback if Step 1/2 miss recall:
              │   apostrophe-strip → 's-strip → head-word → -s-strip)
              ▼
   PrimeKG entity (entity_index, entity_id, name)
              │
              ▼
   /neighbors, /drug_interactions, /disease_drugs, /path, …
```

Bug-class fixes shipped v2.3.46–50 made this chain robust against punctuation
variance ("Coats'" vs "Coats", "Lesch-Nyhan" vs "Lesch Nyhan", "Alzheimer's"
vs "Alzheimer" vs "Alzheimers"). See `scripts/resolver_*_test.py` for the
65-case regression suite (also persisted in Mimir eval under tenant
`asgard_platform`, agent `primekg-*`).

## 3. API surface for Iris

All routes are nested under `/api/v1/knowledge/*`. Tenant context goes in via
`X-Tenant-Id` header (master KB data is tenant-NULL — passing any valid tenant
works for read-only KB lookups).

### 3.1 Unified cross-KB search (recommended starting point)

| Route | Body / Query | What it does |
|---|---|---|
| `GET /api/v1/knowledge/search?q=<term>&k=<int>` | — | Fans `q` out across **all** shared KBs (icd10-tm, tpc, tmt, tmlt, loinc, primekg) in parallel, returns grouped results `{q, k, results: [{kb_id, items, count, latency_ms}], total_ms}`. **Use this when Iris doesn't know which vocab the term belongs to.** |
| `GET /api/v1/knowledge/shared` | — | Catalog of available shared KBs (descriptions, doc links). |

### 3.2 PrimeKG (graph queries — disease, drug, phenotype, gene)

| Route | Body | Use case |
|---|---|---|
| `POST /knowledge/primekg/entity` | `{name, type?, limit?}` | Direct PrimeKG name lookup. `type` filter: `disease` / `drug` / `gene/protein` / `phenotype` / etc. |
| `POST /knowledge/primekg/resolve` | `{text}` | Text → PrimeKG entity via the **Thai-aware resolver chain** (SNOMED→MONDO→PrimeKG + LLM normalize). Iris's main entry point for free-text disease names in any language. |
| `POST /knowledge/primekg/neighbors` | `{entity_index, relation_types?, hops?, limit?}` | 1–3 hop neighbors with optional relation-type filter. ⚠️ Relation types are stored UPPERCASE in PrimeKG: `INDICATION`, `CONTRAINDICATION`, `OFF_LABEL_USE`, `PHENOTYPE_POSITIVE`, etc. |
| `POST /knowledge/primekg/disease_relations` | `{query}` | One-shot disease → balanced edges (all non-contraindication kept + contraindication capped at `limit`). **This is the path the Medical Knowledge Assistant chat uses.** |
| `POST /knowledge/primekg/drug_interactions` | `{drug_index, limit?}` | Drug-drug edges. Counterparts identified by DrugBank id (`DB...`). `severity_filter_supported: false` — PrimeKG doesn't track severity natively; post-filter on `display_relation`. |
| `POST /knowledge/primekg/disease_drugs` | `{disease_index, limit_per_relation?}` | Returns groups `{indication: [...], contraindication: [...], off_label_use: [...]}`. Each item is a drug node (DrugBank id). |
| `POST /knowledge/primekg/symptom_to_disease` | `{phenotype_names: [...], min_match?, limit?}` | HPO phenotype names → matching diseases. Phenotype names use HPO PascalCase: `Fever`, `Headache`, `Cough`. |
| `POST /knowledge/primekg/path` | `{from_index, to_index, max_hops?, limit_paths?}` | Multi-hop path between entities. `max_hops` default 4, `limit_paths` default 3 (clamped 1–10). |
| `POST /knowledge/primekg/assistant` | `{query, session_id?}` | Tenant-pinned (server-side) shared assistant (agent id from `PRIMEKG_ASSISTANT_AGENT_ID`, default 9). Returns `{answer, reasoning, steps}`. |
| `POST /knowledge/primekg/assistant/stream` | `{query, session_id?}` | SSE-streamed version of the above. |

### 3.3 SNOMED CT direct

| Route | Body | Use case |
|---|---|---|
| `POST /knowledge/snomed/search` | `{text, limit?}` | FULLTEXT search on SNOMED descriptions. Returns ranked `{concept_id, term, term_type, semantic_tag}`. |
| `POST /knowledge/snomed/resolve-icd10` | `{text}` | SNOMED match → ICD-10 crosswalk in one call. |

### 3.4 ICD-10 / ICD-10-TM

| Route | Use case |
|---|---|
| `GET /api/v1/icd10/lookup?q=<term>` | Term → ICD-10 cascade (exact code-shape → prefix → semantic via Heimdall BGE-M3). Returns codes with `en_label` + `th_label`. |
| `GET /api/v1/icd10/code/{code}` | Direct code lookup. Returns `{code, en_label, th_label, chapter, block, billable_flag, drg_id, locale_metadata}`. |
| `GET /api/v1/icd10/sources` | List installed source versions (currently `anamai-moph-2010` = Thai MoPH 2010 = effectively ICD-10-TM). |

> **ICD-10-TM is data-driven via `source_version`**, not a separate table. Today the
> only loaded source is Thai MoPH 2010 (Anamai), which covers 15,376 codes with
> Thai labels on 97.6%. To support a second source (e.g. WHO ICD-10), just ingest
> with a different `source_version` — endpoints already join on it.

### 3.5 TMT / TMLT (Thai drugs / lab terms)

| Route | Use case |
|---|---|
| `POST /knowledge/tmt/resolve` | Resolve a Thai/English drug name (incl. brand) to TMT concept(s). Useful to expand brand → generic (e.g. `Glucophage` → `metformin`). |
| `POST /knowledge/tmlt/expand` | Expand a TMLT (lab) code to its hierarchy. |

## 4. Iris query patterns (copy-paste)

All examples assume the in-cluster service URL `http://mimir-api.asgard.svc:8080`.
Replace with `http://localhost:30000` for NodePort/local access.

### 4.1 "User typed 'ไข้เลือดออก' — what is this disease and what's connected to it?"
```bash
curl -s -X POST $URL/api/v1/knowledge/primekg/disease_relations \
  -H 'Content-Type: application/json' -H 'X-Tenant-Id: asgard_medical' \
  -d '{"query":"ไข้เลือดออก","limit":15}'
# → {"resolved_disease":"dengue disease", "seed":{...}, "relations":[...]}
```

### 4.2 "What's the ICD-10 code for type-2 diabetes?"
```bash
curl -s "$URL/api/v1/icd10/lookup?q=type+2+diabetes"
# → {"items":[{"code":"E11","en_label":"Type 2 diabetes mellitus",
#              "th_label":"เบาหวานชนิดที่ 2", ...}], ...}
```

### 4.3 "Cross-vocabulary lookup — show this term in every KB at once"
```bash
curl -s "$URL/api/v1/knowledge/search?q=metformin&k=5"
# → {"results":[
#       {"kb_id":"primekg","items":[{name:"metformin", entity_id:"DB00331", ...}]},
#       {"kb_id":"tmt","items":[{tmt_id:..., fsn:"metformin hydrochloride 500 mg ..."}, ...]},
#       {"kb_id":"icd10-tm","items":[...]}, ...
#    ], "total_ms": ...}
```

### 4.4 "Brand-name drug — resolve to generic"
```bash
curl -s -X POST $URL/api/v1/knowledge/tmt/resolve \
  -H 'Content-Type: application/json' \
  -d '{"name":"Glucophage"}'
# → resolves through TP → GP/VTM to surface "metformin"
```

### 4.5 "Disease → indicated drugs" (chained: lookup → disease_drugs)
```bash
# Step 1: get the disease's entity_index
DM_IDX=$(curl -s -X POST $URL/api/v1/knowledge/primekg/entity \
  -H 'Content-Type: application/json' \
  -d '{"name":"diabetes mellitus","type":"disease","limit":1}' \
  | jq -r '.items[0].entity_index')

# Step 2: fetch grouped drug edges
curl -s -X POST $URL/api/v1/knowledge/primekg/disease_drugs \
  -H 'Content-Type: application/json' \
  -d "{\"disease_index\":$DM_IDX,\"limit_per_relation\":10}"
# → {"indication":[...], "contraindication":[...], "off_label_use":[...]}
```

### 4.6 "Symptoms → candidate diseases" (HPO)
```bash
curl -s -X POST $URL/api/v1/knowledge/primekg/symptom_to_disease \
  -H 'Content-Type: application/json' \
  -d '{"phenotype_names":["Fever","Cough"],"min_match":2,"limit":10}'
# Note PascalCase — "Fever" not "fever".
```

## 5. Gotchas / conventions Iris MUST know

1. **Cypher relation types are UPPERCASE.** When calling `/neighbors` with
   `relation_types`, use `["INDICATION"]` not `["indication"]` — lowercase
   returns 0 items. (Confirmed: regression in `resolver_full_coverage_test.py`.)
2. **DrugBank ids identify drugs more reliably than `type`.** PrimeKG nodes
   sometimes lack the `type` field; check `entity_id.startswith("DB")` if
   you must determine drug-ness from a raw row.
3. **HPO phenotype names are PascalCase** in PrimeKG: `Fever`, `Headache`,
   `Cough`. Lowercase variants generally don't match.
4. **`MONDO_grouped` nodes carry multiple MONDO ids** in `entity_id`,
   underscore-joined. `grouped_name` is the canonical English name of the
   cluster (e.g. "Parkinson disease"); `name` may carry an FSN relabeled
   from the matched SNOMED concept. **Prefer `grouped_name` for display
   when both fields are present.**
5. **Apostrophe/hyphen variance is tolerated** in `/resolve` and
   `/disease_relations` since v2.3.46–50. "Coats disease", "Coats' disease",
   "Lesch-Nyhan syndrome", and "Lesch Nyhan syndrome" all reach the same
   concept. Iris should NOT pre-normalize input — the resolver does it better.
6. **`/disease_relations` is the assistant's code path**, NOT `/resolve`.
   It runs `llm_extract_disease` (LLM) → `primekg_lookup_entity` (NOT the
   SNOMED→MONDO chain). The two have **different fallback chains** and
   their fixes ship independently — when something is wrong in the chat
   experience, suspect `/disease_relations`, not `/resolve`.
7. **Master-data tenancy.** All KB tables use `tenant_id IS NULL`. Endpoint
   queries also pin `WHERE tenant_id IS NULL`. Passing any tenant value in
   the header is fine for reads, but writes/ingest are routed by tenant
   strictly.
8. **ICD-10 codes have granular variants stored at the base.** E.g.
   `E11.9` is stored as `E11`; the cascade in `/icd10/lookup` strips the
   decimal for retrieval but the matcher accepts either input.

## 6. Where the code lives

- Endpoint handlers: `ro-ai-bridge/src/routes/knowledge_*.rs`
  (`knowledge_primekg.rs`, `knowledge_search.rs`, `knowledge_snomed.rs`,
  `knowledge_tmt.rs`, `icd10.rs`)
- Neo4j layer: `ro-ai-bridge/mimir-core-ai/src/services/neo4j.rs`
  (`primekg_lookup_entity`, `primekg_lookup_by_mondo`,
  `primekg_neighbors_filtered`, `primekg_drug_interactions`,
  `primekg_disease_drugs`, `primekg_symptom_to_disease`, `primekg_path`)
- Regression suites: `scripts/resolver_bug_class_test.py`,
  `scripts/resolver_disease_relations_test.py`,
  `scripts/resolver_full_coverage_test.py`
- Mimir-eval persistence: `scripts/persist_primekg_resolver_eval.py`
- Forseti CI gate: `Forseti/examples/test_scripts/mimir_primekg_resolver_e2e.yaml`

## 7. Versioning / changelog markers

Iris pins the API contract by mimir-api **image tag** (`asgard-mimir-api:vX.Y.Z`).
Material changes to terminology resolution since v2.3.45:

| ver | what changed |
|---|---|
| v2.3.41 | Resolver Step 2 FIELD() rank preservation; MONDO_grouped relabel; head-word recall fallback. Fixed dengue→Q-fever, depression→anxiety, missing-crosswalk acne. |
| v2.3.43 | SQL injection fix in resolve() — fully parameterized .bind(); escape_like for LIKE patterns with ESCAPE '|'. |
| v2.3.45 | `/disease_relations` balanced edges (keep all non-contra, cap contra at limit). |
| **v2.3.46** | `MATCH AGAINST DESC` added to Step 1 ORDER BY — fixes apostrophe-stripped / hyphen-as-space inputs ("Coats", "Lesch Nyhan"). |
| **v2.3.47** | `snomed_fsn` picks FSN of TOP concept; `primekg_lookup_by_mondo` sorts position-first (over standalone-vs-grouped). Fixes Parkinson display, Coffin-Siris vs Lowry. |
| **v2.3.48** | Step 3b head-word from `effective` (user input) + trailing-`s` strip. Fixes "Alzheimers disease" typo. |
| **v2.3.49** | `/disease_relations` apostrophe + head-word fallback chain. Fixes "Coats' disease" via assistant. |
| **v2.3.50** | `/disease_relations` adds `'s` → "" variant before `'` → "". Fixes "Parkinson's" / "Huntington's" via assistant. |
