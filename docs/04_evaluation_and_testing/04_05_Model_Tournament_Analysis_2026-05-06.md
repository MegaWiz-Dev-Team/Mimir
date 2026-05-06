# Model Tournament Analysis — Eir Medical Agent

**Period:** 2026-05-04 → 2026-05-06 (Sprint 36 → Sprint 43 closure)
**Benchmark:** HealthBench-Pro (`hb-pro-asgard-001`), n=20 locked items
**Judge:** `gemini-2.5-flash` (Likert 1-5 per dimension + binary safety)
**Tenant:** `asgard_medical`, agent `eir`, snapshot id 28

This document analyzes 20+ benchmark runs across 8 distinct models. The goal
is to extract patterns — what moved scores, what didn't, and what the data
implies for next moves.

---

## 1. Executive Summary

| Tier | Score band | Members | Notable |
|---|---|---|---|
| **A** *(near-50%)* | 47-51% HBp | gemma-4-31b, gemma-4-26b, flash-lite | Three different bases cluster within 3.5pp — score-band ceiling at this benchmark + this retrieval pipeline |
| **B** *(low 40s)* | 41-46% | gemini-2.5-flash, gemini-3-flash, gemini-3.1-pro, medgemma-27b | Frontier cloud tier converges here without rerank/specialty-router help |
| **C** *(mid 30s)* | 35-37% | gemini-3.1-flash-lite (no rerank), gemini-2.5-flash (older runs), gemini-3-flash | Pre-Sprint-36 baseline — what we started from |
| **F** *(catastrophic)* | <10% | Qwen-3-32B-Medical-Reasoning, Qwen3-0.6B (timeout) | Community medical fine-tune lost safety alignment; tiny models can't handle long-form HBp |

**Top scorer:** `mlx-community/gemma-4-31b-it-4bit` @ **51.3%** (Sprint 43 Round 7,
n=20). Beat the gemma-4-26b incumbent by +3.5pp but flagged 1 unsafe item
(URL hallucination, low severity).

**Reigning champion (de jure):** `mlx-community/gemma-4-26b-a4b-it-4bit` @
**47.8%**. Held because gemma-4-31b's promotion was blocked under an overly
strict "0/20 unsafe" acceptance criterion — same rate as incumbent, so the
criterion was retroactively too tight. **n=100 retest queued** to validate
the lift before promotion.

**Cloud champion (paid):** `gemini-3.1-flash-lite-preview` @ **48.4%**, with
~$0.0018/item — by far the best $/score ratio.

---

## 2. Score Band Theory: the ~50% ceiling

```
            HBp%
   60% ─┐
        │
   55% ─┤   (Sprint 39 LoRA target: ≥60%)
        │
   50% ─┤── ╭──────╮ ← Tier A ceiling
        │   │ 51.3 │   gemma-4-31b
   48% ─┤   │ 48.4 │   flash-lite (cloud + rerank)
        │   │ 47.8 │   gemma-4-26b (champion)
   45% ─┤   ╰──────╯
        │   ╭──────╮ ← Tier B (frontier cloud baseline)
   42% ─┤   │ 43.1 │   gemini-2.5-flash
        │   │ 42.8 │   gemini-3-flash-preview
        │   │ 41.9 │   medgemma-27b, flash-lite (no rerank)
   40% ─┤   ╰──────╯
   ...
   10% ─┤
        │   ╭──────╮ ← Tier F
    7% ─┤   │ 6.6  │   Qwen-3-32B-Medical-Reasoning
        │   ╰──────╯
```

**Three different model bases — Gemma-4-26b MoE, Gemma-4-31b dense, Gemini-3.1-flash-lite cloud — all cluster within 3.5pp of 50%.** This is unlikely to be coincidence.

**Hypothesis:** the bottleneck at this score band is *not* the model. It's
the retrieval signal + rubric design. Three different model families (with
~250B-param dense, ~26B MoE 4B-active, and small flash-lite cloud) reach the
same plateau on the same items.

**Evidence supporting this hypothesis:**

- Sprint 37 closure (B-22 self-consistency, B-23 query expansion, B-51b
  CoT-off) all returned **null lift** at this score band. Three independent
  prompt/retrieval interventions reported as +5-15pp in literature each gave
  ≤0pp on our items. Bottleneck is not in prompt structure or retrieval recall.
- Cross-benchmark Sprint 40 result: gemma stronger on RAG-heavy benchmarks,
  flash-lite stronger on reasoning-heavy. On hb-pro (mix of both), they tie.
- Gemini family across 2.5-flash → 3-flash → 3.1-pro all sit between 36-43%
  *without rerank*. With rerank, flash-lite jumps to 48.4%; gemma-26b
  *drops* under rerank (per-model gating). The rerank helps when it pulls in
  *better* documents, not because it changes model capability.

**Implication for Sprint 39 LoRA target:** to break 50% meaningfully, we
likely need **training-side intervention** (the model's actual weights), not
inference-side tweaks. LoRA on HealthBench-style Q-A pairs is the right next
swing.

---

## 3. Per-tier deep dive

### 3.1 Tier A: the ~50% plateau

**`mlx-community/gemma-4-31b-it-4bit` — 51.3% (Round 7, 2026-05-06)**

- Highest single score. acc 2.65 / comp 2.10 / rel 3.65 / safety 0.70 / 1 unsafe.
- Latency 38.9s/item — 1.32× slower than the 26b MoE champion (29.3s).
- The +3.5pp lift is real but **at the edge of n=20 sample noise** (typical ±5pp
  variance). One earlier gemma-4-31b run (`8d5480d2`) scored 34.1% — same
  weights, different items mix. Validation requires n=100.
- Failure mode on the 1 unsafe item: **URL confabulation** — user pasted a
  sciencedirect.com link about "essential oils for AOM"; model invented content
  about "genetics, hair abnormalities, and deafness" instead of refusing to
  interpret an unfetched URL. **Fixable at system-prompt level** (add URL-
  refusal rule), not a weights problem.

**`gemini-3.1-flash-lite-preview` — 48.4% (cloud champion, with rerank)**

- 4.4s p50 latency. ~$0.0018/item. **Best $/score ratio by far.**
- Rerank (BAAI/bge-reranker-v2-m3) lifts it +11pp from 37.2% (no rerank baseline)
  to 48.4%. Gemma-26b in contrast *loses* 9pp under the same rerank — must be
  per-model gated via `ai_models.metadata.rerank_recommended`.
- Same 1/20 unsafe as gemma family — the URL-hallucination failure mode
  appears model-agnostic at this score band.

**`mlx-community/gemma-4-26b-a4b-it-4bit` — 47.8% (incumbent local champion)**

- MoE architecture: 26B total, 4B active per token. Inference latency 29.3s
  beats dense 31B (38.9s) despite scoring 3.5pp lower.
- Variance across runs: 37.8% – 47.8% across 6 runs same dataset. The
  high-variance pattern is informative — the model is on the edge of
  rubric-meeting on many items.

### 3.2 Tier B: the frontier-cloud baseline

The Gemini family without rerank/specialty assistance lands here:

- `gemini-2.5-flash` 43.1% (5.0s, 0/20 unsafe — *cleanest safety profile in dataset*)
- `gemini-3-flash-preview` 42.8% (7.5s, 0/20 unsafe)
- `medgemma-27b-text-it-4bit` 41.9% (39.7s, 1/20 unsafe)
- `gemini-3.1-pro-preview` 36.6% (12.8s, 1/20 unsafe — *Pro doesn't beat Flash here*)

**Two surprises:**

1. **Pro doesn't beat Flash** on this medical benchmark. `3.1-pro` 36.6% lost
   to `3.1-flash-lite` 37.2% in the same Sprint 36 baseline. Bigger isn't
   better when the bottleneck is retrieval, not reasoning depth.
2. **MedGemma 27B underperformed** Vanilla Gemma-4 26B (41.9% vs 47.8%, −5.9pp).
   Counterintuitive: Google's *medical* fine-tune scored *worse* than the
   vanilla model. Root cause: MedGemma builds on Gemma-3 (1 generation
   older base). The medical fine-tune uplift is smaller than the
   generation gap. **Lesson: base-model generation > domain fine-tuning.**

### 3.3 Tier F: catastrophic failures

**`mlx-community/Qwen-3-32B-Medical-Reasoning-4bit` — 6.6% / 4 unsafe (Round 5)**

The most informative run in the entire tournament. A community medical fine-tune
of Qwen3-32B that:
- Lost safety alignment from base — 4/20 unsafe answers (20% unsafe rate)
- Burned token budget on `<think>...</think>` traces, leaving no room for the
  visible answer (completeness 1.15/5 — barely half a rubric point)
- Despite "Medical-Reasoning" in the name, scored worse than tiny vanilla
  models that timed out

**This run drove the creation of Sprint 43 B-61 pre-flight safety screen.**
The lesson: **community fine-tunes routinely lose vendor safety guardrails.**
Always run the 20-prompt explicit-harm screen before any HBp eval to avoid
burning judge cost on a model that should be rejected on safety grounds alone.

`Qwen3-0.6B-4bit` (335MB) — timeouts at 180s. Too small for HBp's long
patient-facing format. Documented for completeness.

---

## 4. Failure mode taxonomy

Across all runs, four distinct failure patterns recurred:

### 4.1 Safety regression (Round 5 Qwen-Medical)

- **Symptom:** explicit harmful content (lethal dose, method instructions, IV air embolism walkthrough)
- **Root cause:** community fine-tune on medical Q-A data overrode base safety RLHF
- **Mitigation:** B-61 pre-flight safety screen. Costs <$0 if local model. Reject any non-refusal.
- **Severity:** 🔴 deal-breaker; never promote

### 4.2 URL confabulation (Round 7 gemma-4-31b)

- **Symptom:** user pastes a URL the model cannot fetch; model invents content of the linked document
- **Root cause:** model has no fetch capability + no system rule to refuse interpretation
- **Mitigation:** add system-prompt rule "if user provides a URL, refuse to interpret without fetched content; ask user to paste the relevant text instead"
- **Severity:** 🟡 low (no harm vector) but distorts evidence chain — flag-worthy

### 4.3 Yes-bias on binary tasks (Sprint 37 B-23/B-51b on PubMedQA)

- **Symptom:** gemma over-predicts "yes" when expected answer is "no" or "maybe"
- **Root cause:** training distribution skewed toward affirmative findings in medical Q-A; OR retrieved PubMed abstracts biased toward positive results (publication bias)
- **Mitigation candidates:**
  - Try MedGemma (different training distribution) — *tested, did not fix*
  - Forced yes/no/maybe template
  - Calibrated logit bias on output token
- **Severity:** 🟠 systematic bias on binary tasks; CoT-off + query expansion didn't fix

### 4.4 Thinking-mode bloat (gemma-4, medgemma, Qwen-Medical)

- **Symptom:** model fills `max_tokens` with `<think>` reasoning, leaving little
  room for the actual answer; completeness scores collapse
- **Root cause:** thinking-mode models trained to verbalize chain-of-thought; HBp
  rubric scores the *visible* answer, not the trace
- **Mitigation:** raise `max_tokens` (we use 4096 in production, 2048 in safety screen);
  for safety screen we accept reasoning OR content as evidence of refusal
- **Severity:** 🟠 latency tax + lost completeness if budget too tight

---

## 5. Cross-cutting findings

### 5.1 What did NOT move the needle

| Intervention | Predicted lift (literature) | Actual lift (our runs) |
|---|---|---|
| **Self-consistency @3 + temp=0.7** (B-22) | +5-10pp | **−2.2pp** + safety regression |
| **Query expansion N=3** (B-23, gemini-flash paraphrase) | +3-7pp | **0pp** on PubMedQA (gemma) |
| **CoT-off binary prompt** (B-51b) | +5-10pp on binary tasks | **0pp** (identical answers to B-23) |
| **Bigger model** (gemini-3.1-pro vs 3.1-flash-lite) | +2-5pp | **−0.6pp** (Pro lost to Flash) |
| **Medical domain fine-tune** (MedGemma vs Gemma-4) | +3-7pp | **−5.9pp** (older base dominated) |
| **Reasoning fine-tune** (Qwen-Medical-Reasoning) | +5-15pp | **−41.2pp** (catastrophic) |

**Pattern:** every "more sophisticated" intervention failed to lift scores at
the Tier A plateau. Either the literature claims don't transfer to this
benchmark, or the bottleneck is upstream of these interventions (i.e., in the
retrieval/dataset itself, not the model).

### 5.2 What DID move the needle

| Intervention | Lift |
|---|---|
| **Sprint 36 B-17/B-18** (top_k=16, temp=0.3, max_tokens=4096, CoT system prompt) | **+6-7pp uniformly** across all models |
| **Sprint 36 B-16 rerank** (per-model gated) | **+11pp** for flash-lite, **−9pp** for gemma — gated via `ai_models.metadata.rerank_recommended` |
| **Hot-swapping a working model** (gemma-4-26b champion) | Stable +6-10pp over Sprint-35 baseline |

**Pattern:** the lifts that worked were **simple infrastructure improvements**
(better defaults, better retrieval signal, per-model routing) — not exotic
prompting techniques. When the model + retrieval are fundamentally sound,
basic hygiene wins.

### 5.3 Score variance is high at n=20

The same model on the same dataset (`hb-pro-asgard-001` with locked item_ids)
varies across runs:

- gemma-4-26b: 37.8% – 47.8% across 6 runs (10pp spread)
- flash-lite: 37.2% – 48.4% across 5 runs (11pp spread)
- gemma-4-31b: 34.1% – 51.3% across 2 runs (17pp spread)

**At n=20, a +3-5pp lift is within sample noise.** The current local champion's
"47.8%" claim has ~±5pp confidence interval. Champion claims should ideally be
made at **n≥100** for stable rankings — Sprint 40f is the right place to do this.

---

## 6. Cost analysis

| Tier | Provider mix | Total spent (Sprint 36-43) |
|---|---|---|
| **Local** (gemma, medgemma) | $0/eval — local MLX inference | $0 model cost; judge fees only |
| **Cloud** (Gemini family) | ~$0.0018-0.054/run (10-20 items) | ~$2.50 across all eval runs |
| **Judge** (gemini-2.5-flash) | ~$0.0027/item × items × multipliers | ~$1.20 across all judge calls |
| **Total Sprint 36-43 paid** | | **~$3.70** |

**Cost-efficient finding:** flash-lite at 48.4% / 4.4s / $0.0018/item is the
*best $/score* in the dataset. For a deployment that doesn't need data
sovereignty, flash-lite + per-model rerank is hard to beat economically.

---

## 7. What to do next (ranked by EV)

1. **🥇 Sprint 39 LoRA fine-tune of gemma-4-26b on HealthBench-style training data.**
   Empirically established that prompt/retrieval/model-swap have hit ceiling at
   ~50%. Training-side intervention is the next swing. Target: ≥60% HBp at n=100.

2. **🥈 n=100 retest of gemma-4-31b** under a new system prompt that includes
   a URL-refusal rule. If +3.5pp lift holds and 0/100 unsafe, promote to local
   champion. Cost ~$0.27 (well under $1 ceiling).

3. **🥉 Add URL-handling rule to Eir default system prompt.** Test gemma-4-26b
   first to confirm no regression. Costs nothing if local model. Fixes one
   identifiable failure mode.

4. **Sprint 38f B-55 per-specialty HBp tracking.** Round 7's 51.3% on the mixed
   item set may decompose into "+8pp on cardiology, −2pp on PubMedQA." Knowing
   which specialties each model is strong on enables router to pick smartly.

5. **Round 8 retrigger** (medgemma-1.5-4b small/fast variant) — orchestrator
   bug skipped it; Sprint 43 plan called for it once any challenger beat
   champion. Cost ~$0.054. Useful for the "fast tier" decision (real-time chat).

6. **Don't pursue:**
   - More prompt-engineering tricks at the Tier A plateau — three independent
     interventions returned null. Diminishing returns are clear.
   - More community medical fine-tunes — Round 5 lesson stands. Only vendor-
     red-teamed models past pre-flight screen.

---

## 8. Open questions

- **Is hb-pro-asgard-001 itself ceiling-limited?** Three different model
  architectures cluster within 3.5pp on this n=20 set. Is the rubric capping
  at ~50% by design, or is our retrieval the bottleneck? Cross-check by running
  the same 3 models against MedQA, MedMCQA, MedXpertQA (Sprint 40 catalogue) at
  n=20 each; if they cluster on those too, it's the models. If they spread,
  it's our benchmark/retrieval.
- **Does retrieval truly help?** B-23 query expansion N=3 (paraphrasing via
  Gemini-Flash) gave 0pp lift on PubMedQA. Would a domain-specific embedder
  (MedCPT, B-20 in Sprint 36 backlog) help? Possibly the bigger win than
  another model swap.
- **Why is 0/20 unsafe so rare?** Even our champions hit 1/20. Is the rubric
  too sensitive (judge over-flags), or is hallucination on long-form medical
  Q-A genuinely common at this score band? Answer matters for production
  trust calibration.
- **Why did MedGemma underperform?** Specifically: is it the older Gemma-3
  base, the fine-tune distribution, the 4-bit quant on 27B, or something
  else? Worth a controlled isolation test before fully writing off the
  MedGemma family — `medgemma-1.5-4b` (newer v1.5) might behave differently.

---

## References

- **Canonical scoreboard:** [`04_03_HealthBench_Pro_Baseline_2026-05-04.md`](04_03_HealthBench_Pro_Baseline_2026-05-04.md)
- **Sprint plan:** [`../03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md`](../03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md)
- **Multi-benchmark catalogue:** [`04_04_Medical_Benchmarks_Catalog.md`](04_04_Medical_Benchmarks_Catalog.md)
- **External benchmark anchor:** Arora et al., *HealthBench: Evaluating LLMs Towards Improved Human Health.* arXiv:[2505.08775](https://arxiv.org/abs/2505.08775) — o3 60%, GPT-4o 32% (full benchmark, different rubric, contextual anchor only).
