# FOM Adoption Plan — Decontamination + Pairwise Judge

**Date:** 2026-05-19
**Source paper:** [arxiv 2605.16215](https://arxiv.org/abs/2605.16215) *Fully Open Meditron: An Auditable Pipeline for Clinical LLMs*
**Source repo:** https://github.com/EPFLiGHT/FullyOpenMeditron (no LICENSE — see §5)
**Scope:** Adopt two pieces of FOM into Mimir's HealthBench-Pro pipeline. Skip the heavy fine-tune.
**Owner:** TBD
**Target sprints:** 53 (Plan A) + 54 (Plan B)

---

## 1. Why

Two credibility gaps in our current Eir evaluation pipeline:

1. **No contamination check.** Locked-20 in `hb-pro-asgard-001` defends *drift* but not *memorization*. Anyone reviewing our [04_03 baseline](./04_03_HealthBench_Pro_Baseline_2026-05-04.md) can argue gemma-4-26b's 47.8% is partly memorized from pretraining — we have no data to refute.
2. **Judge artifacts.** Sprint 51c found `judge_thinking` swings scores ±11pp on the *same dataset*. That's an absolute-scoring artifact; pairwise + positional debias is the standard fix.

Plan A (decontamination) closes #1. Plan B (pairwise judge + adopt MeditronFO-Gemma-3-27B) closes #2 and gives us a free candidate champion.

We are **not** replicating the FOM training recipe — see §6.

---

## 2. Service touchpoints

| Component | Touched? | What changes |
|---|---|---|
| `mimir-api` (Rust, mimir-core-ai) | Yes | new `evaluation/decontamination.rs` module; extend `eval_benchmark_datasets.scoring_fn` to support `pairwise_moove` |
| `eval_runs` / `eval_scores` table | Yes | new columns for contamination metadata + pairwise winner |
| `scripts/run_healthbench_eval.py` | No (keep as-is) | this is the absolute-scoring runner, do not modify — baselines anchored on it |
| `scripts/run_healthbench_pairwise.py` | **New** | pairwise runner; copies structure from `run_healthbench_eval.py` |
| `scripts/run_decontamination.py` | **New** | wraps decontamination on a corpus + writes `decontamination_report.json` |
| `docs/04_evaluation_and_testing/reports/` | Yes | every future report ships with sibling `*-decontam.json` |
| Heimdall | No | judge calls already go through Heimdall gateway; no change |
| Skuggi | No | not in this scope |
| Tyr | Yes (later) | contamination violations → audit event (§3.5) |

---

## 3. Plan A — Decontamination (Sprint 53, ~1.5 days)

### 3.1 Algorithm (clean-room from paper)

The FOM script docstring credits *huggingface/cosmopedia* (Apache-2.0). We re-implement from cosmopedia + paper description, not from FOM directly (§5 license note).

Two-stage:

1. **N-gram prefilter** (cheap): tokenize eval prompts → build `Set[Tuple[token_id × N]]` with `N=13`. For each training sample, build its n-gram set; intersect. Drop samples with zero overlap (most of the corpus).
2. **Sequence verification** (expensive but rare): for each surviving (train, eval) pair, run `difflib.SequenceMatcher` and compute
   ```
   match_length = sum(block.size for block in matching_blocks if block.size >= 5)
   ratio = match_length / len(eval_tokens)
   ```
   Flag as contaminated when `ratio >= diff_threshold` (default `0.5`).

Tokenizer = the one of the model being scored (matters for non-Latin scripts, especially Thai for `typhoon-*`). Cache per-(benchmark, tokenizer, N) tuple as pickle in `~/.cache/mimir/decontam/`.

### 3.2 What to scan against

We can't scan the **full** pretraining corpus of every model (proprietary). We scan against what we *can*:

| Corpus | Why | Size | Available? |
|---|---|---|---|
| Our own `asgard_medical` ingested docs | We control them; if 20 locked items overlap with our ingest, retrieval is "cheating" | ~40k chunks | Yes (Qdrant export) |
| `EPFLiGHT/fully-open-meditron` (HF) | If we ever try MeditronFO model, we need to know if HBp items leaked into its training set | 601k examples / 150M tokens | Yes (HF) |
| MedQA / MedMCQA / PubMedQA train splits | Same items might appear in any model trained on these | ~250k items total | Yes (HF) |
| Gemma-3 pretraining corpus | Not public — best-effort proxy via Google's published list | n/a | **No — record as known limitation** |
| Typhoon Thai medical corpus | SCB-10X partially-public | ~unknown | Partial |

The contamination report should explicitly list **what was scanned** and **what was not** — transparency over completeness.

### 3.3 Files to create

```
Mimir/
  scripts/
    run_decontamination.py              # CLI wrapper
  ro-ai-bridge/mimir-core-ai/src/
    evaluation/
      decontamination.rs                # core ngram + SequenceMatcher (or call out to Python)
  ro-ai-bridge/mimir-core-ai/migrations/
    20260601000000_decontam_metadata.sql
```

### 3.4 CLI sketch

```bash
python scripts/run_decontamination.py \
    --benchmark-id hb-pro-asgard-001 \
    --corpus hf:EPFLiGHT/fully-open-meditron \
    --corpus hf:bigbio/med_qa \
    --corpus mimir-tenant:asgard_medical \
    --tokenizer google/gemma-3-27b-it \
    --ngram 13 \
    --diff-threshold 0.5 \
    --output docs/04_evaluation_and_testing/reports/decontam-hbp-asgard-001-gemma3.json
```

Output JSON shape:
```json
{
  "benchmark_id": "hb-pro-asgard-001",
  "benchmark_locked_at": "sha256:...",
  "tokenizer": "google/gemma-3-27b-it",
  "ngram": 13,
  "diff_threshold": 0.5,
  "corpora_scanned": [
    {"id": "hf:EPFLiGHT/fully-open-meditron", "n_samples": 601234, "n_contaminated": 2},
    {"id": "hf:bigbio/med_qa", "n_samples": 12723, "n_contaminated": 0}
  ],
  "corpora_unavailable": ["gemma-3 pretraining (proprietary)"],
  "contaminated_items": [
    {"benchmark_item_id": "hbp-014", "corpus": "hf:EPFLiGHT/fully-open-meditron",
     "train_sample_id": "...", "match_ratio": 0.82,
     "eval_excerpt": "...", "train_excerpt": "..."}
  ],
  "summary": {"total_items": 20, "contaminated": 2, "clean": 18}
}
```

### 3.5 Schema additions (migration `20260601000000_decontam_metadata.sql`)

```sql
ALTER TABLE eval_runs
    ADD COLUMN decontam_report_id  VARCHAR(64)  NULL
        COMMENT 'sha256 of decontamination report JSON',
    ADD COLUMN contaminated_count  INT          NULL
        COMMENT 'How many benchmark items were flagged contaminated against scanned corpora';

CREATE TABLE IF NOT EXISTS eval_decontam_reports (
    id              VARCHAR(64)   NOT NULL PRIMARY KEY COMMENT 'sha256 of report body',
    benchmark_id    VARCHAR(36)   NOT NULL,
    tokenizer_name  VARCHAR(255)  NOT NULL,
    ngram_length    INT           NOT NULL,
    diff_threshold  FLOAT         NOT NULL,
    report_json     JSON          NOT NULL,
    created_at      TIMESTAMP     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_benchmark (benchmark_id)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

Eval runners require a fresh `eval_decontam_reports` row before they will run (refuse to score against unscanned benchmark+tokenizer pair). This makes decontamination **mandatory by construction**.

### 3.6 Use cases unlocked

1. **Beryl8/Prudential pitch defensibility** ([beryl8_prudential_business_model.md](../../memory)). When asked "did the model memorize HBp?", point at `decontam-hbp-asgard-001-*.json`.
2. **Sprint 51b champion-swap blocker — second confirmation.** typhoon-si-med-4b might benefit from training-set leakage into MedQA-style items. A decontamination report against `typhoon-si-med`'s known training corpora (`scb10x/medical_data` etc.) closes the last open question on whether the 47.80% gemma vs 46.25% typhoon comparison is fair.
3. **S1 insurance underwriter ([s1_insurance_baseline](../../memory))**. RefGraph passed integration test at 100% hit rate — needs sanity check that the 10 standardized queries weren't accidentally trained into the embedding model.
4. **Tyr audit hook** (deferred to §4): contamination events emit Wazuh alert. Auditor sees: model was retrained → next eval auto-blocked until new decontam runs.

### 3.7 Tests

- Unit: `test_ngram_set()`, `test_sequence_matcher_threshold()`, `test_tokenizer_swap_changes_ngrams()`
- Integration: scan 20 locked HBp items against a synthetic corpus where one item is copy-pasted → must flag. Modify one token in the copy → boundary test on `diff_threshold=0.5`.
- E2E: full run against `EPFLiGHT/fully-open-meditron` on dev box (~10 minutes expected).

### 3.8 Effort

- Day 1 AM: schema + Rust scaffolding + migration
- Day 1 PM: Python CLI + cosmopedia-clean-room core (most code is in `multiprocessing.Pool` orchestration; algorithm itself is ~80 lines)
- Day 2 AM: tests + run against 3 corpora + write findings
- Day 2 PM: enforce gate in `eval_runs` insert + backfill reports for current scoreboard entries

---

## 4. Plan B — Pairwise Judge Protocol + MeditronFO Bake-off (Sprint 54, ~4 days)

### 4.1 Why pairwise

Current `JUDGE_PROMPT` in [scripts/run_healthbench_eval.py:97](../../scripts/run_healthbench_eval.py#L97) asks for absolute Likert 1-5 on `accuracy/completeness/relevance/safety`. Three known failure modes:

1. **Judge thinking budget swings ±11pp** (Sprint 51c)
2. **No positional debias** (not a swap-protected protocol)
3. **No statistical envelope** (n=20 point estimates without CI)

FOM's Auto-MOOVE protocol fixes all three:
- Pairwise → judges are calibrated by *contrast*, less by their internal absolute prior
- Each pair scored twice (swap on/off), winner un-swapped → cancels position bias by construction
- Bootstrap CI on win-rate

### 4.2 Adopted rubric (9 criteria, Likert 1-5)

From `auto_moove/common.py:32` in FOM:

```
Question comprehension
Logical reasoning
Relevance & completeness
Harmlessness
Fairness
Contextual awareness
Communication
Clarity
Alignment with guidelines    ← THIS ONE matters for "Living Clinical Evidence" pivot
```

The last criterion ("Alignment with guidelines") maps directly to [mimir_guideline_lineage_plan](../../memory) Sprint 52 MVP — pairwise judge can read the *Mimir guideline subgraph* into its system prompt and grade adherence.

### 4.3 Files to create

```
Mimir/
  scripts/
    run_healthbench_pairwise.py            # new runner (do NOT modify run_healthbench_eval.py)
  ro-ai-bridge/mimir-core-ai/src/evaluation/
    pairwise_runner.rs                     # parallel to runner.rs absolute-mode runner
  ro-ai-bridge/mimir-core-ai/migrations/
    20260615000000_pairwise_scoring.sql
```

### 4.4 Schema additions

```sql
-- Mark benchmark as pairwise-compatible
ALTER TABLE eval_benchmark_datasets
    ADD COLUMN supports_pairwise TINYINT(1) NOT NULL DEFAULT 0;

-- New scoring_fn value: 'pairwise_moove' (no schema change; just enum extension)

-- Track pair outcomes
CREATE TABLE IF NOT EXISTS eval_pairwise_scores (
    id                  VARCHAR(36)  NOT NULL PRIMARY KEY,
    run_id              VARCHAR(36)  NOT NULL,
    benchmark_item_id   VARCHAR(64)  NOT NULL,
    model_a             VARCHAR(255) NOT NULL,
    model_b             VARCHAR(255) NOT NULL,
    judge_model         VARCHAR(255) NOT NULL,
    -- Scored twice (swap=False and swap=True), then averaged
    a_scores_orig       JSON         NOT NULL COMMENT 'Per-criterion {comprehension,reasoning,...} 1-5',
    b_scores_orig       JSON         NOT NULL,
    a_scores_swapped    JSON         NOT NULL,
    b_scores_swapped    JSON         NOT NULL,
    winner              ENUM('A','B','Tie','Disagree') NOT NULL
        COMMENT 'Disagree = orig and swapped runs picked different winners',
    reasoning_orig      TEXT         NULL,
    reasoning_swapped   TEXT         NULL,
    created_at          TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_run    (run_id),
    INDEX idx_models (model_a, model_b)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

### 4.5 Runner behavior

For each `(item, model_a, model_b)` triple:
1. Generate answers from both models (cached by `sha256(prompt + temp + max_tokens)`)
2. Call judge **twice**:
   - swap=False: model_a presented as "Model 1"
   - swap=True: model_b presented as "Model 1"
3. Parse 9-criterion scores + winner from both
4. Un-swap the swapped run's winner labels (`"Model 1"` in swapped run = our model_b)
5. If both runs pick the same model → `winner=A` or `B`; if different → `Disagree`; if either is `Tie` → `Tie`
6. Aggregate over all items: win-rate of A over B with bootstrap 95% CI

### 4.6 First use case — MeditronFO-Gemma-3-27B bake-off

**Don't replicate the recipe** (§6). EPFL already released `EPFLiGHT/Gemma-3-27B-MeditronFO` on HuggingFace. Reported numbers:
- HealthBench Hard: 47.15% vs base Gemma-3 45.78% (+1.37 pp)
- vs MedGemma-27B: preferred 58.6% under their own Auto-MOOVE
- License: **Gemma terms — research only, no commercial deployment** ⚠️

**The bake-off:**

| Round | Model A | Model B | Verdict needed |
|---|---|---|---|
| 1 | `gemma-4-26b-a4b-it-4bit` (current local champion, our gemma-4 not gemma-3) | `EPFLiGHT/Gemma-3-27B-MeditronFO` (mlx-converted, q4) | Does FOM training transfer beat newer Gemma-4 base? |
| 2 | Winner of Round 1 | `gemini-3.1-flash-lite-preview` (cloud champion) | Does new local champ close the cloud gap? |

Run on 20 locked HBp items + 100 broader items + S1 insurance test queries (cross-tenant transfer check).

Expected outcomes and follow-ups:

- **If MeditronFO-Gemma-3-27B wins Round 1**: keep using it for `asgard_platform` benchmark/research tenant only (license blocks commercial). Pitch the *recipe* not the *model* for actual on-prem Asgard customers — i.e., justify Sprint 56+ replication effort because we've proven the recipe works.
- **If MeditronFO loses or ties**: gemma-4-26b stays champion. Recipe replication deprioritized. Saves ~3 weeks of fine-tuning work.

### 4.7 Use cases unlocked

1. **Asgard Underwriter (S2 multi-insurer)**: pairwise compare candidate insurance LLMs without needing physician-graded reference answers (which we don't have for insurance domain)
2. **Eir specialty agents** ([asgard_agent_registry](../../memory)): compare `eir-cardio` vs `eir-router` vs cross-specialty for the same question — pairwise is more natural for "which agent answered better" than absolute scoring
3. **PrimeKG retrieval ablations** ([three_retrieval_surfaces](../../memory)): pairwise on the *response* side (post-retrieval) tells us if retrieval improvements actually translate to better answers, since absolute Likert may flatten meaningful response-quality deltas
4. **Quarterly model refresh**: built-in protocol for "is the new model better than the current champion?" with statistical envelope, not point estimates

### 4.8 Effort

- Day 1: schema + Rust scaffolding + judge prompt port (9-criterion rubric)
- Day 2: pairwise runner Python; mlx-convert MeditronFO-Gemma-3-27B (test on Mac mini)
- Day 3: run Round 1 (gemma-4-26b vs MeditronFO-Gemma-3-27B); 20 items × 2 swaps × ~30s/judge = ~20 min
- Day 4: run Round 2 + write up + update [04_03 baseline](./04_03_HealthBench_Pro_Baseline_2026-05-04.md) with pairwise scoreboard

---

## 5. Legal / license risk

| Item | Status | Action |
|---|---|---|
| `EPFLiGHT/FullyOpenMeditron` repo | **No LICENSE file** | Do not copy code directly. Re-implement from paper + Apache-2.0 upstream (cosmopedia). |
| `swiss-ai/posttraining-data` repo | **No LICENSE file** | Same. Cited as origin in FOM docstring; cosmopedia is the real upstream. |
| `huggingface/cosmopedia` | **Apache 2.0** ✅ | Safe to copy + modify. This is our actual basis. |
| `EPFLiGHT/Gemma-3-27B-MeditronFO` HF model | **Gemma terms — research only** ⚠️ | Internal benchmarking on `asgard_platform` tenant is OK; **must not** ship to a commercial Asgard customer Mac mini. Tag as `commercial_use=false` in `ai_models.metadata`. |
| `EPFLiGHT/fully-open-meditron` HF dataset | Check on HF card | Likely CC-BY or similar; verify before bundling into Mimir ingest |

**Mitigation:** open a polite GitHub issue on `EPFLiGHT/FullyOpenMeditron` asking them to add a LICENSE — clarifies intent, doesn't block us either way since we're going clean-room from cosmopedia.

---

## 6. Why we are NOT replicating the training recipe

(Captured here so a future sprint planner doesn't redo this analysis.)

1. **Toolchain mismatch.** FOM uses axolotl + Slurm + vLLM rejection sampling (CUDA stack). Our locked toolchain is mlx-lm on M3 Ultra ([asgard_finetune_toolchain](../../memory)). Porting rejection-sampling-with-gold-label-resampling to mlx-lm is a from-scratch implementation, not a port.
2. **Teacher constraint.** FOM uses `gpt-oss-120b` as teacher. Eir agents are local-only ([feedback_eir_agents_local_only](../../memory)) — we'd have to swap teacher to medgemma-27b or gemma-4-26b. The student would no longer be distilling from a meaningfully stronger model, so the +1.37pp effect on HealthBench Hard would likely shrink or invert.
3. **Cheaper alternative exists.** EPFL already published `EPFLiGHT/Gemma-3-27B-MeditronFO` on HuggingFace. Plan B benchmarks it directly in 4 days. If it doesn't beat our current gemma-4-26b champion, the recipe likely won't either when ported and teacher-downgraded.

**Reconsider replication if** (a) HF MeditronFO weights win Plan B Round 1 by ≥5pp, AND (b) commercial-use blocker is unsolvable for shipping, AND (c) we have ≥3 weeks of dedicated M3 Ultra time. All three conditions probably hit Sprint 56+ at earliest.

---

## 7. Execution checklist

**Sprint 53 (Plan A):**
- [ ] Open issue on EPFLiGHT/FullyOpenMeditron requesting LICENSE
- [ ] Write `decontamination.rs` from cosmopedia upstream (Apache-2.0 attribution in module header)
- [ ] Write migration `20260601000000_decontam_metadata.sql`
- [ ] Write `scripts/run_decontamination.py` CLI
- [ ] Run against 3 corpora; commit reports
- [ ] Backfill `decontam_report_id` on existing scoreboard rows OR mark them `legacy_undecontaminated=true`
- [ ] Update [04_03 baseline](./04_03_HealthBench_Pro_Baseline_2026-05-04.md) with contamination column
- [ ] Add Tyr audit hook (or capture as Sprint 54 follow-up)

**Sprint 54 (Plan B):**
- [ ] Write migration `20260615000000_pairwise_scoring.sql`
- [ ] Port 9-criterion judge prompt into `pairwise_runner.rs`
- [ ] Write `scripts/run_healthbench_pairwise.py`
- [ ] mlx-convert `EPFLiGHT/Gemma-3-27B-MeditronFO` q4 (M3 Ultra at HQ)
- [ ] Tag model `commercial_use=false` in `ai_models.metadata`
- [ ] Run Round 1 + Round 2 bake-off
- [ ] Update [04_03 baseline](./04_03_HealthBench_Pro_Baseline_2026-05-04.md) with pairwise scoreboard table
- [ ] Decision: replicate recipe Sprint 56+? (only if §6 conditions hit)

---

## 8. Out of scope (this plan)

- Human rater calibration panel (204 raters) — defer to Sprint 56+ paired with [asgard_annotation_strategy](../../memory) Label Studio rollout
- FOM training recipe replication — see §6
- Ingesting the FOM 601k-example dataset into Mimir — only relevant *if* recipe replication happens
- Auto-MOOVE on the insurance side — same protocol works but needs S2 multi-insurer benchmark items first ([s1_insurance_baseline](../../memory))
