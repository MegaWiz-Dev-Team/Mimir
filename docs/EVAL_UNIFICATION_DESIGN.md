# Unified Evaluation Storage & UI — Design

**Date:** 2026-06-04 · **Branch:** `feat/eval-unify` · **Status:** proposed

## Problem

Eval storage grew one-table-per-type. Today there are **4 separate `*_runs`
tables**, **4 `*_datasets` tables**, and metrics live in three incompatible
shapes:

| family | runs table | metrics stored as | dataset table |
|---|---|---|---|
| agent / QA / HealthBench | `eval_runs` + `eval_scores` + `eval_summary` | `eval_summary.avg_accuracy/...` columns | `eval_datasets`, `eval_benchmark_datasets` |
| RAG | `rag_eval_runs` | 40+ inline columns (`hit_rate`, `mrr`, `ndcg`...) | `rag_eval_datasets` |
| OCR text | `ocr_eval_runs` + `ocr_eval_results` | `ocr_eval_results.cer/wer` | `ocr_eval_datasets` |
| OCR layout | `ocr_layout_eval_runs` + `ocr_layout_region_match` | bespoke | — |

`eval_scores` is becoming a god-table (QA + safety + rubric + retrieval_trace +
tool_calls nullable columns). The UI mirrors it: `/evaluations` has hardcoded
tabs bound to the QA "agent × model matrix" shape, while OCR lives on a
**separate** `/syn-ocr/eval` page and RAG has its own dashboard. Adding NER or
coding means a 5th table + a 5th UI surface. **There is no single scoreboard.**

## Principle

> Separate three concerns: **what was measured** (target × dataset) → **the
> score** (one normalized metric row) → **the evidence** (item / artifact).

The score layer is the unifier: *every* number — accuracy, CER, recall,
hit_rate, leak_rate, p95 latency — becomes one row in `evx_metric`. The
scoreboard is then a single query regardless of family.

## Target schema (`evx_*`, additive — legacy untouched)

```
evx_experiment ──< evx_run >── evx_target          (run = ONE target on ONE dataset)
                       │   └── evx_dataset
                       ├──< evx_metric    normalized score rows  ← the scoreboard
                       ├──< evx_item      per-item evidence (JSON payload)
                       ├──< evx_artifact  heavy diagnostics by URI
                       └──< evx_span      satellite: NER / extraction / layout
```

Key design points:
- **`evx_target`** replaces the overloaded `agent_name`/`model_id` pair. A
  target is *anything under test* — an agent, a model, a RAG pipeline config, or
  a runtime variant (PyThaiNLP vs ONNX vs CoreML-ANE). Deterministic
  `id = SHA2(natural key)` so live writers and backfill converge without a
  lookup.
- **`evx_metric`** carries `name`, `slice_key` (`entity_type=person`,
  `channel=vector`, `doc_type=lab`), `is_primary` (the gate metric),
  `higher_is_better`, `ci_low/ci_high`, and `n`. The `n` + CI columns exist
  specifically to stop the n=20 seed-variance trap and the dot-stripping class
  of measurement bug from hiding behind a single number.
- **`evx_run` = one (target, dataset) execution.** Legacy batch tables that
  tested N targets at once (eval_runs × agent × model; ocr_run × engine) are
  *exploded* into N runs grouped under one `evx_experiment`. This is what makes
  ranking and A/B diff clean.
- **`evx_span`** stores `text_hash`, **never raw PII** — it exists for the
  span-level self-join that powers NER's per-entity regression diff.

Two satellites are deferred until their family lands natively (same pattern, no
core change): `evx_rubric_dimension` (generative/LLM-judge) and `evx_trace_step`
(tool-calling / agentic trajectory).

## How each family maps (proof it generalizes)

| family | target.kind | primary metric | slices | satellite |
|---|---|---|---|---|
| QA/MCQ/HealthBench | agent | overall_score | specialty, difficulty | — |
| RAG | pipeline | hit_rate | channel=vector/tree/graph | — |
| OCR text | model | cer (↓) | doc_type | — |
| OCR layout | model | iou (↓err) | region_type | evx_span (bbox) |
| **NER (Skuggi)** | runtime_variant | recall / leak_rate | entity_type, ocr_noise | evx_span |
| **Coding (SNOMED/ICD)** | pipeline | code_acc | code_system | — (hierarchy scorer) |

All six share the same 7 core tables. PyThaiNLP/ONNX/ANE compare via three
`evx_target` rows under one experiment — same mechanism as gemini-vs-gemma.

## UX / UI

Replace the hardcoded tabs + scattered pages with a **registry-driven** shell so
a new family is config, not a new page (delivering what `eval-tab-registry.tsx`
was supposed to).

```
/evaluations
  ├── Scoreboard   (default) — one table, all families
  │     row = target · primary metric · n · CI · Δ vs champion
  │     filters: family, dataset, target.kind, tenant
  │     powered by the evx_scoreboard view — zero per-family code
  ├── Run detail   — metric cards (primary big, slices as a bar list by slice_key)
  │                  + item table (virtualized) with per-family payload renderer
  ├── Compare A/B  — pick 2 runs → metric delta + item-level diff
  │                  · NER regression diff = this view with a span renderer
  │                  · generalizes the existing evaluations "matrix" + GitCompare
  └── Datasets     — unified browser over evx_dataset (family-filtered)
```

Family-specific code shrinks to **one registry entry**:

```ts
// eval-family-registry.ts
registerEvalFamily('ner', {
  label: 'NER / PII',
  primaryMetric: 'recall',
  sliceDimensions: ['entity_type', 'ocr_noise'],
  ItemRenderer: NerSpanDiff,     // renders evx_span gold-vs-pred overlay
  scoreColor: v => v >= 0.98 ? 'green' : 'red',   // the gate
})
```

The Scoreboard, Run-detail metric cards, Compare delta, and Datasets browser are
all **family-agnostic** — they read `evx_metric.name/slice_key/is_primary` and
the registry's `primaryMetric`. Only `ItemRenderer` is per-family (a span
overlay for NER, a text-diff for OCR, a Q/A card for QA). OCR's `/syn-ocr/eval`
pages collapse into the same shell.

## Migration & cutover (non-destructive, reversible)

0. **BACKUP FIRST — mandatory gate.** Run `scripts/eval_unify_backup.sh`: dumps
   every legacy eval table (+ evx_* if present) to T7, gzips, `gzip -t` verifies,
   writes a MANIFEST with per-table row counts + sha256. Do NOT run step 1/2
   until this exits `[✓]`. (feedback_backup_before_changes — no implicit backups.)
1. **Apply core** (sqlx migration `20260604120000`) — creates `evx_*`, drops
   nothing. Auto-applied on deploy; pure DDL, fast.
2. **Backfill** — `scripts/eval_unify_backfill.sql`, run **manually** (kept OUT
   of the sqlx auto-chain so its large `INSERT...SELECT` over eval_scores /
   rag_eval_queries never locks prod mid-deploy). Idempotent, deterministic ids.
   Covers QA/HealthBench, RAG, OCR-text. (OCR-layout spans + NER born-native.)
3. **Verify in staging** — run the count assertions in the backfill footer
   (incl. the one-primary invariant query); eyeball `evx_scoreboard`.
4. **Dual-write** — point new eval writers at `evx_*`; keep legacy writers on
   for one release so nothing regresses (legacy tables stay live).
5. **Cut the UI over** to the registry shell reading `evx_*`.
6. **Freeze legacy writers**, leave tables read-only as an archive. Drop only
   after a deprecation window (separate migration, deliberate).

Rollback: core has a `down/` migration; the backfill is reversed by deleting
rows with `evx_experiment.legacy_source IS NOT NULL` (preserving native data) —
and the T7 dump from step 0 is the hard floor.

## Review fixes folded in (2026-06-05)

- **Data-loss bugs fixed in backfill:** replicate_index now part of QA `item_id`
  (was collapsing all replicates → 1 row); HealthBench `rubric_score`/`safety`/
  `rubric_items` carried to items + an aggregate `rubric_score` metric; RAG
  per-query items backfilled from `rag_eval_queries`.
- **Feature preserved:** human-override / HITL review (`eval-score-override`)
  → new `evx_item_review` satellite, backfilled from `eval_scores.human_*`.
- **Dataset linkage:** QA runs now link `evx_dataset` via
  `eval_runs.config->benchmark_dataset_id`; OCR `pii_sensitivity` derived from
  `source` (synthetic → none) instead of blanket `raw`.
- **Schema semantics:** RAG **target = (embed_model, rerank_model)** only —
  weights/top_k are tuning knobs on `evx_run.config_json`, so a weight sweep is
  a within-target comparison, not N new targets. `slice_key` split into
  `slice_dim` + `slice_val` for clean GROUP BY. **One-primary-per-run is now
  ENFORCED** by a generated `primary_one` column + unique key (not just
  convention). `evx_scoreboard` exposes `tenant_id`.

## Guardrails

- **PII:** OCR + NER gold sets carry patient names/HN. `evx_dataset` flags
  `pii_sensitivity`; `evx_span` stores `text_hash` only; raw segregated store
  referenced by `raw_store_ref`. This is eval data *for the PII guard itself* —
  treat accordingly.
- **sqlx is source of truth** — manual ops SQL must not diverge.
- **Shared checkout** — authored in worktree `feat/eval-unify`; commit with
  explicit paths only.
- **Judge drift** — `evx_run.judge_model` is mandatory for LLM-judge families;
  a changed judge is a changed measurement.
```
