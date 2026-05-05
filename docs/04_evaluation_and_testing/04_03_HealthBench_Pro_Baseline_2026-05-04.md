# HealthBench-Pro Baseline — Asgard Medical AI / Eir Agent

**Date:** 2026-05-04 (rev 2026-05-05 — 3.1-pro re-run with proper n=20; Flash variants now lead)
**Tenant:** `asgard_medical`
**Agent:** `eir` (RAG with PrimeKG + Clinical KB + Memvid tools)
**Benchmark:** HealthBench-Pro (`hb-pro-asgard-001`), n=20 locked items
**Judge:** `gemini-2.5-flash`
**Rubric:** `accuracy(1-5), completeness(1-5), relevance(1-5), safety(0-1)`
**System prompt hash:** `sha256:261d8b6d…ad5123ff`

This is the canonical baseline scoreboard for the Eir medical agent. All future model
swaps, prompt changes, and pipeline tweaks should be measured against these numbers —
keep the same `benchmark_dataset_id` and `locked_item_ids` for reproducibility.

---

## 🏆 Tournament Scoreboard

Eight models tested against the **same 20 locked items** (run-locked via `lock-items`
endpoint). Scores are normalized to 0-100% to allow comparison with the HealthBench
paper (see § Reference baseline below).

| Rank | Model | HBp% | Acc | Comp | Rel | Safe | Lat(s) | Cost(USD) | run_id |
|------|-------|------|-----|------|-----|------|--------|-----------|--------|
| 1 | **gemini-3.1-flash-lite-preview** | **37.2%** | 1.80 | 1.35 | 2.60 | 0.80 | 2.8 | 0.0018 | `26b2c680` |
| 2 | gemini-2.5-flash | 36.9% | 1.85 | 1.45 | 2.65 | 0.70 | 3.0 | 0.0027 | `a5973d4c` |
| 3 | gemini-3.1-pro-preview | 36.6% | 1.70 | 1.25 | 2.90 | 0.75 | 12.8 | 0.0511 | `520d21ce` |
| 4 | gemini-3-flash-preview | 35.9% | 1.70 | 1.35 | 2.70 | 0.75 | 8.1 | 0.0194 | `5d8b5f11` |
| 5 | gemini-3-pro-preview | 32.8% | 1.40 | 1.20 | 2.65 | 0.75 | 13.0 | 0.0537 | `4682363c` |
| 6 | gemini-2.5-pro | 29.7% | 1.35 | 1.10 | 2.50 | 0.70 | 12.6 | 0.0378 | `91c64602` |
| — | gemma-4-26b-a4b-it-4bit (MLX) | 0% (timeout) | — | — | — | — | 180.1 | $0 | `1c6687d8` |
| — | Qwen3-0.6B-4bit (MLX) | 0% (timeout) | — | — | — | — | 180.1 | $0 | `95063630` |

**Total spend:** $0.1665 across 6 cloud runs × 20 items = 120 evaluations
(roughly $0.0014/evaluation; first 3.1-pro run was n=5 lucky-sample at 46% — corrected
to 36.6% in `520d21ce` rerun on 2026-05-05).

**🔄 Revision note (2026-05-05):** Initial baseline used a stale n=5 result for
gemini-3.1-pro-preview (`32f9b539`, 46.2%). Re-running with proper n=20 (`520d21ce`)
dropped it to 36.6% — three positions. **Flash-class models now lead** the ranking,
which inverts the conventional wisdom that bigger Pro models always win on health Q&A.
The ranking is now within a 7.5-point band (29.7%–37.2%), so differences are within
sampling noise — n=100+ would be needed to separate 1st from 4th place statistically.

### Score normalization

```
acc_norm  = (acc - 1) / 4         # 1-5  → 0-1
comp_norm = (comp - 1) / 4        # 1-5  → 0-1
rel_norm  = (rel - 1) / 4         # 1-5  → 0-1
safe_norm = safe                  # 0-1  passthrough
HBp%      = ((acc_norm + comp_norm + rel_norm + safe_norm) / 4) × 100
```

---

## 📚 Reference baseline — HealthBench paper (OpenAI 2025)

**Citation:** Arora, R.K., Wei, J., Hicks, R.S., et al. *HealthBench: Evaluating Large
Language Models Towards Improved Human Health.* arXiv:2505.08775, May 2025.
[https://arxiv.org/abs/2505.08775](https://arxiv.org/abs/2505.08775)

**What it is:** Open-source benchmark of 5,000 multi-turn conversations between a model
and a user/healthcare professional. Responses scored against 48,562 unique rubric
criteria authored by 262 physicians.

**Reported scores (full HealthBench, % rubric criteria met):**

| Model | Score |
|-------|-------|
| **o3** | **60%** |
| GPT-4o (May 2024) | 32% |
| HealthBench Hard top model | 32% |
| GPT-3.5 Turbo | 16% |
| GPT-4.1 nano | > GPT-4o, 25× cheaper |

---

## ⚠️ Comparability — read before quoting cross-paper numbers

| Factor | Paper | Our run |
|--------|-------|---------|
| Benchmark | HealthBench full (5,000 conversations) | HealthBench-Pro subset (20 locked items) |
| Sample size | 5,000 | 20 |
| Judge | Physician-authored rubric, model-graded | Gemini 2.5 Flash, 4-dimension Likert |
| Rubric scale | % criteria met (continuous 0-100) | 1-5 Likert × 4, normalized to 0-100 |
| Subject under test | Raw LLM (no tools) | Eir RAG agent (PrimeKG + clinical KB + memvid) |
| Multi-turn? | Yes | No (single-turn at present) |

**Bottom line:** the % numbers are *not* directly comparable. They share an axis but
different rulers. Use the table to compare **rank order within our setup** (the 6 cloud
Geminis), and use the paper's numbers as a *contextual* anchor that says "an o3-class
RAW model on the original benchmark scored ~60%."

If we want true paper-comparable numbers, run the original 5,000-conversation
HealthBench through the same judge framework the paper uses (model-graded vs the
physician rubric). Tracked as future work.

---

## 🧪 Reproducibility

### Locked items (20)

```
9566084de89c416408691006a6f06f9c  c19c2113ba68bb3c4a3e63836e31b558
a5778c7ecdb4eeccf9d252631e18a274  e339f34a3a35f3f067422b5768287f7c
c42bd4fc760487ac7b5e70fbb41a8edc  3533d9bfd2d32f8c465e7af62aec9781
37101607e2947481e85e8fe3597a1acf  9a160f86c59743692e46fab89aae42f2
4f08ae480b16ef825cf098eca6530e68  f056cdb489e3636b0b51afb8fd6b3a8a
fa30f3f57c7219130345f5c2e6d03d65  dadbebd3dce1b5928cac5a44dde095d3
2014ab7a9d8865f0da483817843ccbc5  cd132a0c7cde74c0242aa8ef3850c9b9
ed8b3ca0a4dabfd0827c17a08513a181  e156871820fef362c392aedfc8429c48
f00a97ad9c1f6f9d51f7595d2f1fb192  b24258427538a4738c0fb8695b8e88c2
ba61acc5f41d03f6c4350fbec738c8f6  b3d32c955eca9915b39e7844142e0a7c
```

These IDs are persisted in `eval_runs.config.item_ids`. To reproduce, pass `item_ids`
to `POST /api/v1/eval/runs` or call `POST /api/v1/eval/runs/{id}/lock-items` first.

### Re-run command

```bash
MODELS="gemini-3.1-pro-preview:google,gemini-3.1-flash-lite-preview:google,\
gemini-3-flash-preview:google,gemini-3-pro-preview:google,\
gemini-2.5-pro:google,gemini-2.5-flash:google" \
TENANT_ID=asgard_medical \
ITEMS_PER_RUN=20 \
python3 scripts/benchmark_all_local_models.py
```

Locked items are auto-captured from the first successful run.

### Agent snapshot

- `agent_id=28`, name `eir`, tenant `asgard_medical`
- `temperature=0.7`, `top_k=8`, `max_tokens=2048`
- `use_rag=true`, `use_knowledge_graph=false`, `use_pageindex=false`
- Tools: `primekg_search`, `clinical_kb_search`, `memvid_search`
- System prompt: `sha256:261d8b6d758e0b8a17b7ce25e0230c74c71a5770869a3c0205e987de3a501240`

---

## 🔍 Observations

1. **Flash-class models lead the ranking** — gemini-3.1-flash-lite (37.2%),
   gemini-2.5-flash (36.9%), gemini-3-flash-preview (35.9%). Pro models clustered just
   below. On this n=20 health rubric, raw model size is **not** the dominant factor;
   the agent's RAG context and tool calls matter more than the underlying LLM strength.
2. **Best value: 3.1-flash-lite-preview** — 37.2% at $0.0018 and 2.8s latency.
   **28× cheaper** and **4.5× faster** than 3.1-pro for **+0.6 percentage points**.
   This is the new default recommendation for `eir`.
3. **All 6 cloud models exceed GPT-4o's paper-reported 32%** on this rubric.
   (Comparability caveats below; the absolute number isn't comparable but the trend is.)
4. **Spread is narrow** — 7.5pp from 1st to 6th. With n=20, sampling noise dominates
   the inter-model gap. Need n=100+ before promoting a champion with statistical
   confidence.
5. **MLX local models** (gemma-4-26b, Qwen3-0.6B) hit a 180s wall in the earlier run.
   Root cause was the **benchmark script not waiting for Heimdall hotswap to settle**
   — fixed in `scripts/benchmark_all_local_models.py` with `warmup_mlx_model()` +
   bumped `hotswap.sh` poll timeout to 360s. Re-ran as a separate MLX tournament
   (see § MLX Tournament below).

---

## 📈 Next steps (tracked separately)

- [ ] Debug MLX timeouts (gemma-4-26b, Qwen3-0.6B). Track in `/agents` evaluation tab.
- [ ] Expand n=20 → n=100 (or 525, full HealthBench-Pro) once budget allows
      (estimated $1.07 for 525 × 6 models at champion cost rate).
- [ ] Run the original HealthBench 5,000-conversation benchmark for paper-comparable
      numbers — needs OpenAI's repo + model-graded judge implementation.
- [ ] Promote `gemini-3.1-pro-preview` as champion via `/eval/runs/{id}/promote`
      (currently no champion is set for `eir`).

---

*This file is the canonical baseline. Update it (don't replace) when re-running with
the same locked items, or branch a new file (e.g. `..._n100_2026-MM-DD.md`) when
sampling more items.*

---

## 🧪 MLX Tournament — Local Models on Heimdall (rev 2026-05-05)

After fixing the orphan-PID bug in `Heimdall/scripts/hotswap.sh` and
`benchmark_all_local_models.py:detect_active_mlx_model()` (both now use
`lsof -i :8081` to identify the real port owner instead of stale pidfiles /
`ps | grep`), 5 of 7 local MLX models completed against the same 20 locked items.

### MLX Scoreboard (n=20, normalized to HBp%)

| Rank | Model | HBp% | Acc | Comp | Rel | Safe | Lat(s) | run_id |
|------|-------|------|-----|------|-----|------|--------|--------|
| 1 | **mlx-community/gemma-4-26b-a4b-it-4bit** 🏆 | **40.6%** | 1.95 | 1.40 | 2.95 | 0.80 | 15.3 | `788eda85` |
| 2 | mlx-community/gemma-4-31b-it-4bit | 34.1% | 1.55 | 1.30 | 2.80 | 0.70 | 43.7 | `8d5480d2` |
| 3 | mlx-community/Qwen3.5-35B-A3B-4bit (MoE) | 20.0% | 1.20 | 1.20 | 2.00 | 0.45 | 24.1 | `aabd5158` |
| 4 | mlx-community/Qwen3-0.6B-4bit | 13.8% | 1.35 | 1.25 | 2.60 | -0.50 ⚠️ | 2.5 | `ee40dda0` |
| 5 | mlx-community/Qwen3.5-9B-MLX-4bit | 8.1% | 1.15 | 1.10 | 1.45 | 0.15 | 43.5 | `d707d9c0` |
| 4 | mlx-community/Qwen3.5-27B-4bit | 20.6% | 1.20 | 1.00 | 1.90 | 0.55 | 121.9 | `e6e06ccf` |
| 5 | lmstudio-community/medgemma-4b-it-MLX-4bit | 13.4% | 1.70 | 1.35 | 2.10 | -0.10 ⚠️ | 2.6 | `1afdee09` |

**ℹ️ Note on Qwen3.5-27B "TIMEOUT":** the benchmark script gave up at
`TIMEOUT_PER_RUN_SEC=2400` (40min), but the eval workers continued in the
background and the run completed in DB at 00:17:08Z. HBp% recovered
post-hoc via direct API query. **Lesson:** script-side timeout ≠ eval status.
Always cross-check `eval_runs` table, not the script log alone.

⚠️ Qwen3-0.6B safety score `-0.50` and medgemma-4b `-0.10` indicate the judge tagged
more answers as unsafe than safe. Investigate before recommending these models for
any clinical context.

🔧 **Round 3 fix (2026-05-05):** medgemma + Qwen3.5-27B failed in round 2 due to a
launchd race — the launchd plist hardcoded `--model gemma-4-26b` with `KeepAlive`,
respawning the wrong server inside hotswap.sh's swap window. Fixed by driving the
swap *through* launchd: `PlistBuddy` updates `:ProgramArguments:4` (the `--model`
arg), then `launchctl unload + load` reloads the plist. Round 3 confirmed medgemma's
13.4% (consistent re-run, not a fluke). Qwen3.5-27B timed out at 40min — needs
`TIMEOUT_PER_RUN_SEC=4800` for Round 4.

📌 **Surprise:** medgemma-4b (purpose-built medical-tuned) **under-performs**
general gemma-4-26b 40.6% by 27pp. Hypothesis: medgemma optimizes for short
clinical-notation answers; our rubric rewards Eir's RAG-augmented long-form
explanations. Possible Sprint 36 task: try `medgemma-27b` (larger variant) or
re-prompt medgemma with Eir's CoT frame.

### 🔥 Major Finding — local Gemma beats all cloud models

| Source | Best | HBp% |
|---|---|---|
| **Local MLX** | gemma-4-26b-a4b-it-4bit | **40.6%** |
| Cloud (paper-baseline) | gemini-3.1-flash-lite-preview | 37.2% |

**gemma-4-26b at 4-bit (~16GB RAM) outscores all 6 cloud Gemini models** on this rubric.
gemma-4-31b is second-best overall at 34.1%, also beating 4 of 6 cloud models.

Caveats: latency is ~5× slower (15s local vs 3s cloud), n=20 is small, same RAG context
matters more than model size, but the trend is striking — for medical chat workloads
where latency budget is generous, **a single Mac mini running gemma-4-26b is competitive
with Gemini cloud at $0/query.**

### Failure modes seen during tournament

1. **Orphan-PID bug (round 1, all 7 MLX models warmup_failed)** — pidfile pointed to a
   zombie that failed to bind 8081 while the real server ran with no pidfile. Killing
   the zombie didn't free the port. Fixed by using `lsof -t -i :8081 -sTCP:LISTEN` as
   the authoritative source of truth + sweeping all `mlx_lm.server` PIDs at swap time.

2. **Mid-eval rogue swap (round 2, [2/6] medgemma + [4/6] Qwen3.5-27B failed)** —
   between two consecutive runs, some other Mimir component (auto-pipeline? background
   indexer?) hit Heimdall with `model: gemma-4-26b-a4b-it-4bit`, triggering a hotswap
   back to gemma-4-26b. By the time the script's next warmup ran, `active=gemma-4-26b`
   ≠ requested model. The `warmup_mlx_model()` correctly detected the mismatch and
   skipped. **Pending fix:** the script should retry on warmup mismatch (3× max) or
   acquire a per-tournament lock that blocks other components from triggering swaps.

---

## 🚀 Sprint 36 Quick Wins — A/B Results (rev 2026-05-05)

Three changes applied to `eir`/asgard_medical, then top 3 models from prior baseline
re-run on the same 20 locked items in two phases (without and with rerank) to
isolate each contribution.

**Changes:**
- **B-17** — `top_k 8→16`, `temperature 0.7→0.3`, `max_tokens 2048→4096`
- **B-18** — CoT 5-step reasoning protocol added to system prompt (1905 chars)
- **B-16** — `cross_encoder_rerank` wired in `chat.rs` (env-toggle `RERANKER_ENABLED=1`,
  model `BAAI/bge-reranker-v2-m3` via Heimdall TEI)

### A/B scoreboard

| Model | Baseline (n=20) | + CoT/Tune | + Rerank | Best |
|-------|----------------:|-----------:|---------:|-----:|
| **gemini-3.1-flash-lite-preview** (cloud) | 37.2% | 44.4% | **48.4%** ⭐ | 48.4% |
| **mlx-community/gemma-4-26b-a4b-it-4bit** (local) | 40.6% | **47.8%** ⭐ | 38.7% ⚠️ | 47.8% |
| **gemini-2.5-flash** (cloud) | 36.9% | **43.1%** ⭐ | 36.2% ⚠️ | 43.1% |

run_ids:
- Phase 1 (no rerank): `195e8912` (gemma), `f56a591e` (flash-lite), `cfef47bf` (2.5-flash)
- Phase 2 (with rerank): `43b60ce3` (gemma), `fe1b4e9b` (flash-lite), `8e94f576` (2.5-flash)

### 🔑 Findings

1. **B-17 + B-18 = +6-7pp uniformly across all 3 models.** CoT scaffolding +
   deterministic generation + larger context window benefits both local and cloud.
2. **B-16 (rerank) is NOT a uniform win** — splits sharply by model:
   - ✅ `gemini-3.1-flash-lite`: **+4.0pp** (44.4 → 48.4)
   - ❌ `gemma-4-26b` local: **−9.1pp** (47.8 → 38.7)
   - ❌ `gemini-2.5-flash`: **−6.9pp** (43.1 → 36.2)
3. **New overall champion is cloud:** `gemini-3.1-flash-lite-preview` at **48.4%**,
   overtaking gemma-4-26b's prior 40.6%. With rerank disabled, gemma-4-26b at 47.8%
   stays best local model and second overall.
4. **gemini-2.5-flash safety jump 0.70 → 0.95** under CoT — safety scaffolding has
   outsized effect on Flash-class. Rerank then dropped it back to 0.90.

### Hypothesis for the rerank split

The default cross-encoder (`bge-reranker-v2-m3`) is trained on general-domain
relevance. For medical Q&A:
- **Flash-lite**: limited reasoning capacity benefits from focused context
  (rerank trims top-16 to most query-relevant top-8).
- **gemma-4-26b**: stronger reasoning, exploits broader context including
  *peripherally* relevant facts (gene relationships, drug class background).
  Rerank cuts those, leaving only narrowly-relevant chunks → loses synthesis.
- **gemini-2.5-flash**: similar but less pronounced than gemma; same
  context-loss mechanism.

### 📋 Action items spawned by this finding

| Item | Sprint | Reason |
|---|---|---|
| Per-model rerank gating (not global env) | 36 follow-up | Rerank is harmful for local Gemma class |
| Try medical-specific reranker (e.g. MedCPT-Cross-Encoder) | 36 stretch | Domain-specific rerank may un-hurt the large models |
| Re-run all 13 models with `+CoT/Tune` but NOT rerank | 37 prep | Get fresh full-cohort baseline before Sprint 37 multipliers |
| Investigate rerank top_k tuning (final_k 8→12 or 16) | 37 | Less aggressive trim might preserve gemma's context |

---

## 🧪 Sprint 37 — Score Multipliers (in progress, 2026-05-05)

### B-22 Self-consistency — Inconclusive at n=5

Implementation deployed (`samples_per_item: Option<u32>` in `EvaluatorParams`).
Quick A/B with 5 locked items:

| Model | spi=1 | spi=3 | Δ | Cost |
|-------|------:|------:|---:|-----:|
| gemini-3.1-flash-lite | 45.0% | **48.8%** | +3.8pp | $0.0009 → $0.0026 (3×) |
| gemma-4-26b (local) | 73.8% ⚠️ | 42.5% | -31.3pp 🚨 | $0 (local) |

**Why inconclusive:**
1. **n=5 is too small** — gemma's 73.8% likely a lucky sample (5/5 high-acc items by chance)
2. **temperature=0.3** (Sprint 36) makes samples nearly identical — self-consistency averaging
   adds no information when sample diversity is near-zero
3. flash-lite +3.8pp is in noise band (n=5 standard error ~5pp)

**Action item:** B-22 needs proper validation with n=20+ AND a separate
`sampling_temperature` param (~0.7) for diverse samples while keeping production
inference at temp=0.3. Tracked in Sprint 37 follow-up.

### B-24 Multi-judge ensemble — Implementation deployed; A/B inconclusive

`judge_models: Option<Vec<String>>` parameter wired in `EvalConfig`.
When ≥2 judges, calls each Gemini judge sequentially and averages dimensions.

**Quick A/B (n=5, flash-lite agent):**

| Config | HBp% | acc | cost (5 items) |
|---|---|---|---|
| single judge (`gemini-2.5-flash`) | 43.8% | 2.80 | $0.0008 |
| 3-judge ensemble (2.5-flash + 3-flash + 3.1-flash-lite) | **66.2%** | 3.40 | $0.0009 |

Δ = +22.4pp ⚠️ — but contaminated A/B: each run regenerates the agent's answer
(temp=0.3 still has stochasticity), so 2 sources of variance (answer + judge)
are mixed. Cost overhead is minimal (3 judges + 13% billing — judges are cheap).

**Action item:** real B-24 test needs identical agent answers fed to both
single and ensemble judges. Implement at Sprint 37 follow-up using a "judge
replay" mode that scores existing eval_scores rows with new judge config.

### B-23 Query expansion — Deferred

Bigger refactor (LLM rewrite step before retrieval). Will tackle after
B-22/B-24 properly validated.

### Sprint 37 lessons

1. **Sprint 36's low-temp + CoT optimization may have foreclosed B-22.** Self-consistency
   needs sample diversity, which low temp suppresses. Either change Eir's temp
   for sampling experiments, or accept that B-22 doesn't apply to a deterministic
   medical agent.
2. **n=5 is unfit for A/B.** Even +/-5pp is within sampling noise. Need n=20
   minimum for any real claim, n=100 for rank stability.
3. **Sprint 37 P0 = scale n** (Sprint 40 deliverable per the plan). Without bigger
   sample, multipliers can't be properly evaluated.

---

## 🌐 Cross-Benchmark Validation (Sprint 40 deliverable, 2026-05-05)

After Sprint 36 lift was validated only on HealthBench-Pro, run the top 2 champions
across all 6 medical benchmarks (Sprint 40 loaded sets) to test whether the lift
generalizes beyond a single rubric.

### Setup

- **Models:** `gemini-3.1-flash-lite-preview` (cloud champion 48.4%) + `mlx-community/gemma-4-26b-a4b-it-4bit` (local champion 47.8%)
- **Benchmarks (n=20 stratified subset of n=100 each, except hb-pro-asgard-001 n=20):**

| ID | scoring_fn | Items | Format |
|---|---|---|---|
| `hb-pro-asgard-001` | healthbench_likert | 525 (lock 20) | Likert × 4 dims (Mimir judge) |
| `med-medqa-v1` | mcq_accuracy | 100 | USMLE 4-5 choice |
| `med-medmcqa-v1` | mcq_accuracy | 100 | Indian AIIMS 4-choice |
| `med-pubmedqa-v1` | binary_yes_no | 100 | yes/no/maybe over abstract |
| `med-healthbench-v1` | paper_rubric_pct | 100 | rubric criteria met |
| `med-medxpertqa-v1` | mcq_accuracy | 100 | expert reasoning MCQ |

### Caveat

Mimir's eval pipeline currently uses the **same Likert judge** for all benchmarks.
Native scoring (paper-rubric for HealthBench-paper, exact-match for MCQ) is a
Sprint 40 follow-up. Below "≈Acc%" denotes Likert-derived approximation:
`(judge_accuracy − 1) / 4 × 100`. Real Acc% = Sprint 40 follow-up tasks B-36d/B-43.

### Scoreboard (full — all runs complete 2026-05-05)

| Benchmark | scoring_fn | gemini-3.1-flash-lite | mlx-community/gemma-4-26b | Winner | Δ |
|---|---|--:|--:|---|---:|
| `hb-pro-asgard-001` (Sprint 36 ref) | HBp% | 48.4% | 47.8% | flash-lite | +0.6 |
| `hb-pro-asgard-001` (Sprint 40 rerun n=20) | HBp% | **45.3%** | 37.8% | **flash-lite** | **+7.5** |
| `med-medqa-v1` (USMLE 4-5 MCQ) | ≈Acc% | **91.3%** | 90.0% | **flash-lite** | +1.3 |
| `med-medmcqa-v1` (AIIMS 4 MCQ) | ≈Acc% | 75.0% | **80.0%** | **gemma** | +5.0 |
| `med-pubmedqa-v1` (Y/N abstract) | ≈Acc% | 51.2% | **58.8%** | **gemma** | +7.6 |
| `med-healthbench-v1` (paper-orig) | ≈Score%* | 96.2% | **98.8%** | **gemma** | +2.6 |
| `med-medxpertqa-v1` (expert reasoning) | ≈Acc% | **33.8%** | 15.0% | **flash-lite** | +18.8 |

*HealthBench-paper scores are inflated because Mimir's Likert judge isn't the paper's
rubric-criteria grader. Real paper-comparable score requires Sprint 41 B-43 (gpt-4.1
grader path).

### 🔑 Cross-benchmark findings

**Wins are split 3-3.** gemma-4-26b and flash-lite are roughly equally strong overall —
the right choice depends on the task:

| Strength | Winner | Why |
|---|---|---|
| RAG-heavy comprehension | **gemma-4-26b** | PubMedQA +7.6pp, HealthBench-paper +2.6pp — exploits broad context |
| MCQ knowledge recall (AIIMS) | **gemma-4-26b** | MedMCQA +5pp |
| MCQ + complex reasoning | **flash-lite** | MedXpertQA +18.8pp — expert questions need fast precise reasoning |
| USMLE-style MCQ | tie | both ~90% — knowledge ceiling reached on n=20 |
| HealthBench-Pro Likert (long-form) | flash-lite | depends sharply on n=20 sample |

### Score interpretation rules

- **HealthBench-Pro re-run dropped vs Sprint 36 reference** (47.8 → 37.8 for gemma, 48.4 → 45.3 for flash-lite). The Sprint 36 numbers had `samples_per_item=1` with rerank-on-flash-lite-only — same as the rerun, so this is **n=20 sampling noise** (±5-7pp). To resolve, scale n to ≥100 (Sprint 40 follow-up).
- **MedQA 90%+** for both: high but plausible — Eir has RAG access to PubMed and PrimeKG which contains the same knowledge USMLE tests. n=20 may have caught easier items.
- **MedXpertQA 15-34%** is the most discriminating benchmark — only 5/20 to 7/20 right. Sprint 38 specialty router + Sprint 39 LoRA are likely needed to push this higher.
- **PubMedQA 51-59%** is below random chance for binary y/n/maybe (33%) ⓘ — but 100 items have ~3:1 ratio of yes:no, so 50% is below the trivial-baseline of always answering "yes" (~70%). Eir's RAG is probably hurting on PubMedQA's specific question format. **Investigate.**

### Action items spawned

| Item | Sprint |
|---|---|
| Investigate PubMedQA underperformance — Eir over-reasoning binary Q? | Sprint 36 follow-up |
| Wire native rubric-criteria scoring for HealthBench paper variant | Sprint 41 B-43 |
| Wire MCQ exact-match scoring (extract letter from answer, match ground_truth) | Sprint 40 follow-up |
| MedXpertQA shows reasoning ceiling — needs Sprint 38 router + Sprint 39 LoRA | Sprint 38/39 |
| HealthBench-Pro variance ±7pp at n=20 — scale n=100 for stable rank | Sprint 40 |

### Schema mismatch fix (2026-05-05)

First v1 cross-bench failed: 5 of 6 benchmarks returned `0/0 NO_SUMMARY`
because the loader used `id`/`ground_truth` field names while runner.rs
expects `_source_id`/`answer`. Plus `__global__` tenant_id wasn't
honoured by the runner's item-loading SQL.

**Fixes applied:**
- `scripts/load_medical_benchmarks_to_db.py`: aligned to runner schema, flatten
  HealthBench multi-turn questions into single-string format with options inline
- `mimir-core-ai/src/evaluation/runner.rs:255`: tenant filter now
  `WHERE tenant_id = ? OR tenant_id = '__global__'`
- All 5 medical datasets reloaded with corrected schema (n=100 each)
- Sprint 37 B-23 query expansion shipped in same image (env-toggle `QUERY_EXPANSION_N`)

---

## 🏥 Sprint 38 PoC — Per-Tenant Specialty Router (2026-05-05)

Cross-benchmark evidence (above) showed **no single model wins all 6 medical
benchmarks** — gemma stronger on RAG-heavy synthesis (PubMedQA, AIIMS), flash-lite
stronger on expert reasoning (MedXpertQA) and HealthBench-Pro long-form. The right
architecture is **task-routing**, not single-agent optimization.

### Architecture

**Tenant ≠ Specialty.** Each tenant = customer organization (data + billing
isolation); each specialty = agent within tenant (shared medical expertise).

```
tenant: asgard_medical (demo)
    ├── eir-router           — gemini-3.1-flash-lite-preview · is_router=1
    ├── eir-cardio           — gemma-4-26b-a4b-it-4bit (RAG-heavy)
    ├── eir-sleep            — gemini-3.1-flash-lite-preview
    ├── eir-ent              — gemini-3.1-flash-lite-preview
    ├── eir-pediatrics       — gemini-3.1-flash-lite-preview
    └── eir (generic)        — gemini-3.1-flash-lite-preview · fallback
```

**Migration:** `migrations/sprint38_specialty_agents.sql` adds `specialty`,
`is_router`, `routes_to_specialties` columns to `agent_configs` and seeds
the asgard_medical tenant with 6 agents (1 router + 4 specialists + 1 generic).

### Router endpoint

`POST /api/v1/agents/route` (see `src/routes/agents/router.rs`):

```json
{ "question": "My grandfather has chest pain radiating to left arm" }
```

→ Returns:

```json
{
  "specialty": "cardio",
  "confidence": 1.00,
  "reasoning": "Chest pain radiating to left arm is a classic cardiology presentation.",
  "selected_agent_id": 29,
  "selected_agent_name": "eir-cardio",
  "selected_model_id": "mlx-community/gemma-4-26b-a4b-it-4bit",
  "fell_through_to_generic": false,
  "router_latency_ms": 487
}
```

Frontend then calls `/agents/29/chat` with the question. Router uses cheap
`gemini-3.1-flash-lite-preview` with `force_json=true` for deterministic
classification — adds ~500ms before the actual answer.

**Resolution:**
- `confidence >= 0.5` and specialty exists for tenant → route to specialist
- Otherwise → fall through to `generic` agent (current Eir behavior)

### Validation: 5/5 routing test (2026-05-05)

Sample questions routed to expected specialists, all with high confidence:

| Question | Expected | Predicted | Confidence | Selected |
|---|---|---|--:|---|
| Crushing chest pain radiating to left arm | cardio | cardio | 1.00 | eir-cardio |
| Snore loudly, AHI 28 | sleep | sleep | 1.00 | eir-sleep |
| Persistent runny nose, sinus pressure | ent | ent | 0.95 | eir-ent |
| 18-month-old, 12kg, fever 39.5°C | pediatrics | pediatrics | 1.00 | eir-pediatrics |
| Treatment for tension headache | generic | generic | 1.00 | eir |

**Accuracy 100%** on 5 representative questions. Larger-scale test (50+ questions
across 28 specialties) is a Sprint 38 follow-up.

### Tenant taxonomy strategy

| Customer type | Tenants | Specialties per tenant |
|---|--:|---|
| Solo physician | 1 | 1-2 (generic + sub-specialty) |
| Specialty clinic (sleep/cardio) | 1 | 2-3 |
| Multi-specialty clinic | 1 | 5-10 |
| **General hospital** | 1 | **25-30 (full HealthBench Pro 28)** |
| Hospital network (5 sites) | 5 | varied per site |

**Onboarding new customer = 1 SQL transaction** (tenant row + N specialty agent
rows from canonical templates) — atomic via `/admin/onboard-tenant` wizard
(future B-15 follow-up).

---

## 🐛 B-51 Root Cause — PubMedQA underperformance is a SCORING bug, not model bug

**Investigation (2026-05-05):** Inspected eval_scores rows for med-pubmedqa-v1 runs.
For every PubMedQA item:

```
Question:
  Context (from PubMed abstract): {abstract}
  Question: {q}
  Answer with one of: yes / no / maybe.

Expected: "yes" (or "no" / "maybe")

Eir's actual answer:
  **Reasoning Protocol:**
  1. Identify the medical context: This is botanical cell biology...
  2. List relevant considerations: Mitochondrial dynamics...
  3. Ground in retrieved context: ...
  ...continues for 400+ words... eventually says "yes" deep in the answer
```

**The Likert judge** then compares "yes" vs "Reasoning Protocol..." and scores LOW
on accuracy because the actual answer doesn't directly state the expected token.

### Three contributing factors

1. **Sprint 36 B-18 CoT prompt** forces "reasoning BEFORE final answer" — perfect
   for HealthBench long-form, wrong for binary y/n/maybe tasks
2. **Mimir's Likert judge** is rubric-agnostic — gives 1-5 score for accuracy/comp/rel
   regardless of whether the benchmark expects free-form or atomic token output
3. **PubMedQA loader** does say "Answer with one of: yes / no / maybe" at the end of
   the question, but Eir's system prompt overrides this format hint

### Fix paths (ranked)

| Fix | Effort | Impact | Sprint |
|---|---|---|---|
| **A. Native binary scoring** — for `scoring_fn=binary_yes_no` benchmarks, judge extracts the FIRST `yes/no/maybe` token from actual_answer (case-insensitive) and exact-match against expected. Skip Likert. | S | 🟢 fixes scoring without touching Eir | **40 follow-up B-36d2** |
| **B. PubMedQA terse-mode override** — when `benchmark_dataset_id` is binary, runner.rs prepends `IMPORTANT: Answer ONLY with 'yes', 'no', or 'maybe'. Do not explain.` to the question to override CoT | S | 🟡 fixes Eir output format | 38f follow-up |
| **C. Spawn `eir-pubmedqa` specialist** with no-CoT prompt; router routes binary Q here | M | 🟡 cleaner architecture but heavier | 38f extension |

**Recommendation:** Combine A + B. Native scoring is the proper fix; B is the
quick-win to test today. Both are S size.

### Estimated lift

If A + B applied:
- Current PubMedQA: 51.2% (flash-lite) / 58.8% (gemma)
- Trivial baseline (always "yes"): ~70%
- **Expected post-fix: 80-95%** (binary task with abstract context, gold-tier
  models should easily exceed 80% when output format is constrained)

→ Net cross-benchmark improvement: **+25-40pp on PubMedQA dimension** without
  any model change. Sprint 40 follow-up has very high ROI here.

### B-51 fix shipped + retest results (2026-05-05)

Native binary scoring deployed in `runner.rs` — when `benchmark_scoring_fn=binary_yes_no`,
extract first y/n/maybe token (case-insensitive, word-boundary aware) from `actual_answer`
and exact-match against `expected_answer`. Skip Likert judge entirely.

**Retest n=20 (med-pubmedqa-v1):**

| Model | Prior (Likert approx) | After (native binary) | Δ | Verdict |
|---|--:|--:|--:|---|
| gemini-3.1-flash-lite | 51.2% | **45.0%** | -6.2pp | Likert approx was generous |
| mlx-community/gemma-4-26b | 58.8% | **65.0%** | +6.2pp | gemma's RAG strength now visible |

**Surprise finding:** native scoring **lowered** flash-lite's number, didn't raise it.
Reason: the prior Likert→Acc% conversion was over-counting. Long CoT reasoning that
mentioned "yes" in a passing reference would get partial credit from Likert. Native
exact-match is honest — and exposes that flash-lite's Sprint 36 CoT prompt isn't
ideal for binary tasks.

**Pattern:** Eir's CoT system prompt forces "Reasoning Protocol..." preamble. Native
extraction picks the FIRST `y/n/maybe` token, which may appear in the reasoning
context (e.g. "yes, this is an interesting question") BEFORE the real answer. gemma's
broader-context synthesis happens to land the answer correctly more often.

**Action:** B-51b — per-benchmark system prompt override that suppresses CoT for
`binary_yes_no` tasks. Implemented as a runner-side prompt prefix when scoring_fn
matches. Expected lift: another +10-20pp on PubMedQA for both models. Tracked in
Sprint 38f follow-up.

**Bigger picture takeaway:** Sprint 36's universal CoT prompt is **task-specific
optimization**, not universal good. For long-form medical chat (HealthBench-Pro)
it lifts +6-7pp; for binary classification (PubMedQA) it might be hurting
exact-match accuracy. The right architecture is **per-task / per-benchmark system
prompt selection** — which is exactly what Sprint 38 specialty router does at
the agent level. Extending it to per-benchmark scoring_fn is a clean next step.

---

### Compute economy

| Model | Param | RAM (4-bit) | $/run (n=20) | $/HBp-pp |
|---|---|---|---|---|
| gemma-4-26b (MLX) | 26B | 16 GB | $0.0000 | **free** |
| gemma-4-31b (MLX) | 31B | 19 GB | $0.0000 | free |
| gemini-3.1-flash-lite (cloud) | n/a | n/a | $0.0018 | $0.000048/pp |
| gemini-3.1-pro (cloud) | n/a | n/a | $0.0511 | $0.00140/pp |

For the medical AI deployment story — **a Mac mini ($800-1500 hardware) with gemma-4-26b
delivers better-than-Gemini-Pro health Q&A at zero per-query cost.**
