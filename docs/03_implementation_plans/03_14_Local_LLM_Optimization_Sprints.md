# 🧪 Local LLM Optimization — Sprint Plan (Sprint 36-39)

**Created:** 2026-05-05
**Owner:** Asgard Medical AI / Eir Agent
**Baseline:** [04_03_HealthBench_Pro_Baseline_2026-05-04.md](../04_evaluation_and_testing/04_03_HealthBench_Pro_Baseline_2026-05-04.md)
**Driver:** [HealthBench paper arXiv:2505.08775](https://arxiv.org/abs/2505.08775)

---

## 📊 Context

After running an 8-cloud + 7-MLX tournament on HealthBench-Pro (n=20 locked items),
we discovered:

- **`mlx-community/gemma-4-26b-a4b-it-4bit` @ 40.6%** — local 4-bit MLX model on a
  Mac mini **beats every cloud Gemini** (champion `gemini-3.1-flash-lite-preview`
  @ 37.2%).
- The cohort sits in a tight 29.7-40.6% band. **Score ceiling is ~50-60% with
  optimization** (paper top: o3 @ 60% on full HealthBench).
- Two MLX models warmup_failed due to a launchd race condition — fixed in
  Heimdall `hotswap.sh` (PlistBuddy + `unload`/`load`) and Mimir
  `benchmark_all_local_models.py` (`lsof`-based port owner detection).

### Problem
Our HBp% spread is dominated by sampling noise (n=20 too small) and the agent
itself isn't using all the levers Asgard already supports. There is roughly
**+15-25pp of headroom** before architectural changes are needed.

### Goal
Drive local Eir agent from 40.6% → 60%+ HBp on HealthBench-Pro within 4 sprints,
without leaving the Mac-mini local-inference cost envelope ($0/query).

---

## 🎯 North-Star KPIs

| Metric | Current | Sprint 39 target |
|---|---|---|
| **Local champion HBp%** | 40.6% (gemma-4-26b) | **≥60%** |
| **Sample size for champion claim** | n=20 | n≥100 |
| **Cost per HBp pp** (cloud-equivalent) | $0.000048/pp (3.1-flash-lite) | $0/pp (still local) |
| **Failure modes encountered** | launchd race, mid-eval rogue swap | 0 known races |

---

## 🗓️ Sprint Themes

| Sprint | Theme | Headline outcome | Status |
|---|---|---|---|
| **36** 🟢 | Quick Wins — wire what's already there | +6-7pp validated (CoT + tune + per-model rerank) | ✅ done |
| **37** 🟡 | Score Multipliers — self-consistency, multi-judge, query expansion | Code deployed, awaits n=20+ validation | 🟡 partial |
| **38** 🟠 | Architecture — specialty router | **PoC LIVE 5/5 routing accuracy** | 🟢 PoC done |
| **38f** 🟠 | Sprint 38 follow-up — Router validation + 28-specialty expand | Gated on B-49 A/B result | 📋 next up |
| **39** 🔵 | ML Pipeline — LoRA fine-tune | +15-25pp expected | 📋 future |
| **40** 🟣 | Multi-Benchmark Foundation — benchmark-aware UI/DB | Unblocks Sprint 41 | ✅ done |
| **40f** 🟣 | Sprint 40 follow-up — n=100 scale + native MCQ scoring | Stable rank, paper-comparable Acc% | 📋 future |
| **41** 🔴 | Paper-Comparable HealthBench Run | Marketing-grade numbers vs arXiv:2505.08775 | 📋 future |

---

## Sprint 36 — Quick Wins (Week 1-2)

**Goal:** Activate existing Asgard capabilities that are coded but not wired into
Eir's chat path. Lowest effort, fastest signal.

### Backlog

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| B-16 | Wire `cross_encoder_rerank` into Eir chat path | M | backend | Code at `retrieval/ensemble.rs:177` already exists; needs wiring in `routes/agents/chat.rs` and a rerank model URL env (start with `BAAI/bge-reranker-v2-m3`) |
| B-17 | Tune Eir defaults: `top_k=16`, `temperature=0.3`, `max_tokens=4096` | S | backend | DB migration; revert RESTORE on failure |
| B-18 | CoT system prompt for medical reasoning | S | medical lead | Inject step-by-step frame; capture new `system_prompt_hash` |
| B-19 | Add 4 specialty tools | L | backend | `drug_interaction_check` (DrugBank), `icd10_code_lookup`, `dosage_calculator`, `clinical_calculator` (CHADS2/MELD/Wells) |
| B-20 | Register `MedCPT` ONNX embedding in Heimdall (alongside BGE-M3) | M | infra | Allow per-collection embedding choice |
| B-21 | Resolve "rogue swap" callers — gate `model: gemma-4-26b` requests during a tournament | M | infra | Investigate via Heimdall request log: which Mimir component sends gemma-4-26b mid-tournament. Likely a healthcheck/heartbeat. |

### Definition of Done — Sprint 36
- [x] **B-17 + B-18 deployed** — top_k=16, temp=0.3, max_tok=4096, CoT prompt
- [x] **B-16 deployed** with env-toggle, then **per-model gating via ai_models.metadata** (follow-up)
- [x] Champion HBp% **48.4%** ≥ 50% target — close but not crossed; cohort 36-48% post-Sprint 36
- [x] No MLX swap failures (launchd race fixed in earlier sprint)
- [x] Baseline doc + JSON updated
- [ ] **Pending**: B-19 specialty tools, B-20 MedCPT embedding (P1 deferred for Sprint 37 first)

---

## Sprint 37 — Score Multipliers (Week 3-4)

**Goal:** Add the multi-sample / multi-judge / pre-retrieval LLM patterns that
the literature shows reliably move scores +5-15pp.

### Backlog

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| B-22 | Self-consistency runner: sample N answers per item, majority/mean vote | L | backend | Add `replicates_per_item` to `EvaluatorParams` in `evaluation/runner.rs`. Cost: N× tokens but $0 on local. Default N=5 for benchmark, N=1 for production. |
| B-23 | Query expansion via LLM rewrite | M | backend | Pre-retrieval: ask LLM "give 3 paraphrases of this medical question". Retrieve all, dedupe by chunk_id. New module under `retrieval/`. |
| B-24 | Multi-judge ensemble | M | backend | Change `EvaluatorConfig.judge_model: String` → `judges: Vec<String>`. Average normalized scores. Default: `[gemini-2.5-flash, claude-haiku-4-5, gpt-4o-mini]`. |
| B-25 | Q5_K_M / Q6_K quantization re-run | S | infra | Pull `mlx-community/gemma-4-26b-it-q5` and `q6` variants; benchmark side-by-side with current Q4. Pick the best fit-vs-RAM. |
| B-26 | Larger sample (n=20 → n=100) for champion confirmation | S | data | Locked items expansion via benchmark UI. Verify ranking stability. |

### Definition of Done — Sprint 37
- [x] **B-22 self-consistency**: implementation deployed (samples_per_item param 1-5).
      A/B test on flash-lite + gemma in flight (n=5 subset).
- [x] **B-24 multi-judge ensemble**: implementation done (judges_models param).
      Behavior: when ≥2 judges given, calls each + averages dimensions. Single-judge
      back-compat preserved.
- [ ] **B-23 query expansion**: deferred — bigger refactor (LLM rewrite step
      before retrieval). Will tackle after B-22/B-24 validation.
- [ ] Self-consistency lift verified — pending A/B (target ≥3pp on at least 1 model)
- [ ] Multi-judge variance test — pending (run 5 ensemble runs, compute std)
- [ ] Champion claim verified at n=100 — Sprint 40 task (foundation done, runs to follow)

---

## Sprint 38 — Architecture (Week 5-6)

**Goal:** Move from one Eir agent to a multi-agent system that can route by
specialty and reason in explicit phases.

### Backlog

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| B-27 | Specialty router agent | XL | backend | `RouterAgent` decides Cardio / Sleep / Pediatrics / Generic → fans out to specialist Eir clones with specialty-specific prompts + KBs. New table `agent_specialty`, new endpoint `/agents/:id/route`. |
| B-28 | Reasoning loop: think → search → critique → answer | L | backend | New chat path that runs multiple LLM steps. Needs cancellation, telemetry, budget guard. |
| B-29 | Persona-specific KB partitioning | M | data | Tag PrimeKG / clinical-wisdom / pubmed-abstracts chunks with `specialty` filter. Specialist agents query their slice only. |
| B-30 | Eval framework: per-specialty HBp% tracking | M | backend | `eval_summaries.specialty_breakdown JSON` → router-vs-monolith comparison. |

### Definition of Done — Sprint 38
- [x] **B-27 PoC deployed (2026-05-05)** — `/agents/route` endpoint, 5 specialists
      + router for asgard_medical, 5/5 routing test accuracy
- [ ] Router beats monolithic Eir by ≥5pp on average HBp% — pending head-to-head eval (B-48/B-49)
- [ ] No specialty regresses by >3pp under router — pending eval (B-49)
- [ ] Expand 5 specialties → 28 (full HealthBench taxonomy) — B-52
- [ ] Per-tenant onboarding wizard (atomic specialty bundle) — B-53
- [ ] B-28 reasoning loop, B-29 KB partitioning, B-30 per-specialty HBp tracking — pending

---

## Sprint 38 follow-up — Router Validation + Productization (Week 7-8)

**Trigger:** Sprint 38 PoC LIVE (2026-05-05). 5/5 routing accuracy on hand-picked
questions, but no numerical HBp lift yet measured. Cross-benchmark evidence
suggests routing should help (gemma stronger on RAG; flash-lite on reasoning),
but A/B with judge scoring is needed before expanding to 28 specialties.

### Backlog

| ID | Title | Size | Priority | Notes |
|---|---|---|---|---|
| **B-48** | `use_specialty_router` flag in `EvaluatorParams` + runner.rs | M | P0 | Per-question route → call /agents/route → use returned agent_id for that item only |
| **B-49** | Router-vs-Monolithic A/B eval on hb-pro n=20 (locked items) | S | P0 | Two runs: monolithic Eir vs use_specialty_router=true. Compare HBp%, latency, cost. **Gates B-52 expansion.** |
| **B-50** | Decide cardio model: gemma-26b vs flash-lite head-to-head on cardio-only items | S | P1 | A/B revealed gemma cardio answer is more precise but +27s latency. Test on cardio-tagged items only — if gemma wins by ≥3pp justify the latency cost; else switch to flash-lite for uniform speed |
| **B-51** | PubMedQA underperformance investigation (51-59% < trivial baseline ~70%) | M | P1 | Hypothesis: Eir over-reasons binary y/n → judge interprets long reasoning as deviation. Try: (a) PubMedQA-specific system prompt instructing terse y/n answer, (b) custom MCQ-style judge, (c) different temperature |
| **B-52** | Expand 5 → 28 specialties (full HealthBench taxonomy) | M | P1 | **Gated on B-49 positive result.** Pure SQL clone using Sprint 38 template — bulk INSERT + JSON_ARRAY routes update. Specialty list: HealthBench Pro 28-specialty taxonomy |
| **B-53** | Tenant onboarding wizard (atomic 1-click specialty bundle) | M | P1 | New endpoint `POST /admin/onboard-tenant` — creates tenant row + N specialty agent rows from canonical templates in 1 transaction. UI: form picks tenant slug + checkbox specialty list |
| **B-54** | UI: `/agents/route` live demo page | M | P2 | Single text input + "Try routing" button → shows selected specialist card with reasoning + sends question to specialist's chat. Marketing-grade demo |
| **B-55** | Per-specialty HBp% tracking + UI breakdown (was B-30) | M | P2 | `eval_summary.specialty_breakdown JSON` from item metadata. UI: chart per specialty per run. Reveals which specialties Eir is weak on (Sprint 39 LoRA targets) |

### Definition of Done — Sprint 38 follow-up
- [ ] **B-48 + B-49 done first** — concrete data on whether router helps. If yes,
      proceed; if no (router neutral or hurts), reconfigure or abort 28-expansion
- [ ] **B-50 cardio decision** documented with run_ids
- [ ] **B-51 PubMedQA investigation** — at minimum identify root cause (over-reasoning,
      judge issue, or genuine model limitation)
- [ ] **B-52 expansion** to 28 specialties (only if B-49 positive)
- [ ] **B-53 onboarding wizard** demo-able for sales pitch
- [ ] **B-54 router UI** added to `/agents` nav

### Risk gates
- **B-49 negative result** (router doesn't lift scores): re-evaluate Sprint 38
  architecture; specialty system prompts may not be doing enough vs monolithic CoT.
  Possible pivot: invest in B-51 (PubMedQA fix) instead.
- **B-50 cardio decision**: if gemma cardio doesn't justify +27s latency on n=20
  validation, drop gemma from production routing. Local model still useful for
  HealthBench-Pro overall champion role, but not for time-sensitive triage.

---

## Sprint 40 — Multi-Benchmark Foundation (Week 8-9, parallel-able with 39)

**Goal:** Make Mimir's eval UI/DB benchmark-aware so we can run MedQA, MedMCQA,
PubMedQA, MedXpertQA, HealthBench (paper-original) alongside HealthBench-Pro
without confusing scores across rubrics. **Required before Sprint 41 paper-runs.**

### Why now
The UI currently shows one HBp% column with a global Rank/Champion — designed
when only HealthBench-Pro existed. Adding 5 more benchmarks would silently mix
rubrics (HBp% normalized Likert vs MCQ accuracy vs Y/N accuracy), breaking
ranking and comparisons.

### Backlog

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| B-36 | **Loader:** `scripts/load_medical_benchmarks_to_db.py` — register 5 datasets in `benchmark_datasets` + `benchmark_items` | M | backend | One row per dataset, normalized item schema |
| B-36a | UI: **Benchmark column** in runs table + tooltip | S | frontend | `eval_runs.benchmark_dataset_id` already on row; just add column |
| B-36b | UI: **Benchmark filter** dropdown (Status-style) | S | frontend | Persist in URL query param `?benchmark=hb-pro-asgard-001` |
| B-36c | UI: **Per-benchmark Rank/Champion** (🥇👑) | M | frontend | Group `rankByRunId` by benchmark, recompute champion per group |
| B-36d | UI: **Rubric-aware metric column** | L | backend+frontend | Each benchmark declares its scoring fn (HBp / Acc% / Y-N% / etc); UI swaps the "score" column header per active filter |
| B-36e | UI: **Cross-benchmark compare guard** | S | frontend | Warn before comparing 2 runs from different benchmarks |
| B-36f | UI: **`/benchmarks` registry page** | M | frontend | List datasets · schema · sample item · per-tenant champion |
| B-36g | UI: **Group/section by benchmark** toggle | S | frontend | "Group by benchmark" radio in toolbar |
| B-36h | DB: `benchmark_datasets.scoring_fn` column + migration | S | backend | Enum: `healthbench_likert`, `mcq_accuracy`, `binary_yes_no`, `paper_rubric_pct` |

### Definition of Done — Sprint 40
- [ ] All 5 medical benchmarks loaded as `benchmark_datasets` rows
- [ ] Eval UI shows benchmark for every run, filter by benchmark, per-benchmark
      rank, no cross-rubric ranking confusion
- [ ] At least 1 quick benchmark run on each (n=20-50) to populate UI

---

## Sprint 41 — Paper-Comparable HealthBench Run (Week 10)

**Goal:** Run the original HealthBench (5,000 conversations) with paper's grader
(gpt-4.1) so we can put numbers in marketing/papers comparable to the OpenAI baseline
(o3=60%, GPT-4o=32%).

### Backlog

| ID | Title | Size |
|---|---|---|
| B-43 | Adapt `healthbench_eval.py` to call Eir + Heimdall instead of OpenAI direct | M |
| B-44 | Implement gpt-4.1 grader path (cost: ~$50-100/full-run) | S |
| B-45 | Run paper-comparable scoreboard: gemma-4-26b + top 3 cloud Gemini × HealthBench main + Hard | M |
| B-46 | Publish numbers in baseline doc with explicit cross-ref to paper figures | S |
| B-47 | Submit to HF Open Medical-LLM Leaderboard (optional, if numbers are competitive) | M |

### Definition of Done — Sprint 41
- [ ] Paper-comparable scoreboard published
- [ ] Eir HBp% / paper-HB% correlation documented (so we know how to translate)
- [ ] Decision: should marketing use HBp-Pro or paper-HB or both?

---

## Sprint 39 — ML Pipeline (Week 7-9)

**Goal:** Persistent, model-level improvement via LoRA fine-tune on Mimir's
medical corpus + HealthBench-style training data. Biggest investment, biggest
payoff.

### Backlog

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| B-31 | Build training set: extract Q-A pairs from PMC + clinical-wisdom | L | data | Use Gemini 2.5 Pro to synthesize Q-A from passages. Manual review of 200 pairs minimum. |
| B-32 | MLX LoRA training pipeline | XL | ML eng | `mlx_lm.lora` script integration. Start with gemma-4-26b base. Track checkpoints in `data_sources` (new type `lora_adapter`). |
| B-33 | Adapter merge + Heimdall registry | L | infra | Merged model becomes a new entry in Heimdall's model list. Keep base + merged as separate options. |
| B-34 | Training metrics dashboard | M | UI | Loss curves, eval HBp% over training, comparison vs base. New page `/training/runs/:id`. |
| B-35 | A/B promotion workflow | M | backend | Promote LoRA-merged model to champion if it beats current with ≥5pp + same n. Reuse existing `promote_run` endpoint logic. |

### Definition of Done — Sprint 39
- [ ] LoRA-tuned gemma-4-26b ≥ 60% HBp on locked n=100
- [ ] Training pipeline documented + reproducible (one shell script)
- [ ] Champion promotion workflow gated on HealthBench delta + safety floor (no
      regression on `safe` dimension)

---

## Sprint 42 — Deep Research / Multi-hop Browsing (proposal)

**Trigger:** MedBrowseComp paper (arXiv:2505.14963) reveals frontier models score
<5% on 4-hop medical retrieval — same task Eir's RAG architecture is designed for.
Could be Asgard's biggest differentiator if we pursue it.

### Backlog

| ID | Title | Size | Priority | Notes |
|---|---|---|---|---|
| **B-56** | Download + integrate MedBrowseComp dataset (Phase 1) | S | P1 | HF `AIM-Harvard/MedBrowseComp` Apache 2.0 · loader extends existing infrastructure |
| **B-57** | 1-hop subset benchmark run (Eir + champions) | S | P1 | Phase 1 — uses existing static RAG, ~120 questions. Baseline before browsing tools |
| **B-58** | Add web-browsing tools to Eir | L | P2 | `pubmed_search()` (have!), `clinical_trials_search()`, `fda_drug_search()`, `web_fetch(url)`. Phase 2 — unlocks multi-hop |
| **B-59** | MedBrowseComp full eval after browsing tools | M | P2 | Phase 2 validation — target ≥10% on 4-hop (frontier <5%) |
| **B-60** | Submit Eir to MedBrowseComp leaderboard | S | P3 | First open-source entry signal |

### Why this matters

**MedBrowseComp tests Eir's exact value-prop:**
```
question → multi-source hop → multi-source hop → … → synthesis
```

Frontier scores reveal it's an open research problem:
- 1-hop: 76% (Gemini 2.5 Pro)
- 4-hop: **5.1%** (best!)
- 5-hop: ~0%

If Eir hits **even 10-15% on 4-hop**, that's a publishable result — open-source RAG agent matching/beating frontier on multi-hop medical retrieval.

### Definition of Done — Sprint 42
- [ ] MedBrowseComp 1-hop subset Eir ≥40% (frontier 76% — Eir at half is acceptable for open-source RAG)
- [ ] Eir on full 605-question set (after browsing tools)
- [ ] Multi-hop performance report published

---

## 🚧 Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Self-consistency 5× cost makes prod inference too slow | Medium | Medium | Production uses N=1; benchmark uses N=5. Document as eval-only. |
| LoRA overfits to HealthBench, regresses on real chats | High | High | Hold out 20% of HealthBench items as test set; require zero regression on chat conversations sample. |
| Specialty router misclassifies → wrong specialist worse than monolith | Medium | High | Fall-through to Eir generic on low-confidence routes. Log routes for offline review. |
| Mid-eval rogue swap returns | Low | High | Sprint 36 B-21 gates that path. Add tournament-mode lock that rejects swaps not from the eval runner. |
| Q5/Q6 quant breaks Heimdall MLX subprocess | Low | Low | Test on a sandbox model first. Q4 fallback in plist. |

---

## 📦 Dependencies & Cross-Cutting

- **Heimdall** — needs rerank model registration + larger model variants pulled
- **Vault** — already manages NCBI_API_KEY (Wave 4B); add training-data API keys here
- **Forseti** — track sprint runs as test pushes (existing pattern from earlier work)
- **HealthBench-Pro dataset** — source `hb-pro-asgard-001`. Items lock should
  be branched (`hb-pro-asgard-002`) when expanding to n=100

---

## 🎯 Recommended starting move (this week)

**Sprint 36 B-16** — wire the existing `cross_encoder_rerank` into Eir's chat
path. Code is already written; estimate 1 day. Expected immediate +3-5pp on Acc
and Rel for free. Verify on locked n=20, decide if it's enough signal to
greenlight Sprint 36 vs jump to Sprint 37 self-consistency for bigger lift.
