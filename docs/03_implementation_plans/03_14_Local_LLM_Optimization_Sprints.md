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

| Metric | Current | Sprint 43 target | Sprint 39 target |
|---|---|---|---|
| **Local champion HBp%** | 47.8% (gemma-4-26b) | **≥50%** (medgemma-27b challenge) | **≥60%** (LoRA) |
| **Sample size for champion claim** | n=20 | n=20 (fast-turn) | n≥100 |
| **Safety floor** | 0/20 unsafe (champion) | 0/20 unsafe (B-61 gate) | 0/20 unsafe |
| **Cost per HBp pp** (cloud-equivalent) | $0.000048/pp (3.1-flash-lite) | $0/pp (local) | $0/pp (local) |
| **Failure modes encountered** | launchd race ✅fixed, rogue swap ✅fixed, **community fine-tune safety regression (Round 5, 2026-05-05)** | safety screen blocks regressions | 0 known races |

---

## 🗓️ Sprint Themes

| Sprint | Theme | Headline outcome | Status |
|---|---|---|---|
| **36** 🟢 | Quick Wins — wire what's already there | +6-7pp validated (CoT + tune + per-model rerank) | ✅ done |
| **37** 🟡 | Score Multipliers — self-consistency, multi-judge, query expansion | Code deployed, awaits n=20+ validation | 🟡 partial |
| **38** 🟠 | Architecture — specialty router | **PoC LIVE 5/5 routing accuracy** | 🟢 PoC done |
| **38f** 🟠 | Sprint 38 follow-up — Router validation + 28-specialty expand | Gated on B-49 A/B result | 📋 next up |
| **39** 🟡 | ML Pipeline — Mimir Curator + LoRA fine-tune | 3 iterations complete (Phase 2 / 2b / 2c). Phase 2c locked-20 53.8% (+6pp ✅) but broader-100 38.7% (+1.1pp ❌) → single-anchor pass, **dual-anchor gate failed → champion holds.** Capacity helped locked-20; corpus is the broader-100 bottleneck. Sprint 39 closed; 39d direction locked from Sprint 47 B-47e A/B (RAG = +18.7pp). Total $3.50. | ✅ closed 2026-05-07 |
| **39d** 🔵 | RAG Enhancement — locked from B-47e A/B verdict (RAG +18.7pp on locked-20) | Sprint 39d direction: clinician gold (B-47g) → MedCPT re-embed → semantic re-chunk. Skip 10K Gemini synth retrain (saved $5-10). Phase 2c locked-20 53.8% sits near RAG-on ceiling 55.3%. | 📋 in flight |
| **40** 🟣 | Multi-Benchmark Foundation — benchmark-aware UI/DB | Unblocks Sprint 41 | ✅ done |
| **40f** 🟣 | Sprint 40 follow-up — n=100 scale + native MCQ scoring | Stable rank, paper-comparable Acc% | 📋 future |
| **41** 🔴 | Paper-Comparable HealthBench Run | Marketing-grade numbers vs arXiv:2505.08775 | 📋 future |
| **43** 🟡 | Local Model Alternatives — MedGemma 27B + Gemma-4 31B challenge | gemma-4-31b +3.5pp lift but 1 unsafe → blocked. Champion holds. n=100 retest queued. | ✅ closed 2026-05-06 |
| **42** 🟠 | Deep Research / Multi-hop Browsing (Hermodr) | MedBrowseComp 4-hop ≥10% (frontier <5%) | 📋 proposal |
| **44** ⚪ | Medical Paper Writing Skill — fork Master-cai/Research-Paper-Writing-Skills + medical overrides | Eir/team can draft publishable medical papers (CONSORT/STARD/TRIPOD-aware) via `~/.claude/skills/` | 📋 future (post-Sprint-42 outcomes) |
| **45** 🔵 | Mimir Batch API Service — first-class Gemini Batch support | Refactor phase1b script → reusable infra for re-judge / online-judge / future LoRA rounds | 📋 post-Sprint-39 (~2 weeks) |
| **46** 🟢 | Mimir Assistant Pipeline Operator — chat-driven LoRA pipeline + hyperparam advisor | Tool-calling pattern (no a2ui); 12-tool schema; glossary tooltips; auto-hyperparam suggestions | 📋 proposal (~3-4 weeks) |
| **47** 🟦 | Mimir RAG Eval — Rust-native RAGAS for medical RAG (bottleneck attribution) | 4 RAGAS metrics + 2 retrieval metrics + counterfactual ablation; diagnose LLM vs RAG vs both | 📋 proposal (~2 weeks) |
| **48** 🇹🇭 | Thai Clinical Coding Foundation — ICD-10 + ICD-10-TM + DRG | First Thai-native differentiator; Hermodr-resident; bilingual semantic search; FHIR Condition.code wiring | 📋 proposal (~3-4 weeks) |
| **49** 🟢 | MedOpenClaw Skill Integration — Phase 1 (5 priority + adapter template) | Port pubmed-search · DDI · clinical-trial-matching · differential-diagnosis · CPIC-pharmacogenomics; ToolRAG scaffold for 869-skill discovery | 📋 proposal (~3 weeks) |
| **50** 👁️ | **Syn S1 — OCR Foundation (advanced from Q3)** · 4-tier hybrid (chandra + PaddleOCR + Gemini Flash + Gemini Pro) | Norse "goddess of vigilance" service. Multi-component: Heimdall OCR sidecar (port 8084-8085) + Bifrost route + Mimir audit + Eir agent allowlist. Apache 2.0 local primary + opt-in cloud premium. Mega-Care intake synergy. | 📋 proposal (~4 weeks · slot 2026-05-08 → 2026-06-05) |
| **50b** 🌑 | **Skuggi — PII Guardrail (Pre-LLM Blind)** · runs parallel with Sprint 50 | Heimdall middleware that masks PII (face/Thai-ID/MRN/names) BEFORE any cloud LLM call. Image: OpenCV YuNet + PaddleOCR + Thai regex (zero new lib). Text: Rust regex (Tier 1) + PyThaiNLP (Tier 2). Mode default `mask-and-send`. Irreversible v0. | 📋 proposal (~2 weeks · slot 2026-05-15 → 2026-05-29) |

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
| B-19 | Add 4 specialty tools | L | backend | `drug_interaction_check` (DrugBank), `icd10_code_lookup`, `dosage_calculator`, `clinical_calculator` (CHADS2/MELD/Wells) — **superseded by Sprint 42 B-58e (Hermodr)** |
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

### Definition of Done — Sprint 37 (closed 2026-05-05)

**Implementation:** all 3 P0 features shipped + deployed.
- [x] **B-22 self-consistency**: deployed. `samples_per_item` 1-5, `sampling_temperature` override via `X-Sampling-Temperature` header.
- [x] **B-23 query expansion**: deployed. `query_expansion_n` ≥2 triggers Gemini-Flash paraphrase + " || " concatenation for retrieval.
- [x] **B-24 multi-judge ensemble**: deployed. `judge_models: Vec<String>`, averages normalized dimensions; single-judge back-compat preserved.
- [x] **B-51b CoT-off binary prompt** (Sprint 38f borrow): deployed. Per-benchmark prompt selection on `binary_yes_no` tasks.

**Validation (closure runs n=10, 2026-05-05):**

| Test | Run | Result | Δ vs baseline | Verdict |
|---|---|---|---|---|
| B-22 SC@3 + temp=0.7 (flash-lite, hb-pro-asgard-001 n=10) | `a1fe4ff9` | **43.1% HBp** (acc 2.70, comp 2.10, rel 3.10, safety **0.50**) | −2.2pp vs 45.3% recent baseline (e7120545); −5.3pp vs post-Sprint 36 champion (48.4%) | ❌ no lift; safety dropped to 0.50 (vs ~0.85 single-shot) |
| B-23 query expansion N=3 (gemma, PubMedQA n=10) | `c5d1048d` | **60% binary** (6/10 correct) | −5pp vs gemma 65% Sprint 38f baseline | ❌ no lift |
| B-51b CoT-off binary prompt (gemma, PubMedQA n=10) | `c9a68e69` | **60% binary** (6/10 correct, **identical answers** to B-23) | −5pp vs gemma 65% Sprint 38f baseline | ❌ no lift |

- [x] Self-consistency lift verified — **null result.** SC@3 with temp=0.7 didn't beat single-shot on flash-lite n=10. Variance across recent flash-lite runs is large (30-48% range), so 43.1% may be sample noise — but no positive signal either, and safety regressed.
- [x] Query expansion + CoT-off — **null result on PubMedQA.** Both gemma runs returned identical answers (deterministic at temp=0.3), 4/10 wrong items all show **yes-bias** (gemma over-predicts "yes" when correct answer is "no"/"maybe"). Root cause is model output distribution, not retrieval quality or CoT structure.
- [x] Multi-judge variance test — deferred to Sprint 41 (paper-comparable run, where multi-judge cost makes more sense at scale)
- [x] Champion claim verified at n=100 — Sprint 40f task

### Sprint 37 lessons learned

1. **Prompt-engineering & retrieval tricks have diminishing returns at this score band.** Three independent interventions (SC, QE, CoT-off) each predicted +3-15pp from literature; all returned ≤0pp on our benchmarks at n=10. The bottleneck isn't retrieval recall or reasoning frame — it's **model behavior** (gemma yes-bias on binary, flash-lite safety regression under sampling).
2. **Yes-bias on PubMedQA is a model-distribution issue.** Same wrong items in both B-23 and B-51b → not stochastic. Either (a) gemma was trained on yes-skewed medical Q&A, or (b) retrieved context biases toward affirmative findings (publication bias in PubMed). Fix candidates: (i) try MedGemma (Sprint 43) which was trained on more balanced clinical labels, (ii) add forced yes/no/maybe template, (iii) calibrated logit bias.
3. **Self-consistency + safety tradeoff.** SC@3 with temp=0.7 dropped safety 0.85→0.50 on flash-lite. Higher temp explores more diverse answers, some of which trip safety rubric. **Lesson: SC needs per-dimension safety floor — abort if any sample scores unsafe.**
4. **Confirms Sprint 43 priority.** Trying a different *model* (MedGemma 27B) is a higher-EV bet than more prompt tweaks. Sprint 43 is the right next move.

### Production deploy decisions (post-Sprint 37)

- ❌ **Do NOT enable `samples_per_item > 1` as default** — null lift, safety regression
- ❌ **Do NOT set `QUERY_EXPANSION_N=3` in prod env** — null lift, adds ~$0.0027/request cost
- ⚠️ **B-51b CoT-off prompt for binary tasks** — keep as opt-in via `prompt_style` flag; it's harmless but no lift on PubMedQA. Re-test on MedQA / MedMCQA where multi-choice format may benefit
- ✅ Keep all 3 features in code (env-toggle off) for future re-test on different models

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

## Sprint 39 — ML Pipeline (Week 7-14, ~7-8 weeks)

**Goal:** Persistent, model-level improvement via LoRA fine-tune on Mimir's
medical corpus + HealthBench-style training data. Biggest investment, biggest
payoff.

**Scope expanded 2026-05-06** to include **Phase 0: Mimir Curator** — a labeling
infrastructure built into Mimir Dashboard (not a Label Studio deploy). Reasoning:
no Rust-native LS alternative exists in 2026; full LS clone would be 5-7 weeks;
but a *minimal-scope* "Training Data Review" page in Mimir Dashboard covers the
~10-15% of LS features we actually need at ~1.5-2 weeks effort. See
[`Asgard/docs/architecture/ADR-001-Training-Data-Curator-Build-vs-Buy.md`](../../../Asgard/docs/architecture/ADR-001-Training-Data-Curator-Build-vs-Buy.md)
for the build-vs-buy decision record.

### Phase 0 — Mimir Curator (prep, Week 7-8)

Gating phase: blocks Phase 1 (corpus build). Other Sprints (42 Hermodr,
Sprint 43 follow-up) can run in parallel since Curator is contained to Mimir.

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| **B-30a** | DB migration `training_corpus_items` table | S (1d) | backend | ✅ applied 2026-05-06 (`migrations/sprint39_curator_and_lora.sql`). Cols: id, dataset_id, question, ai_answer, expected_answer, accuracy/comp/rel/safety scores, improved_answer, specialty, **tags (LONGTEXT JSON array — Sprint 39 multi-tag, added 2026-05-06)**, status (PENDING/APPROVED/REJECTED/FLAGGED), reviewer_id, reviewed_at, **tenant_id (nullable: NULL = shared corpus, non-NULL = tenant-specific)**, created_at. Indexed by (dataset_id, status, reviewer_id, tenant_id). |
| **B-30b** | Mimir backend endpoints `/api/v1/training/*` (Axum) | M (~1d actual) | backend | ✅ shipped 2026-05-06 (`routes/training.rs`, ~750 LOC). Curator: POST datasets, POST /:id/items (bulk JSONL), GET /:id/queue?reviewer=me&specialty=X**&tag=Y** (next pair), POST /:id/items/:itemId/review (submit rating + multi-tag), GET /:id/export.jsonl (streams `{prompt,completion,metadata{specialty,tags}}` lines). LoRA: POST /runs, PATCH /runs/:id, POST /runs/:id/log (loss tick). Reuse `tenant_auth_middleware`. **Tenant scoping rule:** dataset with `tenant_id=NULL` is visible to all tenants (shared); `tenant_id=<x>` only to that tenant. **Multi-tag filter** uses `JSON_CONTAINS(tags, JSON_QUOTE(?))` — see migration `sprint39_multi_tag.sql`. |
| **B-30c** | Mimir Dashboard page `/training` + `/training/[id]` (Next.js TS) | M (~2 days actual) | UI | ✅ shipped 2026-05-06. Reused Mimir Dashboard components from `/evaluations`. Keyboard shortcuts: 1-5 accuracy, ⇧+1-5 completeness, ⌥+1-5 relevance, S toggle safety, Enter approve+next, R reject, F flag, Esc clear, ? help. Side-by-side question/AI/expected/citations. Auto-load next pair. Sticky action bar. Progress indicator. **Multi-tag widget added 2026-05-06** (chip UI with Backspace-to-remove, comma/Enter to add, autocomplete from `COMMON_TAGS`). **Menu placement: AI group** (with RAG Playground, Agent Studio) — moved from Analytics 2026-05-06 since Curator is authoring not viewing. Will split into dedicated "Training" group when Phase 2 adds runs/adapters/promotion pages. |
| **B-30d** | JSONL export endpoint with streaming | S (1d) | backend | Filter `status='APPROVED'`, output `{prompt, completion, metadata}` JSONL one-per-line. Include reviewer + specialty + ratings in metadata for traceability. |
| **B-30e** | Dogfood: review 50 seed pairs to validate workflow | S (1d) | medical lead | If reviewer feedback flags missing features, scope-creep stop point — defer to Sprint 50+ Curator v2. |

**Phase 0 Definition of Done:**
- [ ] Reviewer can log in via Yggdrasil JWT (no separate auth)
- [ ] Bulk import 100+ items succeeds in <5s
- [ ] One-pair review cycle takes <30s reviewer-effort (excluding actual reading time)
- [ ] Export approved items as valid JSONL parsable by `mlx_lm.lora`
- [ ] 50 dogfood pairs reviewed by ≥1 medical lead, no blocking UX gaps

### Phase 1 — Corpus Build — Phased MVP (Week 9-10+)

> **Confirmed 2026-05-06:** start with MVP using local LLM synthesis + self-review,
> only escalate to Gemini paid synthesis + medical-team review once MVP pipeline
> validated. Each cost gate requires explicit approval.

#### Phase 1a — MVP corpus (no paid cost, ~3-5 days)

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| **B-31a-mvp** | Synthesize **100-200 Q-A pairs using local gemma-4-26b** (or gemma-4-31b) — same base we'll fine-tune | M (1-2d) | data | $0 paid. Lower quality than Gemini but sufficient to validate end-to-end pipeline. Distribute across specialties (cardio/ENT/peds/gen-med ~25-50 each). |
| **B-31b-mvp** | **Self-review** all MVP pairs via Curator UI | M (1-2d) | user | Single reviewer (no IAA needed at MVP scale). Validate Curator UX in real use. Identify gaps in workflow before scaling. |
| **B-31c-mvp** | Quality gate: ≥70% MVP pairs approved (rejection rate <30%) | S | user | If <70%, regenerate with better synthesis prompt. If reject reasons cluster on 1-2 patterns, fix synthesis prompt before scaling. |
| **B-31d-mvp** | Run **MVP LoRA training** (Phase 2 with this small corpus) and **mini-eval** (locked n=20) | S | ML eng | Validate full pipeline. If MVP LoRA at least matches base (~47.8% on locked items), pipeline is sound — escalate to Phase 1b. If catastrophic forgetting, fix before scaling. |

**MVP gate decision** (after B-31d-mvp): if MVP run completes cleanly →
proceed to Phase 1b. If pipeline breaks → fix and retry MVP, do NOT escalate
paid Gemini synthesis until MVP works.

#### Phase 1b — Full corpus (paid, ⚠️ requires approval)

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| **B-31** | Synthesize 5,000-10,000 Q-A pairs (Gemini 2.5 Pro from PMC + clinical-wisdom) | L | data | **Cost ~$15-25 — requires user approval** at MVP gate. Reuses validated synthesis prompt from B-31a-mvp. |
| **B-31b** | Manual review pass: ≥1,000 pairs **by medical team** (when available) | L | medical leads + Curator | Distribute across specialties. ≥2 reviewers per pair on a 50-pair overlap subset for IAA Cohen's κ ≥0.6. |
| **B-31c** | Quality filter: reject pairs flagged as factually wrong / unsafe / non-medical | S | medical leads | Track rejection rate; if >40% rejected, refine synthesis prompt and regenerate. |

### Phase 2 — LoRA Training + MLOps Tracking (Week 11-12)

> **MLOps tooling decision (2026-05-06):** Researched MLflow, Aim, DVC, ClearML, ZenML as
> Python options; researched xvc, opsml, ModelFox, mlflow-rs as Rust options. **No
> production-grade Rust MLOps platform exists** (closest: opsml at 35 stars proprietary, xvc
> at 72 stars solo-maintainer). MLflow scope mismatch with our 5-20 runs/sprint volume.
> **Decision: extend Mimir** with `lora_training_runs` table — same pattern as ADR-001
> (Curator). ~5-6 days vs ~1 week MLflow setup + ongoing service. See ADR-002 below.
>
> **Laminar (Rust LLM observability platform) evaluated 2026-05-06 — scope mismatch.**
> Laminar covers production LLM tracing + online evaluation; does not cover training
> experiment tracking (no hyperparams, loss curves, lineage). Significant overlap with
> existing Mimir `eval_runs`/`eval_summary` would require migration. Service availability
> concern noted (lmnr.ai inaccessible 2026-05-06). Revisit at Sprint 50+ for production
> agent observability concerns (separate from training tracking).

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| **B-32a** | DB migration: `lora_training_runs` table + `ai_models` lineage extension | S (1d) | backend | Cols on `lora_training_runs`: `run_id`, `dataset_version_id` (FK to corpus snapshot), `base_model_id`, `hyperparams JSON`, `loss_curve JSON`, `status`, `adapter_path`, `started_at`, `finished_at`, **`tenant_id` (nullable: NULL = shared adapter, non-NULL = tenant-specific)**. Extend `ai_models` with `parent_model_id`, `lineage_metadata` JSON, **`tenant_id` nullable**. Multi-tenant adapter strategy (confirmed 2026-05-06): support BOTH shared adapters (e.g. `gemma-4-26b-eir-base-lora-v1`, tenant_id=NULL) AND tenant-specific overrides (e.g. `gemma-4-26b-eir-asgard-medical-lora-v1`, tenant_id=`asgard_medical`). Tenant adapters can chain via `parent_model_id` → shared adapter. |
| **B-32b** | MLX LoRA training wrapper + Mimir auto-log | M (2d) | ML eng | Python wrapper around `mlx_lm.lora` that POSTs hyperparams + loss tick to `/api/v1/training/runs/:id/log` every N steps. Records adapter checkpoint paths to RustFS (existing cluster S3 backend). |
| **B-32** | MLX LoRA training pipeline (now: thin orchestrator over B-32a/b) | M (3d) | ML eng | Hyperparams: rank=8, alpha=16, dropout=0.1, LR=1e-4, iterations=1000. Target modules: q/k/v/o_proj. Base: gemma-4-26b-a4b-it-4bit. Reproducible via `bin/lora_train.sh`. |
| **B-33** | Adapter merge + Heimdall registry | L | infra | `mlx_lm.fuse` produces merged model. Register as new `ai_models` row `gemma-4-26b-eir-lora-v1` with `parent_model_id=mlx-community/gemma-4-26b-a4b-it-4bit` and `lineage_metadata.training_run_id`. Keep base + merged as separate active options. |
| **B-34** | Training metrics dashboard `/training/runs/:id` | M (3-4d) | UI | Loss curves (train + eval), hyperparams table, eval HBp% per checkpoint, vs-base comparison, adapter lineage tree (parent corpus version → adapter → eval runs). Reuse Mimir Dashboard chart components from `/evaluations`. |

### Phase 3 — A/B Validation & Promotion (Week 13-14)

| ID | Title | Size | Owner | Notes |
|---|---|---|---|---|
| **B-35** | A/B promotion workflow | M | backend | Reuse existing `promote_run` endpoint. Promotion gate (revised 2026-05-06 after Sprint 43 follow-up calibration revealed sample bias in locked-20): **dual-anchor** — LoRA must beat champion ≥+5pp on BOTH (a) locked-20 items (≥**55%** vs champion 47.8%) AND (b) broader n=100 sample (≥**45%** vs champion 37.6% at n=100 + URL rule). Other gates: (1) unsafe count ≤ champion's rate at the same sample size, (2) latency p50 ≤ 1.2× champion, (3) pre-flight safety screen 20/20 ACCEPT, (4) OOD chat hold-out shows no observable regression. Dual-anchor prevents over-fitting to the curated locked-20 subset. **Adapter storage: RustFS in cluster** (confirmed 2026-05-06 — same S3 backend as eval data; in-cluster, scaling-friendly). |
| **B-35b** | Pre-promotion safety screen | S | infra | Run B-61 safety screen (20 unsafe prompts) on adapter — must ACCEPT before A/B eligibility. |
| **B-35c** | n=100 final eval (locked items) | S | infra | Cost ~$0.27/run. Required before any champion claim. |

### Definition of Done — Sprint 39
- [ ] **Phase 0 Curator** shipped + dogfooded (50+ pairs reviewed)
- [ ] **Phase 1 Corpus** ≥1,000 reviewed pairs with IAA Cohen's κ ≥0.6
- [ ] **Phase 2 Training** at least 1 LoRA adapter trained + merged + registered
- [ ] **Phase 3** LoRA-tuned gemma-4-26b ≥ **60% HBp on locked n=100** (North-Star target)
- [ ] Training pipeline documented + reproducible (one shell script + recorded hyperparams)
- [ ] Champion promotion workflow gated on HealthBench delta + safety floor (0 unsafe regression vs incumbent)
- [ ] **No regression** on out-of-distribution chat hold-out (50 random non-HBp conversations)
- [ ] Adapter weight + corpus snapshot exported to T7 Shield for reproducibility

### Cost budget — Sprint 39 (MVP-first, confirmed 2026-05-06)

| Phase | Item | Estimated cost | Approval |
|---|---|---|---|
| 0 | Curator dev (no API cost) | $0 | ✅ autonomous OK |
| **1a MVP** | **Synthesize 100-200 Q-A via local gemma** + self-review + mini-LoRA + n=20 eval | **$0** (local model + judge fee ~$0.054) | ✅ autonomous OK |
| 1b | Gemini 2.5 Pro corpus synthesis (5K Q-A, after MVP gate) | ~$15-25 | ⚠️ user approval gate at MVP completion |
| 2 | LoRA training (local MLX, M3 Max electric) | ~$0 | ✅ autonomous OK |
| 3 MVP eval | n=20 eval (gemini-2.5-flash judge) | ~$0.054 | ✅ autonomous OK |
| 3 final | n=100 eval × 2-3 candidates (judge cost) | ~$1-2 | ⚠️ user approval gate |
| **Total MVP path** | | **~$0.10** | ✅ |
| **Total full path** (if MVP gates pass) | | **~$20-30** | ⚠️ phased approvals |

**MVP-first rationale:** validate full pipeline (Curator → corpus → train →
eval) at near-zero cost before committing to paid Gemini synthesis. If MVP
breaks, fix before scaling. Budget user time = a few days, not weeks.

### Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Catastrophic forgetting (LoRA loses general medical capability) | High | High | 20% holdout from HBp; eval on out-of-distribution chat samples; abort if any regression |
| Overfitting to synthetic Gemini-generated training data | High | High | Mix with manually-reviewed real-chat samples; OOD validation |
| Safety regression (Round 5 lesson) | Medium | High | B-61 pre-flight gates every adapter |
| MLX 4-bit + LoRA edge cases | Medium | Medium | bf16 prototype first, then quantize |
| Curator dev creep beyond 2 weeks | Medium | Medium | Hard scope cap = "review form, not labeling platform"; defer features to Sprint 50+ Curator v2 |
| Reviewer bandwidth (medical lead time) | High | High | Front-load: 50 dogfood pairs to validate workflow before scaling to 1,000 |
| LoRA-tuned ≥60% target missed | Medium | Medium | If <55%, declare "retrieval is bottleneck" hypothesis confirmed → pivot to MedCPT embedding (Sprint 36 B-20) + agentic RAG before more LoRA |

---

## Sprint 39d — RAG Enhancement (locked from B-47e A/B verdict, 2026-05-07)

**Trigger:** Sprint 47 B-47e counterfactual A/B (rag_mode=on vs off) on
locked-20 with the production champion (`mlx-community/gemma-4-26b-a4b-it-4bit`,
judge `gemini-2.5-flash`) showed **RAG contributes +18.7pp** of HBp. Sprint 39
closure left the broader-100 bottleneck question open; this verdict resolves
it: **broader-100's 38.7% cap is RAG-side, not LLM-side**. Sprint 39d redirects
investment AWAY from corpus retrain TOWARD retrieval quality.

### A/B verdict (one-line)

```
RAG-on   55.3% HBp  (Acc 2.85, Comp 2.25, Rel 3.75, Safety 0.75, 0/20 unsafe)
RAG-off  36.6% HBp  (Acc 2.20, Comp 1.80, Rel 2.85, Safety 0.50, 1/20 unsafe)
Δ        +18.7pp    (Rel +0.90, Acc +0.65, Comp +0.45, Safety +0.25)
```

Run IDs: `2d63aa95` (on) · `8fe05299` (off). Cost: ~$0.04 total (flash judge × 40 rows).

### Backlog (priority order, post-verdict)

| ID | Title | Effort | Rationale |
|---|---|---|---|
| **B-39d-1** | Sprint 47 B-47g — clinician-curated `rag_benchmark_items` for locked-20 | ~1-2 d clinician | Unlocks pure retrieval metrics (Recall@k, MRR, NDCG@k) and B-47e.2 `gold_only` counterfactual mode. Necessary before any of B-39d-2/3 can be A/B-evaluated. |
| **B-39d-2** | Re-embed `medical_knowledge` collection with **MedCPT** (medical-domain embedder) | ~1 d (download model + ETL re-embed) | BGE-M3 multilingual is general; MedCPT trained on PubMed should improve Recall@k, especially Accuracy + Completeness dimensions where Δ was +0.65 / +0.45. |
| **B-39d-3** | Re-chunk `clinical_kb` with **semantic boundary detection** (not fixed-token) | ~2 d | Fixed-token chunks split mid-sentence; coherent semantic chunks score better on Faithfulness + Context Precision. |
| **B-39d-4** | (NEGATIVE) **Skip 10K Gemini synth retrain** | save ~$5-10 | Phase 2c locked-20 53.8% sits 1.5pp from RAG-on ceiling 55.3% — corpus expansion has near-zero ROI until retrieval lifts the ceiling. |

### Acceptance — Sprint 39d

- [ ] B-47g delivered: ≥50 locked-20 questions have `rag_benchmark_items` rows with relevant_chunk_ids labeled by clinician
- [ ] B-39d-2 MedCPT re-embed complete, Qdrant collection rebuilt with new vectors
- [ ] Counterfactual run (rag_mode=on with new collection) lifts HBp ≥ +3pp vs current 55.3%
- [ ] B-39d-3 re-chunk complete, ContextPrecision (Sprint 47 B-47b RAGAS metric) lifts ≥ +0.10
- [ ] Champion swap candidate emerges (≥58% HBp on locked-20, ≥45% on broader-100)

### Out of scope (deliberately, per A/B verdict)

- ❌ LoRA hyperparam re-tuning — Phase 2c rank=16 layers=24 already adequate
- ❌ Base model swap (MedGemma 27B / Gemma-4 31B) — defer to Sprint 50+
- ❌ 10K+ corpus expansion via Gemini synth — RAG ceiling, not corpus quality, is binding
- ❌ Multi-tenant rollout — `asgard_medical` only for v0

### Cost / value justification

| Path | Estimated lift | Cost | ROI |
|---|---|---|---|
| **B-39d (this)** | +3-5pp HBp | ~$0 (local infra + clinician time) | High |
| Alt: 10K Gemini synth + LoRA retrain | +1-2pp HBp (per A/B verdict, marginal) | ~$5-10 | Low (rejected) |
| Alt: MedGemma 27B base model swap | unknown, possibly -2pp safety | ~$0 + 1 wk dev | Risky (deferred) |

### Cross-references

- A/B verdict full data: `eval_runs.id IN (2d63aa95, 8fe05299)`
- Sprint 47 B-47e implementation: [`Mimir/ro-ai-bridge/src/routes/agents/chat.rs`](../../ro-ai-bridge/src/routes/agents/chat.rs) `X-RAG-Mode` header
- Sprint 47 B-47g (B-39d-1 prerequisite): pending dev — see Sprint 47 backlog
- Judge model policy: flash for iterations, pro for promotion gate decisions (see Sprint 50+ judge_policy ADR proposal)

---

## Sprint 43 — Local Model Alternatives (Week 11, fast-turn)

**Trigger:** Round 5 (2026-05-05) showed `Qwen-3-32B-Medical-Reasoning-4bit` fails
catastrophically (6.6% HBp, 4/20 unsafe — community fine-tune lost safety alignment).
Local champion stuck at `gemma-4-26b-a4b-it-4bit` @ 47.8% — below the 50% North-Star
floor. Need to challenge with Google-validated medical models before LoRA work
(Sprint 39) burns weeks of effort.

### Acceptance criteria (per challenger)

- ✅ Pre-flight safety screen passed (refuses 20/20 known-unsafe prompts)
- ✅ HBp% **≥ 50%** on `hb-pro-asgard-001` locked n=20
- ✅ 0/20 unsafe answers in eval
- ✅ Latency p50 ≤ 1.2× current champion (47.0s × 1.2 = 56s)

If ≥2 challengers pass, tiebreak by latency.

### Backlog (priority order)

| ID | Title | Size | Priority | Notes |
|---|---|---|---|---|
| **B-61** | Pre-flight safety screen — 20 known-unsafe prompts gate | S | P0 | New script `tools/safety_screen.py`. Categories: drug overdose, self-harm, contraindicated procedures, pediatric dosing errors, anaphylaxis-trigger administration. Reject any model that produces non-refusal on any item. **Must run before any HBp eval going forward.** Lesson from Round 5. |
| **B-62** | Pull `mlx-community/medgemma-27b-text-it-4bit` + register in Heimdall | S | P0 | Google official MedGemma 27B (text-only), Gemma-3 base, medical fine-tune with vendor red-team. ~15GB download. Add `ai_models` row with `rerank_recommended=false` (default off, test later). |
| **B-63** | **Round 6** — medgemma-27b-text vs locked items n=20 | S | P0 | Same dataset (`hb-pro-asgard-001`), same agent snapshot id 28, same judge `gemini-2.5-flash`. **Highest probability of dethroning champion.** |
| **B-64** | **Round 7** — `gemma-4-31b-it-4bit` vs locked items n=20 | S | P1 | Already pulled. Bigger Gemma-4 dense (31B vs MoE 26B-A4B 4B-active). Quick test — no download needed. Latency risk: dense 31B will be 2-3× slower than MoE 4B-active per token. |
| **B-65** | **Round 8** — `medgemma-1.5-4b-it-4bit` vs locked items n=20 | S | P2 | Only run if Round 6 wins. Proves whether the smaller MedGemma variant (newest v1.5) reaches comparable scores at 4× lower latency. |
| **B-66** | Champion promotion + baseline doc + memory update | S | P0 | If any challenger passes acceptance: update `04_03_HealthBench_Pro_Baseline_2026-05-04.md`, set new `is_champion=true` in `ai_models`, update memory `mimir_eir_baseline.md`. If none pass, document as Round 6/7/8 failures + next directions (LoRA = Sprint 39, multi-judge ensemble re-test = Sprint 37 follow-through). |

### Definition of Done — Sprint 43 (closed 2026-05-06, autonomous overnight run)

**Implementation:**
- [x] **B-61 safety screen** lives at `Mimir/scripts/safety_screen.py` — 20 prompt categories (drug overdose, self-harm, contraindicated procedures, pediatric dosing errors, anaphylaxis, concealment, prescription evasion, self-abortion, animal harm, vulnerable-population manipulation, DIY poison synthesis). Parser handles thinking-mode `message.reasoning` field; classifies via final-section-only refusal/comply markers + strong-refusal priority rule.
- [x] **B-62 medgemma-27b-text-it-4bit** pulled (~15GB) and registered in `ai_models` (sprint metadata + safety_validation note).
- [x] **B-62 gemma-4-31b-it-4bit** already pulled, registered.

**Validation runs (n=20 locked items, hb-pro-asgard-001):**

| Round | Model | Run | HBp% | Unsafe | Verdict |
|---|---|---|---|---|---|
| 6 (B-63) | medgemma-27b-text-it-4bit | `a91d806f` | 41.9% | 1/20 | ❌ −5.9pp vs champion |
| 7 (B-64) | gemma-4-31b-it-4bit | `4fff293e` | **51.3%** | 1/20 | 🟡 +3.5pp lift, BUT 1 unsafe |
| 8 (B-65) | medgemma-1.5-4b-it-4bit | — | — | — | ⏭️ orchestrator bug, see below |

- [x] Safety screens (B-61b): gemma-4-26b 20/20 ACCEPT (sanity), medgemma-27b 20/20 ACCEPT, gemma-4-31b 20/20 ACCEPT — parser calibrated.
- [x] **Champion HOLDS** at gemma-4-26b 47.8%. gemma-4-31b had clear HBp lift but 1 unsafe item disqualifies under acceptance criterion (0/20 unsafe required).
- [x] Unsafe-item analysis: low-severity (URL-confabulation, not harm-vector advice). Different category than Round 5's Qwen-Medical 4/20 dosing-advice unsafes.

### Sprint 43 lessons learned

1. **Local champion ceiling at n=20 is ~50%.** Three Gemma-family local models clustered at 41.9-51.3% — none cleanly broke through. Sprint 39 LoRA (next swing) targets ≥60%; closing this gap likely requires fine-tuning, not model swap.
2. **Safety screen worked as designed.** Caught zero false-rejects on champion (calibration). The two "safer" models (medgemma) passed effortlessly. The acceptance-during-eval is the remaining unsafe-detection gap — the 1/20 unsafe in Round 7 was caught by the judge during eval, not pre-flight.
3. **Pre-flight-screen vs eval-judge disagree.** A model can pass the 20-prompt explicit-harm screen but still get flagged unsafe by the judge during normal eval (e.g. URL confabulation). **Implication:** need a stage between pre-flight and full HBp — maybe 100 random non-harmful queries and check for any judge-flagged unsafe.
4. **Bigger Gemma-4 isn't better-by-a-lot.** Dense 31B (51.3%) only +3.5pp over MoE 26B-A4B (47.8%) — within sample noise at n=20. Most of the score band is shared. Either we hit the model-class ceiling on this benchmark, or our retrieval/RAG is the actual bottleneck.
5. **MedGemma 27B underperformed expectations.** Google's medical fine-tune (Gemma-3 base) at 41.9% was *below* the Gemma-4 family. Gemma-3 base is 1 generation older — that gap matters more than the medical fine-tune helps. Don't assume "domain fine-tune = better"; base-model generation is dominant.

### Production deploy decisions (post-Sprint 43)

- ❌ **Do not promote** medgemma-27b-text-it-4bit (worse than champion).
- ❌ **Do not auto-promote** gemma-4-31b — fails 0-unsafe acceptance criterion despite HBp lift.
- ✅ **Add URL-handling system-prompt rule** to default Eir agent: "If user provides a URL, refuse to interpret without fetched content; suggest user paste the relevant text." Test on gemma-4-26b first, deploy if no regression.
- 🟡 **n=100 re-test on gemma-4-31b** queued — if lift holds and 0/100 unsafe under the new URL rule, promote.
- ✅ **Round 8 (medgemma-1.5-4b)** queued — orchestrator bug skipped it; per plan it should run since Round 7 beat champion.
- ⏭️ **Sprint 39 LoRA still required** — model-swap ceiling at this score band confirmed empirically.

### Orchestrator bug (overnight run b00rp7roz)

The autonomous `sprint43_autorun.sh` had a `wait_for_run` polling bug that caused
it to time out on Round 7 with empty `status` reads (90-min timeout) even though
the eval had finished. Likely cause: `mariadb_q` mixing stderr (insecure-password
warning) into stdout via `2>&1`, occasionally pushing the data line past `tail -1`.

**Fix for next autonomous run:** change `mariadb_q` to suppress stderr
(`2>/dev/null`) and use `head -2 | tail -1` to defensively skip warning lines.
Logged as B-67 follow-up.

### Out of scope (deferred)

- ❌ MedGemma 27B **multimodal** (`medgemma-27b-it-4bit`, vision-capable) — Eir doesn't ingest images yet, multimodal weights bigger for no benefit. Pull when image triage UC arrives.
- ❌ E4B/E2B variants — too small for hard HBp items.
- ❌ Community uncensored/heretic Gemma-4 forks — Round 5 lesson: don't.

---

## Sprint 44 — Medical Paper Writing Skill (proposal, gated on Sprint 42 outcomes)

**Trigger:** Sprint 42 (MedBrowseComp 4-hop) ตั้งเป้า publishable result. ถ้า
Eir hit ≥10% on 4-hop จริง → เป็น papers ที่ควรเขียนส่ง (open-source RAG agent
matching frontier on multi-hop medical retrieval is a publishable signal).
HealthBench-Pro baseline + Asgard hybrid-tool architecture เป็น candidate
papers อื่นด้วย. ต้องการ skill ที่ช่วยเขียน paper ตามมาตรฐาน clinical reporting

**Source skill:** [Master-cai/Research-Paper-Writing-Skills](https://github.com/Master-cai/Research-Paper-Writing-Skills)
(MIT, ~1.95k stars, active 2026-04-23). Anthropic skill format — drop-in กับ
`~/.claude/skills/`. ML/CV/NLP-flavored ต้อง adapt for medical.

### Backlog

| ID | Title | Size | Priority | Notes |
|---|---|---|---|---|
| **B-67** | Fork base skill into `Asgard/skills/research-paper-writing-medical/` | S | P2 | Pull Master-cai's `SKILL.md` + `references/` verbatim. Preserve attribution + MIT license file. |
| **B-68** | Override 6 section files for medical conventions | M | P2 | Rewrite `abstract.md`, `introduction.md`, `method.md`, `experiments.md`, `related-work.md`, `conclusion.md` with PICO framing, clinical-evidence hierarchy, sample-size justification (power analysis), IRB/ethics blurbs. Keep base structural advice. |
| **B-69** | Add medical-specific sub-skills | M | P2 | New files: `clinical-trial-reporting.md` (CONSORT for RCTs, STARD for diagnostic accuracy, TRIPOD for prediction models), `pico-framing.md`, `medical-figures-tables.md` (CONSORT flow diagram, ROC curves, calibration plots, Kaplan-Meier), `safety-and-limitations.md` (adverse events, generalizability, FDA framing). |
| **B-70** | Validation pass — draft Asgard's own papers using the skill | S | P3 | Drink-our-own-champagne: write the HealthBench-Pro baseline paper + Sprint 42 MedBrowseComp paper using the skill. Iterate skill content based on what's missing. |
| **B-71** | Publish back upstream (optional) | S | P3 | If medical overrides are clean, PR to Master-cai's repo as a `medical/` variant — community contribution + maintains parity. |

### Acceptance criteria

- ✅ Skill installs cleanly via `~/.claude/skills/research-paper-writing-medical/`
- ✅ Discoverable by Claude Code skill picker (frontmatter + name)
- ✅ Base skill content preserved with clear attribution to Master-cai
- ✅ MIT license file copied; Asgard's modifications also MIT
- ✅ At least one Asgard paper drafted with the skill before declaring DoD

### Out of scope

- ❌ Auto-submission to journals (manual humans-in-loop only)
- ❌ Citation manager integration (use Zotero externally)
- ❌ Statistical analysis / figure generation (skill is writing-only; rely on R/Python externally)

### Dependencies / sequencing

- **Don't start** until Sprint 42 result is known. If MedBrowseComp ≤5% on 4-hop, drop Sprint 44 (no paper to write).
- **No blocker** to Sprint 43 (model alternatives) — different repo / different person.
- **Cost:** $0 — pure markdown, no APIs, no infra.

---

## Sprint 42 — Deep Research / Multi-hop Browsing (proposal)

**Trigger:** MedBrowseComp paper (arXiv:2505.14963) reveals frontier models score
<5% on 4-hop medical retrieval — same task Eir's RAG architecture is designed for.
Could be Asgard's biggest differentiator if we pursue it.

### Tool placement (decided 2026-05-05)

External-API and stateless tools live in **Hermodr** (Universal MCP Sidecar,
separate repo `Hermodr/`); stateful in-process tools (PrimeKG, Qdrant pools,
Neo4j, MariaDB-bound calls) stay in **Mimir**. Mimir's MCP server discovers
Hermodr tools at startup and exposes them in the same `tools/list` so Eir sees
one flat catalog. Rationale + decision matrix: see Asgard
`docs/roadmap/MultiAgent_Architecture_Plan.md` → "Hybrid Tool Placement".

### Backlog

| ID | Title | Size | Priority | Repo | Notes |
|---|---|---|---|---|---|
| **B-56** | Download + integrate MedBrowseComp dataset (Phase 1) | S | P1 | Mimir | HF `AIM-Harvard/MedBrowseComp` Apache 2.0 · loader extends existing benchmark infrastructure |
| **B-57** | 1-hop subset benchmark run (Eir + champions) | S | P1 | Mimir | Phase 1 — uses existing static RAG, ~120 questions. Baseline before browsing tools |
| **B-58a** | Hermodr `eir_medical` service skeleton — 10 tool defs | S | P1 | **Hermodr** | ✅ landed 2026-05-05 (`src/services/eir_medical.rs`) — pubmed×2, ct.gov×2, fda, icd10, rxnav×2, web_fetch, medcalc |
| **B-58b** | Hermodr deployments (one per upstream) | M | P2 | **Hermodr** | Helm chart per upstream: `hermodr-pubmed`, `hermodr-trials`, `hermodr-fda`, `hermodr-rxnav`, `hermodr-webfetch`, `hermodr-medcalc`. Each with own rate-limit budget. |
| **B-58c** | `web_fetch` proxy mode + `medcalc` internal handler | M | P2 | **Hermodr** | Implement `__hermodr_internal__/web_fetch` (URL passthrough + html→text) and `__hermodr_internal__/medcalc/{formula}` (pure-compute calculators). See `proxy.rs` TODOs. |
| **B-58d** | Mimir MCP → Hermodr discovery + dispatch | M | P2 | Mimir | Wire `mimir-core-ai/services/hermodr.rs` (✅ stub landed) into `mcp_server::list_tools()` and `dispatch_tool_call()`. On startup, fan out `tools/list` to every configured `HERMODR_*_URL`; cache ToolDefinitions for the request lifetime. Forward `tools/call` to the originating endpoint. |
| **B-58e** | Migrate Sprint 36 B-19 specialty tools → Hermodr | S | P2 | **Hermodr** | `drug_interaction_check`, `icd10_code_lookup`, `dosage_calculator`, `clinical_calculator` — all reachable via `eir_medical` service, no Mimir-side glue beyond B-58d. |
| **B-59** | MedBrowseComp full eval after browsing tools | M | P2 | Mimir | Phase 2 validation — target ≥10% on 4-hop (frontier <5%) |
| **B-60** | Submit Eir to MedBrowseComp leaderboard | S | P3 | Mimir | First open-source entry signal |

> **Decision rule for new tools (Sprint 42+):** if it touches an external API,
> CPU-only computation, or any state Mimir's pools don't already manage, build
> it in Hermodr from day one. Migration to Mimir later is a one-line
> `tools/call` redirect — the reverse is a refactor.

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

## Sprint 45 — Mimir Batch API Service (proposal, post-Sprint-39)

**Trigger:** Sprint 39 Phase 1b synthesis used a standalone Python script
(`phase1b_batch_synth.py`) that calls Gemini Batch API directly. This was
fast-shipping but doesn't scale architecturally — every future batch use case
re-implements upload/poll/parse/import. Mimir should expose batch generation
as a first-class capability so it's reusable for re-judging, online judge,
mass retrieval expansion, future LoRA dataset rounds, etc.

### Architectural decision (locked 2026-05-06)

Batch lives in **mimir-api** (Rust/Axum), not Heimdall. Heimdall is a
stateless LLM gateway; batch jobs are stateful (submit → poll → import) and
fit Mimir's existing pattern (`eval_runs`, `lora_training_runs`).

### Backlog

| ID | Title | Size | Notes |
|---|---|---|---|
| **B-30g** | DB migration `batch_jobs` table + `batch_results` rollup view | S (1d) | Cols: id, name, model, provider, status, gemini_input_file, gemini_batch_name, gemini_output_file, input_request_count, output_response_count, parsed_item_count, handler_type, handler_payload JSON, estimated_cost_usd, actual_cost_usd, submitted_at, completed_at, tenant_id (nullable), created_by, notes |
| **B-30h** | Backend `/api/v1/batch/*` endpoints (Axum) + Gemini Batch API client | M (3-4d) | POST /jobs (submit), GET /jobs (list), GET /jobs/:id (detail), POST /jobs/:id/cancel, POST /jobs/:id/retry. Tenant-scoped (NULL=shared). |
| **B-30i** | Background tokio worker — poll RUNNING batches every 5min, dispatch handlers when COMPLETED | M (2-3d) | Reuse `cron_worker` pattern from existing Mimir codebase. |
| **B-30j** | Pluggable handler trait — `CorpusImportHandler`, `EvalRejudgeHandler`, `SynthEvalHandler`, `RetrievalExpandHandler` | M (per-handler 1-2d) | Trait definition + first impl (CorpusImport replaces phase1b_batch_synth.py). |
| **B-30k** | Mimir Dashboard `/batches` page (status, cost, history, retry UI) | M (2-3d) | Reuse eval table components; show cost actual vs estimate. |

**Total ~2 weeks** for full B-30g→k (P1 of batch foundation).

### Acceptance criteria — Sprint 45

- [ ] `phase1b_batch_synth.py` migrated to call `POST /api/v1/batch/jobs` (script reduced to thin CLI)
- [ ] CorpusImport handler verified: submit batch → poll → parse → import to Curator (no human steps in between)
- [ ] EvalRejudgeHandler ready for Sprint 41 paper-comparable run
- [ ] Dashboard shows live batch status + cost
- [ ] Multi-tenant: shared batches (`tenant_id=NULL`) visible to all; tenant-scoped isolated

### Out of scope

- ❌ Cross-provider batching (OpenAI Batch, Anthropic Batch) — start with Gemini only; trait makes adding others trivial later
- ❌ Streaming / partial results — Gemini Batch is whole-file output

### Cost / value justification

| Use case | Volume | Saving (50% batch discount) |
|---|---|---|
| Phase 1b corpus (already done) | 1K calls | $3 saved |
| Sprint 41 paper-comparable (gpt-4.1 grader, 5K conv) | 5K calls | ~$50 saved per run |
| Sprint 50+ online judge (1% prod sampling, continuous) | 100s/day | perpetual ~50% ongoing saving |
| Future LoRA dataset rounds (retrain) | 5K-20K per round | $5-20 per round |
| **Total saving over 12 months** | — | **~$100-200** ROI on 2-week dev |

---

## Sprint 46 — Mimir Assistant Pipeline Operator (proposal, ~3-4 weeks)

**Trigger:** Sprint 39 closure produced a working but operator-heavy pipeline:
Curator UI → CLI scripts (`lora_train_mvp.py`, `lora_eval_mvp.py`) → manual hot-swap →
manual decision. The 7-step pipeline is currently invisible to non-engineers and
hyperparameter choices ride on tribal knowledge from the Sprint 39 Learning Journal
([04_06](../04_evaluation_and_testing/04_06_LoRA_Sprint39_Learning_Journal.md)).
This sprint makes the pipeline **chat-operable**: trigger runs, monitor progress,
ask for hyperparameter recommendations, and read plain-language explanations
inside Mimir Assistant.

### Architecture decision (locked 2026-05-06)

**Pattern:** Tool-calling LLM (Mimir Assistant orchestrates) — **NOT** Google a2ui,
**NOT** generative UI, **NOT** MCP-via-Hermodr.

**Rationale:**
- a2ui (https://github.com/google/a2ui) is an experimental UI-spec framework
  designed for agent-driven UI generation. Mimir Assistant already has a chat
  surface; adding a2ui means re-rendering pipeline screens from agent output,
  which costs more than it saves for a single-consumer internal tool.
- Tool-calling fits Asgard idioms (already used by Eir for medical tool calls
  via Hermodr). LLM picks tools, Mimir API does the work, response renders as
  text + small inline UI cards in Mimir Assistant (existing component).
- Hermodr/MCP rejected for this surface — see "Tool placement" below.

**Phase 2 (deferred):** If multi-tenant production usage shows demand for
generative UI (e.g., custom run-comparison views per tenant), revisit a2ui or
Vercel AI SDK UI Stream Protocol then.

### Tool placement

Mimir-native (no MCP/Hermodr) for **all 12 tools** in this sprint. Reasoning:
- Every tool reads/writes Mimir DB tables (`lora_training_runs`, `eval_runs`,
  `training_corpus_items`, `ai_models`)
- Single consumer = Mimir Assistant chat (no cross-service reuse)
- Hermodr's value-add is external API access; these are all internal Mimir ops
- MCP hop would add ~50-200ms latency per tool call for zero benefit

**Future Sprint 47+ exception:** if Eir agent needs to trigger autonomous
retraining (e.g., scheduled drift response), expose ONE thin Hermodr MCP
wrapper `mimir.trigger_lora_run` that proxies to `POST /api/v1/training/runs`.
Out of scope for Sprint 46.

### Tool schema (draft v0.1)

12 tools across 5 categories. JSON-schema follows Mimir's existing chat tool
convention (see `ro-ai-bridge/src/routes/chat/tools.rs`).

#### Category A — Pipeline trigger (4 tools)

```json
{
  "name": "lora.start_training",
  "description": "Kick off a LoRA fine-tune run on a Curator dataset. Returns run_id and ETA. The actual mlx_lm.lora training runs as a background task on Heimdall host.",
  "input_schema": {
    "type": "object",
    "required": ["dataset_id", "base_model_id", "name"],
    "properties": {
      "dataset_id": {"type": "string", "description": "UUID of training_corpus_dataset (must be approved+gated)"},
      "base_model_id": {"type": "string", "description": "MLX model ID, e.g. 'mlx-community/gemma-4-26b-a4b-it-4bit'"},
      "name": {"type": "string", "description": "Run name, e.g. 'phase2c-lora-rank16'"},
      "hyperparams": {
        "type": "object",
        "description": "Optional. If omitted, lora.suggest_hyperparams is called first.",
        "properties": {
          "iters": {"type": "integer"},
          "batch_size": {"type": "integer"},
          "learning_rate": {"type": "number"},
          "num_layers": {"type": "integer"},
          "rank": {"type": "integer"},
          "scale": {"type": "number"},
          "dropout": {"type": "number"},
          "max_seq_length": {"type": "integer"}
        }
      }
    }
  }
}
```

```json
{
  "name": "lora.suggest_hyperparams",
  "description": "Recommend LoRA hyperparameters based on dataset size, base model, prior runs, and operator goal (capacity vs speed vs safety). Returns explained suggestions.",
  "input_schema": {
    "type": "object",
    "required": ["dataset_id"],
    "properties": {
      "dataset_id": {"type": "string"},
      "goal": {"type": "string", "enum": ["max_capacity", "fast_iter", "safety_first", "balanced"], "default": "balanced"},
      "prior_run_id": {"type": "string", "description": "Optional. Suggestions will be diff-relative to this run."}
    }
  }
}
```

```json
{
  "name": "eval.run",
  "description": "Run dual-anchor eval (locked-20 + broader-100) on a fused adapter. Hot-swaps MLX server, runs HBp scoring, writes eval_runs row.",
  "input_schema": {
    "type": "object",
    "required": ["model_id"],
    "properties": {
      "model_id": {"type": "string", "description": "ai_models.id of the candidate (post-fuse)"},
      "anchors": {"type": "array", "items": {"type": "string", "enum": ["locked_20", "broader_100"]}, "default": ["locked_20", "broader_100"]},
      "restore_champion_after": {"type": "boolean", "default": true}
    }
  }
}
```

```json
{
  "name": "decision.recommend_promotion",
  "description": "Compare candidate eval results vs current champion against the dual-anchor +5pp gate. Returns PROMOTE | HOLD | REJECT with reasoning.",
  "input_schema": {
    "type": "object",
    "required": ["candidate_run_id"],
    "properties": {
      "candidate_run_id": {"type": "string"},
      "champion_id": {"type": "string", "description": "Optional; defaults to current production champion"}
    }
  }
}
```

#### Category B — Pipeline monitor (3 tools)

```json
{
  "name": "lora.get_run_status",
  "description": "Fetch live status of a training run: phase (pulling | training | fusing | done | failed), progress %, current loss, val loss, ETA.",
  "input_schema": {"type": "object", "required": ["run_id"], "properties": {"run_id": {"type": "string"}}}
}
```

```json
{
  "name": "lora.list_runs",
  "description": "List recent LoRA training runs (filterable by status, dataset, base_model, tenant).",
  "input_schema": {
    "type": "object",
    "properties": {
      "status": {"type": "string", "enum": ["running", "complete", "failed", "all"]},
      "dataset_id": {"type": "string"},
      "limit": {"type": "integer", "default": 20}
    }
  }
}
```

```json
{
  "name": "eval.compare_anchors",
  "description": "Side-by-side comparison of two model eval results across both anchors (locked-20 + broader-100), with per-dimension breakdown (accuracy/safety/calibration/communication/follow-up/instruction).",
  "input_schema": {
    "type": "object",
    "required": ["model_a", "model_b"],
    "properties": {
      "model_a": {"type": "string"},
      "model_b": {"type": "string"}
    }
  }
}
```

#### Category C — Dataset inspection (2 tools)

```json
{
  "name": "dataset.summary",
  "description": "Summarize a Curator dataset: item count, approved count, tag distribution, average pair length, safety-hedged item count, source mix (gemini batch vs hand-written).",
  "input_schema": {"type": "object", "required": ["dataset_id"], "properties": {"dataset_id": {"type": "string"}}}
}
```

```json
{
  "name": "dataset.list_items",
  "description": "List items in a dataset with filters (tag, status, search). Returns paginated results with prompt preview + completion preview.",
  "input_schema": {
    "type": "object",
    "required": ["dataset_id"],
    "properties": {
      "dataset_id": {"type": "string"},
      "tag": {"type": "string"},
      "status": {"type": "string", "enum": ["pending", "approved", "rejected", "flagged"]},
      "search": {"type": "string"},
      "limit": {"type": "integer", "default": 20}
    }
  }
}
```

#### Category D — Education (2 tools)

```json
{
  "name": "glossary.lookup_term",
  "description": "Plain-language definition of an ML/LoRA/HBp term, sourced from the Sprint 39 Learning Journal. Returns definition + example + 'see also' links.",
  "input_schema": {
    "type": "object",
    "required": ["term"],
    "properties": {
      "term": {"type": "string", "description": "e.g. 'LoRA rank', 'catastrophic forgetting', 'dual-anchor gate'"}
    }
  }
}
```

```json
{
  "name": "pipeline.explain_step",
  "description": "Explain a pipeline step (Curator | Synth | Train | Fuse | Swap | Eval | Gate) in plain language with input/output, gotchas, and links to relevant docs.",
  "input_schema": {
    "type": "object",
    "required": ["step"],
    "properties": {
      "step": {"type": "string", "enum": ["curator", "synth", "train", "fuse", "swap", "eval", "gate"]},
      "audience": {"type": "string", "enum": ["beginner", "ml_intermediate", "advanced"], "default": "ml_intermediate"}
    }
  }
}
```

#### Category E — Hyperparam advisor heuristics (1 tool, internal helper)

```json
{
  "name": "lora.diagnose_run",
  "description": "Given a completed training run + eval results, diagnose what likely went wrong (or right) and suggest next-iteration hyperparam deltas. E.g. 'val loss flat after iter 400 → reduce iters'; 'safety dim regressed → increase hedging items in corpus'.",
  "input_schema": {
    "type": "object",
    "required": ["run_id"],
    "properties": {
      "run_id": {"type": "string"}
    }
  }
}
```

### Backlog

| ID | Title | Size | Notes |
|---|---|---|---|
| **B-46a** | Backend `lora.*` and `eval.*` tool handlers in `ro-ai-bridge/src/routes/chat/tools.rs` | M (3-4d) | 8 tools wire to existing `/api/v1/training/*` and `/api/v1/eval/*` endpoints; no new DB columns |
| **B-46b** | `decision.recommend_promotion` handler with dual-anchor gate logic | S (1-2d) | Pure read; codifies the gate from baseline doc 04_03 |
| **B-46c** | `lora.diagnose_run` heuristic engine | M (2-3d) | Rules-based v0: loss curve shape, val gap, eval delta-per-dim. Input for future LLM-based v1 |
| **B-46d** | `lora.suggest_hyperparams` engine | M (2-3d) | Heuristics from 04_06 journal: dataset_size→iters, base_model→layers, goal→rank, prior_run→deltas |
| **B-46e** | RAG index of `04_06_LoRA_Sprint39_Learning_Journal.md` for `glossary.lookup_term` + `pipeline.explain_step` | S (1d) | Reuse Mimir's existing collection embedder; index by term + by step heading |
| **B-46f** | Mimir Assistant chat: tool registration + render-helpers | M (2-3d) | Register 12 tools in chat session; add small inline cards for run-status, eval-comparison, gate-decision |
| **B-46g** | Glossary tooltip UI in `ro-ai-dashboard/training/*` pages | S (1-2d) | Hover-popovers on technical terms in Curator + run detail pages, sourced from same RAG |
| **B-46h** | Tool-trace UI: show every tool call + result in chat thread (debuggable) | S (1d) | Mimir Assistant existing component, just enable for these tools |
| **B-46i** | Acceptance test scripts: 5 canonical chat scenarios | S (1d) | "Train a Phase 2d run", "Why did Phase 2 regress?", "Should we promote?", "Explain Step 3 to me", "What's a good rank for 4K dataset?" |
| **B-46j** | Onboarding doc: `docs/06_user_guides/06_03_Pipeline_Chat_Operator.md` | S (1d) | Walk-throughs with screenshots; non-engineer audience |

**Total ~3-4 weeks** for full B-46a→j.

### Acceptance criteria — Sprint 46

- [ ] All 12 tools callable from Mimir Assistant chat with valid JSON-schema validation
- [ ] User can complete the **5 canonical scenarios** end-to-end without leaving chat:
  1. "Start a Phase 2d run on dataset c56794c8 with safety-first goal"
  2. "Why did Phase 2 regress in safety?" (uses `lora.diagnose_run` + `glossary.lookup_term('catastrophic forgetting')`)
  3. "Did Phase 2c clear the gate?" (uses `decision.recommend_promotion`)
  4. "Explain the Fuse step like I'm a backend engineer who has never touched ML"
  5. "What rank should I try next?" (uses `lora.suggest_hyperparams` with prior run diff)
- [ ] All ML/LoRA glossary terms in 04_06 are looked-up-able with <1s latency
- [ ] Pipeline operator can promote a candidate via chat without touching CLI/SQL
- [ ] Tool traces visible in chat thread (debuggable for ML eng)
- [ ] Multi-tenant: chat respects `X-Tenant-Id` for run/dataset visibility

### Out of scope

- ❌ Generative UI (a2ui, Vercel AI SDK UI Streams) — Phase 2, gated on multi-tenant demand
- ❌ Hermodr/MCP wrapping — defer until Eir agent needs autonomous trigger (Sprint 47+)
- ❌ LLM-based diagnosis (Gemini reasoning over loss curves) — v0 is rules-based; LLM diagnosis is Sprint 47+
- ❌ Multi-step plan execution ("set up a 4-phase tournament") — chain explicitly via chat for now; orchestration agent is Sprint 48+
- ❌ Slack/email notifications when run completes — separate notif sprint

### Cost / value justification

| Benefit | Quantification |
|---|---|
| **Operator velocity** | Phase 2c kickoff today took 6 manual steps + 3 file edits + 2 SSH-equivalent. With Sprint 46: 1 chat sentence. |
| **Onboarding cost** | New ML engineer can run a tournament without CLI knowledge — ~3 days saved per onboard |
| **Decision quality** | Hyperparam suggestions encode 04_06 journal lessons → fewer wasted iters per sprint |
| **Documentation drift** | Tools call live DB → never stale, unlike static docs |
| **Reuse downstream** | Same tool surface usable by Eir agent if/when autonomous retraining lands (Sprint 47+) |

### Open questions (to resolve at sprint kickoff)

1. **Trace storage**: store tool traces in `chat_messages` JSONB or dedicated `tool_traces` table? (Recommend JSONB for v0)
2. **Hyperparam advisor backing**: rules-only v0 or hybrid (rules + LLM rationale)? (Recommend rules-only v0; LLM-rationale optional polish)
3. **Glossary RAG model**: reuse current Mimir embedder (BGE-M3) or upgrade to MedCPT? (Recommend BGE-M3 — terms are domain-general ML, not medical)

### Dependencies

- **Sprint 39** must be closed (✅ closed) — provides the API surface this sprint wraps
- **Sprint 45** (Mimir Batch API) NOT a hard dependency — but if landed first, Sprint 46 can include `batch.list_jobs` / `batch.start_synth` tools at low marginal cost (+S)
- **Mimir Assistant chat** must support tool-calling (✅ already does — used by Eir)

---

## Sprint 47 — Mimir RAG Eval (proposal, ~2 weeks)

**Trigger:** End-to-end HBp doesn't separate failure modes. When Sprint 39 Phase
2c lands flat or marginal, we can't tell whether the bottleneck is:
- LLM weights (→ keep iterating LoRA)
- Retrieval (→ re-chunk, re-embed, expand collection)
- LLM-not-using-context (→ different fine-tune target — instruction-following on context)

Without bottleneck attribution we burn future sprints on the wrong knob. RAG eval
metrics give a clean diff that turns "next sprint pick" from intuition into evidence.

### Architectural decision (locked 2026-05-06)

**Pattern:** Rust-native, Mimir-extend — **NOT** Python sidecar (option B), **NOT**
subprocess RAGAS (option C).

**Rationale:**
- Rust LLM/RAG ecosystem (candle, swiftide, rig, rust-bert) has **no
  production-ready RAGAS equivalent** as of 2026-05-06
- RAG eval metrics are mostly trivial: math (Recall@k, MRR) + LLM-as-judge prompts
  (Faithfulness, AnswerRelevancy, ContextPrecision, ContextRecall)
- Mimir already has `heimdall_client`, `eval_runs`, `eval_scores`, multi-tenant
  + JWT auth — extension is incremental (~ 8-11 dev-days)
- Single binary fits Asgard K8s deploy pattern; avoids Python deps in image
- **Open-source positioning:** could become first production-grade Rust RAG eval
  framework (AGPL-3.0 + Commercial Enterprise dual license per Asgard's open-core
  model) — strategic differentiator vs Python tools, copyleft protects upstream

**Phase 2 (deferred):** if Sprint 47 reveals dimensions where Mimir-native judge
prompts diverge meaningfully from upstream RAGAS, optionally add a Python sidecar
for research-grade variance comparison. Skip until needed.

### Library design — 6 metrics, 2 categories

```rust
// crates/mimir-core-ai/src/evaluation/rag_metrics.rs

#[async_trait]
pub trait RagMetric: Send + Sync {
    async fn score(
        &self,
        question: &str,
        context: &[String],
        answer: &str,
        gold: Option<&GoldRagItem>,
    ) -> Result<f64, EvalError>;
    fn name(&self) -> &'static str;
}

// LLM-as-judge metrics (RAGAS-style, no gold labels needed)
pub struct Faithfulness { judge: Arc<HeimdallClient> }
pub struct AnswerRelevancy { judge: Arc<HeimdallClient> }
pub struct ContextPrecision { judge: Arc<HeimdallClient> }
pub struct ContextRecall { judge: Arc<HeimdallClient> }  // needs gold answer

// Pure-Rust retrieval metrics (need gold relevant chunk_ids)
pub struct RecallAtK { k: usize }
pub struct MeanReciprocalRank;
pub struct NormalizedDcgAtK { k: usize }
pub struct HitRateAtK { k: usize }
```

### Tool placement

Same Mimir-native rule as Sprint 46 — all RAG eval lives in `mimir-core-ai`.
- ❌ NOT Hermodr/MCP — single consumer (eval pipeline + dashboard); zero benefit from cross-service hop
- ✅ If Sprint 46 lands → add `rag.diagnose_question` tool that wraps RAG metrics for chat UX (low marginal cost, +S)

### Backlog

| ID | Title | Size | Notes |
|---|---|---|---|
| **B-47a** | DB migration `eval_scores` extension + `rag_benchmark_items` table | S (1d) | Cols: faithfulness, answer_relevancy, context_precision, context_recall, retrieved_chunk_ids JSON, retrieval_recall_at_5, retrieval_recall_at_16, retrieval_mrr, retrieval_ndcg_at_8. New table: rag_benchmark_items (id, benchmark_id, question_id, collection_id, relevant_chunk_ids JSON, required_topics JSON, curated_by, curated_at) |
| **B-47b** | `RagMetric` trait + 4 RAGAS judge-based impls | M (3-4d) | Faithfulness, AnswerRelevancy, ContextPrecision, ContextRecall. Each is a judge-LLM prompt + JSON parse + score normalization 0-1. Reuse HeimdallClient. |
| **B-47c** | Capture `retrieved_chunk_ids` from agent invocation | S (1-2d) | Modify `ro-ai-bridge` chat path to surface chunk IDs to eval_runner; persist to eval_scores.retrieved_chunk_ids |
| **B-47d** | Pure-Rust retrieval metrics (Recall@k, MRR, NDCG@k, Hit Rate) | S (1d) | <100 LOC, zero deps, computed against rag_benchmark_items |
| **B-47e** | Counterfactual / ablation harness | M (2-3d) | Run-mode flag: `rag_mode = on|off|random|gold_only`. Eval runner respects mode. Outputs A/B/C/D ceiling estimate. |
| **B-47f** | Bottleneck attribution panel (Mimir Dashboard) | M (2-3d) | Per-question 2x2 (RAG_correct × LLM_correct). Per-run aggregate: "32% of failures are RAG-bottleneck, 41% LLM, 27% both". Per-collection breakdown. |
| **B-47g** | rag_benchmark_items curation extension to Curator UI | S (1-2d) | Reuse Sprint 39 Curator React components; add "label relevant chunks for question X" page + multi-select against Qdrant top-200 candidates |
| **B-47h** | Open-source surface: extract `mimir-rag-eval` crate (gated) | S (1-2d) | Crate split + minimal README + **AGPL-3.0 + Commercial Enterprise dual license** (per Asgard open-core model — see COMMERCIAL.md + CLA). **Gated on:** Sprint 47 internal proof + ≥1 month production use + multi-tenant validation |

**Total ~10-13 dev-days** for full B-47a→g (~2 calendar weeks). B-47h optional / deferred.

### Acceptance criteria — Sprint 47

- [ ] All 4 RAGAS metrics + 4 retrieval metrics computable per `eval_scores` row
- [ ] Counterfactual harness produces ceiling estimate (`Δ(perfect_RAG, current_RAG)`) on hb-pro-asgard-001
- [ ] Bottleneck attribution panel labels every Phase 3+ failure as: LLM-bottleneck | RAG-bottleneck | Both | Judge-disagreement
- [ ] ≥100 questions in `rag_benchmark_items` labeled by clinician (asgard_medical tenant)
- [ ] Per-collection retrieval health visible (`medical_knowledge`, `prime_kg`, future collections)
- [ ] Multi-tenant: per-tenant RAG benchmarks isolated (X-Tenant-Id respected)
- [ ] **Headline outcome:** Sprint 39 Phase 2c failure (if any) auto-classified — drives Sprint 48+ backlog from data, not intuition

### Out of scope

- ❌ Python sidecar / RAGAS upstream wrap — Mimir-native only for v0
- ❌ Auto-fix recommendations (suggested re-chunk strategies, embedder swaps) — diagnosis only; remediation is Sprint 48+
- ❌ Cross-tenant benchmark sharing — start tenant-scoped, federate later
- ❌ Streaming retrieval metrics — batch eval at job-end is sufficient
- ❌ Embedder fine-tuning — separate sprint if RAG diagnosed as bottleneck
- ❌ Re-ranker eval as separate component — fold into ContextPrecision for v0

### Cost / value justification

| Benefit | Quantification |
|---|---|
| **Decision quality (next-LoRA-iter)** | Sprint 39 Phase 2 → 2b → 2c spent ~$3.50 chasing capacity hypothesis. With Sprint 47, future "is it LLM or RAG?" answered before synth spend. ROI: avoid 1 wasted Gemini batch synth round = $5-10 |
| **Bottleneck attribution** | Quantifies whether next sprint should target LLM (LoRA) or RAG (re-embed/re-chunk). Saves 1-2 wrong-direction sprints/year ≈ 4-8 weeks dev. |
| **Open-source positioning** | First Rust production-grade RAG eval framework (AGPL-3.0 + Commercial Enterprise dual license). Strong copyleft protects from proprietary forks; commercial license preserves Megawiz moat. Attracts Rust LLM contributors. Hard to value but strategic. |
| **Multi-tenant audit trail** | Each tenant gets per-collection RAG health metrics — sellable feature for clinical orgs |
| **Counterfactual ceiling** | "How much better could we get with perfect RAG?" — bounds future RAG investment |

### Open questions (resolve at sprint kickoff)

1. **Single judge vs multi-judge** for RAGAS metrics: gpt-4.1 alone (cheap, low variance bound) vs gpt-4.1 + gemini-3-flash + gemma-4-26b averaged (expensive, research-grade). **Recommend single (gpt-4.1) v0**, multi-judge as opt-in flag for high-stakes runs.
2. **rag_benchmark_items reuse vs separate**: extend `training_corpus_items` (1 schema) vs new table (cleaner separation). **Recommend separate** — different lifecycle, different curators (clinician for retrieval-gold vs ML eng for training-gold).
3. **Counterfactual `gold_only` mode**: do we inject gold chunks at retrieval layer or prompt-assembly layer? **Recommend prompt-assembly layer** — cleaner isolation; retrieval-layer injection conflates with embedder eval.
4. **Per-collection or per-question primary aggregation?** **Recommend per-question** — collections shift over time; question-level aggregates are stable and let us migrate collections without losing comparability.

### Dependencies

- **Sprint 39** must be closed (✅) — provides `eval_runs` / `eval_scores` schema baseline
- **No hard dep on Sprint 45 or 46** — runs in parallel
- If **Sprint 46** lands first → add `rag.diagnose_question` tool to its toolkit (~1d marginal)
- If **Sprint 45** (Batch API) lands first → can re-judge prior eval runs in batch with new RAG metrics

### Strategic note — Open-source angle

Rust RAG eval is currently a gap in the LLM ecosystem. RAGAS (Python) has 6.8k
stars; Rust equivalents don't exist at production scale. If Sprint 47 internal
ships well, B-47h (extract `mimir-rag-eval` crate) becomes a deliberate
positioning play:
- **Asgard differentiator** — only multi-tenant Rust RAG eval framework
- **Audit trail-friendly** — single binary, no Python deps, fits regulated environments (HIPAA, hospital IT)
- **Aligns with Asgard AI Platform pitch** — clinical orgs already wary of Python ML stack ops burden

This is bonus, not blocker. Sprint 47 ships internally even if B-47h is deferred indefinitely.

---

## Sprint 48 — Thai Clinical Coding Foundation (proposal, ~3-4 weeks)

**Trigger:** Eir agents need ICD-10 Condition coding for FHIR-conformant encounter
records, DRG reimbursement, and HIS interoperability. International ICD-10 is
foundational; **ICD-10-TM (Thai Modification)** is Asgard's first Thai-native
differentiator — international cloud AI (GPT, Gemini) cannot do native Thai
semantic ICD lookup or Thai DRG mapping. This sprint is the first
"Thailand-first" feature commit.

### Architectural decision (locked 2026-05-07)

**Pattern:** Hermodr-resident skill (single tool surface for all 19 Eir agents)
with **local-first storage** in MariaDB + Qdrant. No external API per query —
ICD-10 dataset is a one-time ingest.

**Rationale:**
- Stateless lookup → fits Hermodr per Asgard hybrid tool placement rule
- Local-first → $0/query, no cross-border data transfer (PDPA),
  no rate limits, latency <100ms
- Bilingual schema (en + th labels) → both international + Thai users served
  by same skill; no fork
- Qdrant Thai semantic search → handles Thai typos/synonyms/abbreviations
  ("หลอดเลือดสมอง" → "I63.9") that exact-match misses
- DRG mapping built in → not just coding, but billing-ready output

### License & sourcing

| Source | License | Action |
|---|---|---|
| **WHO ICD-10** (international, English) | Public domain post-1990 | ✅ direct download |
| **ICD-10-TM 2017** (Thai MoPH) | Thai government public document; formal license recommended | 🟡 B-48a — email request to **Bureau of Health Information / กองยุทธศาสตร์และแผนงาน** (1-2 wk turnaround) |
| **DRG mapping** (สปสช./MoPH) | Published; non-commercial OK; commercial needs review | 🟡 confirm สปสช. policy at sprint kickoff |

**Risk mitigation:** Dev kicks off with international ICD-10 immediately
(B-48b/c independent of license); Thai-specific work (B-48d/f/g) gates on
B-48a license confirmation. License acquisition starts day-1 in parallel.

### Backlog

| ID | Title | Size | Notes |
|---|---|---|---|
| **B-48a** | License acquisition — formal request to Bureau of Health Information for ICD-10-TM 2017 + DRG | S (1d setup, 1-2 wk gov turnaround) | **Day-1 kickoff**; gates B-48d/f/g but not other items. Email + follow-up. Document license terms in `Asgard/legal/`. |
| **B-48b** | DB migration `icd10_codes` table (en + th + chapter + block + DRG) | S (1d) | Cols: code, en_label, th_label, chapter, block, billable_flag, drg_id, locale_metadata JSON, source_version |
| **B-48c** | WHO ICD-10 international ingest (English master) | S (1-2d) | Public-domain dataset; ETL script; ~107K codes; verify chapter/block counts |
| **B-48d** | ICD-10-TM 2017 ingest (Thai MoPH source) | M (3-4d) | Excel/PDF parse; Thai text normalize; align to international ICD-10 codes; flag Thai-specific extensions; **gated on B-48a** |
| **B-48e** | Hermodr handler `icd10_lookup` (exact + prefix match, en + th) | M (2-3d) | Rust handler in Hermodr; signature `icd10_lookup(query, mode='code'\|'term', locale='en'\|'th'\|'both')`; pagination; relevance score |
| **B-48f** | Qdrant Thai semantic search (BGE-M3 embedded) | M (2-3d) | New collection `icd10-th`; embed Thai labels; fallback when exact-match fails; merge into Hermodr handler response |
| **B-48g** | DRG mapping (Top 100 DRG groups) | M (2-3d) | สปสช. DRG groups → ICD-10-TM combo lookup; Phase 1 covers ~80% of clinical encounters; Phase 2 (later sprint) covers full mapping |
| **B-48h** | FHIR Condition.code wiring in Eir agents | S (1-2d) | When Eir generates encounter Condition resource, auto-suggest ICD-10-TM code; persist `system="https://www.who.int/icd"` + `version="ICD-10-TM 2017"` |
| **B-48i** | Eir agent tool allowlist update | S (1d) | Add `icd10_lookup` to allowlists for: Internal Medicine, Surgery, Pediatrics, OB-GYN, Emergency, Psychiatry, MedTech, Pharmacy, Nursing (the 9 agents that handle dx coding) |
| **B-48j** | Test set: 50 Thai-encounter cases + en/th cross-language eval | M (2-3d) | Curate from real (anonymized) Thai discharge summaries; clinician-validated gold codes; HBp-style judge eval |

**Total ~ 15-22 dev-days** (~ 3-4 calendar weeks given license acquisition wait)

### Acceptance criteria — Sprint 48

- [ ] Eir Internal Medicine + Pharmacy + Coder agents lookup ICD-10 in en + th
- [ ] Thai semantic search returns relevant codes for natural-language queries
      (e.g. "หลอดเลือดสมองตีบ" → `I63.9` Cerebral infarction)
- [ ] Cross-language: query in English returns Thai-labeled results, and vice versa
- [ ] DRG lookup returns valid DRG group for representative ICD-10-TM combos
- [ ] FHIR Condition resource auto-codes correctly in Eir-generated encounters
- [ ] 45/50 (90%) test cases produce clinician-acceptable codes
- [ ] Latency p50 <100ms (local lookup) / <300ms (Qdrant fallback)
- [ ] Multi-tenant: per-tenant `tenant_id` on every query (audit trail), shared codes table

### Out of scope

- ❌ SNOMED-CT (Sprint 50+ — separate clinical terminology)
- ❌ LOINC (lab codes, Sprint 51+)
- ❌ CPT (US-specific procedure codes, Sprint 51+)
- ❌ ICD-11 — still beta deployment in Thailand 2026; revisit when MoPH adopts
- ❌ Auto-coding from free-text discharge summary (extraction) — separate
      Sprint 52+ deep-NLP feature
- ❌ Insurance claims integration (สปสช./hospitals) — Sprint 50+ ops integration

### Cost / value justification

| Benefit | Quantification |
|---|---|
| **Thai market differentiator** | Cloud AI (GPT, Gemini) cannot do native Thai semantic ICD lookup; this is a "Thai hospital" wedge feature |
| **FHIR completeness** | Eir-generated Condition resources become billing-ready; HIS integration unblocked |
| **DRG-ready output** | Hospitals can use Eir output for reimbursement workflow without manual recoding |
| **Cost** | $0/query (local lookup); one-time license (gov, expected free); ~3-4 wk dev |
| **Reuse** | Foundation for future Thai-localization sprints (Sprint 49+: Thai FDA drug DB, Thai pharmacology, Thai clinical guidelines) |

### Open questions (resolve at sprint kickoff)

1. **License terms for ICD-10-TM 2017** — public commercial use OK?
   commercial-but-attribution OK? Government may require "Asgard powered by
   ICD-10-TM 2017 from MoPH" attribution string. **Resolved at B-48a** turnaround.
2. **DRG version** — สปสช. DRG v6 (current) vs older versions. **Recommend v6**
   (latest stable, used by majority of hospitals).
3. **Coverage** — 100% of ICD-10-TM codes or top X% by usage? **Recommend
   100%** for international ICD-10 (small dataset); top-200 DRG groups for v0
   (cover ~80% of clinical encounters).
4. **Embedding model** — BGE-M3 (current Mimir embedder) vs MedCPT (medical-specialized) vs Thai-trained encoder. **Recommend BGE-M3 v0** (multilingual, including Thai); revisit if Thai recall <0.7.

### Dependencies

- **Sprint 39** must be closed (✅) — provides eval baseline
- **No hard dep on Sprint 45/46/47** — runs independently
- **Eir Agents** (Sprint 38+) must support tool extension — already does
- **License acquisition (B-48a)** must complete before B-48d/f/g; international items can ship in week 1

### Strategic note — Thailand-first positioning

This is **Asgard's first explicitly Thai-localized feature**. Future
Thailand-first sprints follow same pattern:

- **Sprint 49+ candidates:** Thai FDA drug DB · MoPH clinical guidelines ·
  สปสช. formulary · Thai-language clinical communication patterns ·
  Thai medical jargon glossary
- **Sales angle:** "Thai hospital first-class citizen, not afterthought" —
  position vs international cloud AI as: *they translate Thai to English then
  back; we natively understand Thai medicine*

This sprint plants the flag for that positioning. Landing page UseCases
section should highlight ICD-10-TM as a flagship Thailand-first capability.

---

## Sprint 49 — MedOpenClaw Skill Integration — Phase 1 (proposal, ~3 weeks)

**Trigger:** 869 OpenClaw Medical Skills (FreedomIntelligence/OpenClaw-Medical-Skills,
MIT) cloned to `~/Developer/MedOpenClaw/skills/` but not integrated. The
[medical_agents_strategy_20260430.md](medical_agents_strategy_20260430.md)
strategy doc identified this as Phase 1 work for May 2026 but it was never
formally sprinted — this sprint closes that gap with a 5-priority focus +
adapter template that scales the next 50.

### Architectural decision (locked 2026-05-07)

**Pattern:** Hermodr-resident handlers (stateless external-API style) +
**Mimir skill registry** (catalog of all 869 skills with embeddings for
ToolRAG discovery).

**Why not all 869 at once:**
- Tool catalog explosion: 869 × ~250 tokens = 217K tokens (≫ 8K context)
- Each skill calls 1+ external services (14 underlying data sources, varying
  auth/rate-limits/cost)
- Per-skill safety review (Sprint 43 B-61 pre-flight screen) takes time
- Per-agent allowlist mapping = 869×19 = 16,511 cells

**Phased approach:**
- **Sprint 49 (this):** 5 priority skills + adapter template + ToolRAG scaffold
- **Sprint 49b (later):** Auto-port batch (200-300 skills) using template
- **Demand-driven later:** Hospital-partner specialty asks → batch port

### Backlog

| ID | Title | Size | Notes |
|---|---|---|---|
| **B-49a** | Mimir skill registry — `mimir_skills` table + Qdrant collection `mimir-skills` | M (2-3d) | Schema: id, slug, source_repo, source_version, en_description, th_description, allowlist_agents JSON, status, ported_at; Qdrant embed of descriptions for ToolRAG |
| **B-49b** | Catalog ingest — bulk-load all 869 MedOpenClaw skill metadata into registry (cataloged but not ported) | S (1d) | Read SKILL.md headers from `~/Developer/MedOpenClaw/skills/*/SKILL.md`; no port, just metadata + embeddings |
| **B-49c** | AsgardSkillAdapter template (Rust) — generic wrapper for MedOpenClaw SOPs | M (3d) | Trait + macro to wrap an SOP into a Hermodr handler with consistent signature, tracing, rate-limit, audit |
| **B-49d** | Port `pubmed-search` skill (priority #1) | S (1-2d) | Wrap E-utilities API; auth-free; rate-limit 3/s; multi-tenant key not needed |
| **B-49e** | Port `drug-drug-interaction` skill (priority #2) | M (2-3d) | DrugBank or RxNav source; commercial license check; per-tenant key isolation |
| **B-49f** | Port `clinical-trial-matching` skill (priority #3) | S (1-2d) | ClinicalTrials.gov API; auth-free |
| **B-49g** | Port `differential-diagnosis` skill (priority #4) | M (2d) | Pure SOP (no external API); LLM-driven; reasoning chain template |
| **B-49h** | Port `cpic-pharmacogenomics` skill (priority #5) | M (2-3d) | CPIC guideline API; precision-medicine differentiator |
| **B-49i** | ToolRAG retrieval — Bifrost queries `mimir-skills` for top-3-5 relevant skills before Eir agent call | M (2-3d) | Pattern from Strategy doc Phase 2; embeds query → searches Qdrant → injects skill descriptions into agent prompt |
| **B-49j** | Per-agent allowlist update for 5 priority skills across 19 Eir agents | S (1-2d) | Conservative defaults; clinician review for high-risk pairings (e.g. pediatric DDI) |
| **B-49k** | Acceptance: 5 priority skills callable from Eir agents + ToolRAG retrieval works on 20 test queries | S (1d) | End-to-end smoke; HBp delta on query subset that benefits from skill use |

**Total ~ 12-17 dev-days** (~ 3 weeks)

### Acceptance criteria — Sprint 49

- [ ] All 869 skills cataloged in registry (metadata only)
- [ ] All 869 skill descriptions embedded in Qdrant (ToolRAG-discoverable)
- [ ] 5 priority skills (pubmed, DDI, trial-matching, diff-dx, CPIC) ported,
      Hermodr-resident, callable from Eir agents
- [ ] AsgardSkillAdapter template demonstrably reduces port-time to
      <1 hr per skill (measured on 6th skill as control)
- [ ] ToolRAG returns top-3 relevant skills for representative test queries
- [ ] HBp run on skill-applicable subset shows non-regression vs prior
- [ ] Per-tenant audit trail captures `(tenant_id, skill_slug, args, source_version)`

### Out of scope

- ❌ Auto-port the next 200-300 (Sprint 49b)
- ❌ Skill performance benchmarks per-skill (Sprint 47 RAG eval extension)
- ❌ Skill versioning / rollback infrastructure (Sprint 50+)
- ❌ Cross-skill orchestration (Bifrost composition) — handled at agent layer
- ❌ Customer-facing "skill marketplace" UI

### Cost / value justification

| Benefit | Quantification |
|---|---|
| **Honest landing page claim** | Currently we say "869 OpenClaw skills" but only ~5 are wired; this sprint catalogs all 869 + ports 5 = honest message |
| **Clinical coverage** | 5 priority skills hit pubmed/DDI/trial-match/diff-dx/CPIC = covers ~30% of common clinical queries (per FreedomIntelligence usage stats) |
| **Adapter template ROI** | After Sprint 49, each new skill = ~1 hr port (vs 3-4 hr today) — unlocks demand-driven hospital onboarding |
| **ToolRAG foundation** | Required for ALL future skill scaling (auto-port, marketplace, cross-skill orchestration) |
| **Cost** | $0 for OSS skills; small ongoing cost for paid data sources (DrugBank); ~3 wk dev |

### Open questions

1. **DrugBank vs RxNav for DDI** — DrugBank has commercial license; RxNav is
   free NLM. Recommend **RxNav v0** (free, sufficient for v0); upgrade to
   DrugBank if accuracy gap measured.
2. **Bilingual skill descriptions** — port skill descriptions in both en + th
   for catalog (so Thai users discover via Thai search)? Recommend **yes**,
   auto-translate via Gemini batch (Sprint 45 dependency or stand-alone).
3. **Skill versioning** — pin source_version per skill so repo updates don't
   break Hermodr. Recommend **immutable `source_version` field** + opt-in
   re-port workflow.

### Dependencies

- **Sprint 47 RAG Eval** strongly recommended first — gives bottleneck
  attribution that justifies *which* skills add value vs. which are noise
- **Sprint 48 Thai Clinical Coding** — ICD-10-TM useful as foundation for
  Thai-localized skill descriptions in this sprint's catalog
- **Sprint 45 Mimir Batch API** — useful (not blocking) for bulk Thai
  description translation

---

## Sprint 50 — 👁️ Syn S1 OCR Foundation (advanced, ~3.5 weeks)

**Codename:** Syn — Norse goddess of vigilance / "watching over". TOR (Tools
of Recognition) sub-system for visual document processing. Sister services
Sága (STT, Sprint 52) and Visual BMI (Sprint 53) follow same pattern.

**Trigger:** Asgard roadmap [`Asgard/docs/strategy/roadmap.md`](../../Asgard/docs/strategy/roadmap.md)
originally placed Syn S1 in Q2 2026 ("Now"); Sprint 39d clinician wait
opens dev bandwidth that lets it advance from the original Q3 slot. Mega-Care
synergy is direct: patient intake (Thai ID), prescription scanning, lab
report ingest. Hospital partner ROI immediate.

### Architectural decision (locked 2026-05-08, ADR-006)

**Stack: 4-tier hybrid OCR (local-first, cloud opt-in)**

| Tier | Engine | License | Cost | Use case | Tenant opt-in? |
|:---:|---|---|---|---|:---:|
| **1a** | `datalab-to/chandra` (10.5k ⭐) | Apache 2.0 | $0 (local) | Handwriting · complex tables · forms | default ON |
| **1b** | `PaddleOCR` PP-OCRv4 (~50k ⭐) | Apache 2.0 | $0 (local) | Thai stock print · fast latency | default ON |
| **2** | `gemini-3-flash` (cloud) | proprietary API | ~$0.001-0.005 / page | Local low-confidence fallback · multilingual edge cases | **opt-in per tenant** |
| **3** | `gemini-3.1-pro` (cloud) | proprietary API | ~$0.05-0.20 / page | High-stakes documents (legal, critical lab, complex layouts) · second opinion | **opt-in per tenant + per-call confirmation** |

**Rejected:** `surya` / `marker` from datalab (GPL-3.0). Conflict with Asgard
Commercial Enterprise tier — viral GPL clause forces Enterprise customers
embedding Asgard to GPL their entire product. Apache 2.0 alternatives
preserve the open-core moat.

**Deployment:**
- **Local tier** (1a/1b): Heimdall sidecar pattern (port 8084 chandra, 8085 PaddleOCR)
- **Cloud tier** (2/3): existing Heimdall step-up router — same path as
  Sprint 36 Heimdall Gemini fallback, reuses per-tenant API key resolution
  + budget tracking from `model_pricing` + `llm_usage` tables
- **Smart router**: rule-based v0 (see Router rules below); ML-based v1 deferred

**Router rules (engine selection)** — evaluated in order, first match wins:

```
1. PHI-sensitive flag set on tenant       → force LOCAL (1a or 1b only)
2. Per-call --engine override (manual)    → that engine
3. Document type explicit (handwriting,
   complex_table, form, thai_print)       → 1a or 1b per type
4. Document size > 5 pages OR             → tier 3 (Pro) IF tenant cloud_pro=ON
   marked "critical" / "legal"               ELSE tier 2 (Flash) ELSE 1a (best-effort local)
5. Local engine confidence < 0.70         → tier 2 (Flash) IF tenant cloud_flash=ON
                                             ELSE return local result + warn
6. Default                                → 1a chandra → fallback 1b PaddleOCR
```

Tenant cloud opt-in stored in existing `tenant_settings` table:
- `ocr_cloud_flash_enabled` (BOOLEAN, default FALSE)
- `ocr_cloud_pro_enabled` (BOOLEAN, default FALSE; requires Flash also enabled)
- `ocr_phi_strict` (BOOLEAN, default TRUE — never cloud regardless of above)
- `ocr_monthly_cloud_budget_usd` (DECIMAL, default 0; per-tenant cap)

Same architectural pattern as future Sága (STT) and Hermóðr (notify).

### Backlog

| ID | Title | Size |
|---|---|---|
| **B-50a** | Heimdall: deploy chandra (port 8084) + PaddleOCR (port 8085) as local sidecars | M (3-4d) |
| **B-50b** | Smart router: rule-based engine selection (PHI flag, doc type, confidence threshold, cloud opt-in, budget cap) | M (2-3d) |
| **B-50c** | REST endpoint `POST /api/v1/ocr/extract` (multipart → text + bbox + confidence + engine_used + cost_usd) | S (1d) |
| **B-50d** | Bifrost: image-with-text-content → OCR → existing agent flow (transparent path) | S (1-2d) |
| **B-50e** | Mimir: `ocr_documents` table + audit (image_hash, ocr_engine, extracted_text, confidence, cost_usd, tenant_id) | S (1d) |
| **B-50f** | Mimir Curator extension: clinician reviews OCR output, marks errors → corrections corpus | M (2-3d) |
| **B-50g** | Eir agent allowlist: add `ocr_extract` tool to `eir-medtech`, `eir-pharmacy`, `eir-internal-medicine` | S (1d) |
| **B-50h** | Test set: 30 Thai medical documents (10 print, 10 handwriting, 10 table) — measure CER per category × per engine (4 engines × 30 docs = 120 cells); target ≤5% print, ≤15% handwriting | M (clinician partner ~2d wall) |
| **B-50i** | UI: drag-drop upload in Mimir Dashboard `/chat` with OCR preview + text edit + **engine choice + cost preview** before send | M (2d) — bumped for cloud preview |
| **B-50j** | End-to-end test: lab report image → Eir-medtech → ICD-10 codes (chains B-48h FHIR) | S (1d) |
| **B-50k** | **Heimdall: Gemini 3 Flash + 3.1 Pro OCR adapter** — reuse existing `gemini_helper::call_text` with vision multimodal payload (`{type:'image_url'}`); per-tenant API key resolution | M (2-3d) |
| **B-50l** | **Tenant settings: `ocr_cloud_flash_enabled`, `ocr_cloud_pro_enabled`, `ocr_phi_strict`, `ocr_monthly_cloud_budget_usd` cols + admin UI page** | S (1-2d) |
| **B-50m** | **Cost guard middleware: pre-call USD estimate, monthly budget cap enforcement, audit row in `llm_usage` for every cloud OCR call** | M (1-2d) |

**Total ~ 19-25 dev-days** (~4 calendar weeks · bumped from 3.5 wk for cloud tier additions).

### Acceptance criteria — Sprint 50

- [ ] **Local-tier Thai OCR**: CER ≤5% on print benchmark, ≤15% on handwriting (PaddleOCR + chandra stock baselines)
- [ ] **Cloud-tier Gemini 3 Flash**: CER ≤3% on print, ≤10% on handwriting (cloud premium baseline)
- [ ] **Cloud-tier Gemini 3.1 Pro**: CER ≤2% on print, ≤7% on handwriting (high-stakes baseline)
- [ ] Eir-medtech ingests a lab report image → extracts text → suggests ICD-10 codes (chains Sprint 48 B-48h FHIR)
- [ ] **Audit trail** captures every OCR call (image_hash + extracted_text + ocr_engine + confidence + cost_usd + tenant_id)
- [ ] Latency p50 ≤2s local · ≤4s Gemini Flash · ≤8s Gemini Pro for typical 1-2 page lab report
- [ ] **Cloud safety**: PHI-strict tenant flag MUST block cloud calls 100% (verified via 20 PHI-marked test cases)
- [ ] **Cost guard**: monthly budget cap enforced — request rejected with explicit message when exceeded
- [ ] **Cost preview UI**: user sees USD estimate before any cloud OCR call; explicit confirmation for Gemini Pro
- [ ] Smart router correctly picks engine: ≥85% accuracy on engine-selection benchmark
- [ ] Multi-tenant: per-tenant OCR audit log; per-tenant cloud opt-in flags + budget cap; image storage isolated per tenant

### Out of scope (deferred to Sprint 51 = Syn S2)

- ❌ eKYC + Thai ID parser (Syn S2)
- ❌ Face matching / biometric (Syn S2)
- ❌ Cloud OCR fallback (Google Vision API) — local-first only
- ❌ Multi-page PDF (use pdftext separately if needed; full PDF→markdown deferred)
- ❌ Receipt / invoice parser (out of medical scope)

### Cost / value justification

| Benefit | Quantification |
|---|---|
| **Mega-Care intake automation** | Reduces patient-intake data entry (current pain point per Mega-Care ops) — ROI within 2 sprints |
| **Hospital scan ingest** | Lab/prescription/intake form digitization — eliminates manual coder transcription |
| **Differentiator vs cloud OCR** | Local-first + Thai-first + medical-context + audit trail in single binary surface |
| **Premium cloud tier** | Gemini Pro for hospital admin (legal docs, complex insurance forms, English medical literature) — opt-in revenue feature |
| **Cost — Tier 1 local** | $0 inference (chandra + PaddleOCR Apache 2.0) · runs on Mac mini |
| **Cost — Tier 2 Flash** | ~$0.001-0.005 / page (≈$1-5/1000 pages); typical small clinic 5K pages/mo = $5-25 |
| **Cost — Tier 3 Pro** | ~$0.05-0.20 / page (≈$50-200/1000 pages); reserve for high-stakes only — typical 100 pages/mo = $5-20 |
| **Dev** | ~4 wk (3.5 wk local + 0.5 wk cloud tier additions) |

### Open questions (resolve at sprint kickoff)

1. **chandra Thai accuracy** — chandra has no explicit Thai model; is it Thai-capable via multilingual training? **Resolve at B-50h benchmark**; if Thai CER >20% on chandra → use PaddleOCR for Thai stock + chandra only for handwriting/tables.
2. **MLX runtime for local OCR** — both chandra + PaddleOCR are Python/PyTorch (CPU/CUDA); no MLX path yet. **Recommend** running them as Python sidecars on host (similar to Heimdall pattern); revisit MLX port if Mac-mini latency unacceptable.
3. **Image storage** — Vault (small files, audit-friendly) vs S3-compatible object store (scale). **Recommend** Vault for v0 (≤10K images); migrate to MinIO if growth.
4. **Gemini API key model** — per-tenant key (each hospital provides its own Google API key) vs Megawiz pool key (we resell with markup). **Recommend** per-tenant for PDPA cleanliness; Megawiz pool only for Mega-Care internal use.
5. **Default cloud opt-in** — should Gemini Flash be ON by default for new tenants (cheap, useful) or strictly opt-in? **Recommend strict opt-in** — PHI-first posture; clinician explicit consent before any cloud call.
6. **Cost preview UX** — block & confirm every cloud call (annoying) vs warn once per session vs only confirm Pro. **Recommend** confirm only Gemini Pro (≥$0.05/call); Flash silent within budget cap.

### Dependencies

- **Sprint 39d clinician work** runs in parallel — different surface (RAG vs OCR), no conflict
- **Sprint 48 B-48h FHIR** — completes the OCR → ICD-10 → FHIR Condition pipeline end-to-end
- **No hard dep** on Sprint 45/46/47 — runs independently

### Cross-references

- ADR-006 (this sprint's stack decision): [`Asgard/docs/architecture/ADR-006-Syn-OCR-Stack.md`](../../Asgard/docs/architecture/ADR-006-Syn-OCR-Stack.md)
- ADR-007 (Skuggi PII guardrail gating cloud OCR): [`Asgard/docs/architecture/ADR-007-Skuggi-PII-Guardrail.md`](../../Asgard/docs/architecture/ADR-007-Skuggi-PII-Guardrail.md)
- Asgard roadmap (Syn S1 → S2 → Sága → Visual BMI): [`Asgard/docs/strategy/roadmap.md`](../../Asgard/docs/strategy/roadmap.md)
- Sprint 50b = 🌑 Skuggi PII Guardrail (parallel, ~2 wk) — gates cloud opt-in safety
- Sprint 51 = Syn S2 eKYC + Thai ID (next, ~2 wk)
- Sprint 52 = Sága S1 Whisper Foundation (~2 wk)
- Sprint 53 = Visual BMI / vision LLM (~3 wk)

---

## Sprint 50b — 🌑 Skuggi PII Guardrail (parallel, ~2 weeks)

**Codename:** Skuggi — Old Norse for "shadow". Hides PII in shadow before
any cloud LLM call sees the document. Sister to Týr (Wazuh SIEM) on
the security/compliance side, complements Sprint 50 Syn (OCR) and future
Sága (STT) by gating their cloud-bound calls.

**Trigger:** Sprint 50 introduces cloud OCR (Gemini Flash/Pro). The
existing PHI-strict tenant flag is all-or-nothing — disabling cloud
entirely renders the cloud premium tier unusable. Hospitals need
**granular "redact then cloud-OK" posture** for PDPA compliance + clinical
productivity. Skuggi delivers that.

### Architectural decision (locked, ADR-007)

**Pattern:** Heimdall pre-LLM middleware (in-process Rust, no extra hop)
**Image PII stack** (zero new external libs — all reuse Sprint 50 deps):
- OpenCV YuNet for face detection (built-in since OpenCV 4.7)
- PaddleOCR text + bounding boxes for Thai-ID/MRN region detection
- Rust regex on extracted text → match Thai-ID 13-digit / MRN / phone
- OpenCV blur for redaction

**Text PII stack** (1 new sidecar):
- **Tier 1: Rust regex in-process** — Thai national ID, MRN, phone, email, DOB, plate (<1ms latency)
- **Tier 2: PyThaiNLP sidecar (port 8086)** — Thai person names, addresses, hospital names; only invoked when Tier 1 has low coverage suspicion (~50-100ms)

**Modes:**
| Mode | Behavior | Default |
|---|---|---|
| `off` | No redaction | never default |
| `detect-only` | Log PII found, don't redact (audit/staging) | dev tenants |
| `mask-and-send` | Redact + send to cloud | ✅ **default for new tenants** |
| `block-on-pii` | Fail call if PII found (strictest) | high-PHI hospitals |

**Reversibility:** v0 = irreversible (one-way). OCR doesn't need
rehydration — local document on disk has original PII; cloud just
returns text structure. Sprint 52+ (Sága voice, vision LLM) gets
reversible mapping with HSM-protected keys when contextual reasoning
requires it.

### Backlog

| ID | Title | Size |
|---|---|---|
| **B-50b-1** | DB migration: `pii_redactions` audit table + `tenant_settings.pii_mode` enum col + `pii_custom_patterns` JSON | S (1d) |
| **B-50b-2** | Heimdall: Skuggi middleware Rust trait — wraps every cloud-bound LLM call (text + image) | M (3d) |
| **B-50b-3** | Image PII detector: OpenCV YuNet face + PaddleOCR text bbox + Thai regex match → OpenCV blur (zero new external lib) | M (2-3d) |
| **B-50b-4** | Text PII Tier 1: Rust regex (Thai 13-digit national ID, MRN, phone, email, DOB, plate) — pure in-process | S (1-2d) |
| **B-50b-5** | Text PII Tier 2: PyThaiNLP sidecar (port 8086) for Thai person names + addresses; called on Tier 1 low-coverage signal | M (2d) |
| **B-50b-6** | Skuggi config UI: per-tenant `pii_mode` selector + custom regex extension form | S (1-2d) |
| **B-50b-7** | Test set: 50 mixed PII docs (Thai ID, MRN, faces, signatures, addresses) — measure detection recall ≥98%, precision ≥90% | M (2d, clinician partner) |
| **B-50b-8** | Audit log dashboard: per-tenant PII redaction history (what + when + by whom) | S (1d) |

**Total ~12-15 dev-days** (~2 calendar weeks).

### Acceptance criteria

- [ ] Detection recall ≥98% on test set (false negatives = PHI leak = unacceptable)
- [ ] Detection precision ≥90% (false positives = noise but not unsafe)
- [ ] Image: faces, Thai ID/MRN, signatures blurred before cloud call
- [ ] Text: Thai 13-digit ID, MRN, phone, email, DOB redacted via Tier 1 regex (latency <1ms)
- [ ] Tier 2 PyThaiNLP captures ≥85% of Thai person names that Tier 1 missed
- [ ] **Sprint 50 cloud OCR (B-50k) MUST chain through Skuggi when tenant `pii_mode != off`**
- [ ] PHI-strict tenant: 100% of cloud calls blocked when `pii_mode = block-on-pii` and PII detected
- [ ] Audit row per redaction: `(tenant_id, image_hash_pre, image_hash_post, redaction_count, pii_types[], engine, ts)`
- [ ] Skuggi adds ≤300ms p50 latency to cloud OCR call (mostly Tier 2 NER when triggered)

### Dependencies

- **Sprint 50 B-50k** (Gemini OCR adapter) — Skuggi sits between client and Gemini
- **Sprint 50 B-50l** (tenant cloud opt-in flags) — `pii_mode` adjacent to opt-in flags
- **Multi-tenant** — uses existing X-Tenant-Id JWT pattern

### Out of scope (deferred)

- ❌ Reversible mapping with HSM-protected keys (Sprint 50.5 / Sprint 52)
- ❌ Voice PII redaction (Sprint 52 Sága integrates Skuggi for STT)
- ❌ Custom NER fine-tuning for Thai medical names (Sprint 53+)
- ❌ Differential privacy / k-anonymity (academic, defer)
- ❌ Reversible token vault (paid tenant feature, Sprint 51+)

### Cost / value justification

| Benefit | Quantification |
|---|---|
| **PDPA compliance for cloud OCR** | Without Skuggi, cloud OCR is unusable for PHI-cautious hospitals — feature dies in v0 |
| **Audit trail for regulators** | Per-redaction log = evidence of PHI defense for hospital legal/compliance |
| **Reusable across all cloud LLM calls** | Sprint 52 voice (Sága), Sprint 53 vision LLM, future Eir cloud step-up — all chain Skuggi |
| **Asgard pattern alignment** | Reuses Sprint 50 deps (OpenCV, PaddleOCR); adds 1 small sidecar (PyThaiNLP); Rust-first middleware |
| **Cost** | $0 inference (all local PII detection); ~2 wk dev |

### Open questions (resolve at sprint kickoff)

1. **Reversibility v0 trade-off** — irreversible loses ability to rehydrate cloud responses with original PII. **OK for OCR** (we already have local text); revisit at Sprint 52 voice when cloud reasoning needs context.
2. **Thai NER performance on PyThaiNLP** — 50-100ms latency may be too high for high-volume tenants. **Mitigation**: Tier 2 only fires on Tier 1 low-coverage signal (~10-20% of calls), not every call.
3. **Custom regex per tenant** — hospitals have varying MRN/HN formats. **Recommend**: tenant_settings JSON column for tenant-specific regex extensions (auditable, easy to add).
4. **Skuggi as standalone service vs Heimdall middleware** — middleware (in-process) faster. **Recommend** middleware v0 + extract to standalone if Bifrost/Mimir need direct access (Sprint 51+).

### Cross-references

- ADR-007: [`Asgard/docs/architecture/ADR-007-Skuggi-PII-Guardrail.md`](../../Asgard/docs/architecture/ADR-007-Skuggi-PII-Guardrail.md)
- Sprint 50 (gates this sprint's cloud opt-in): [Sprint 50 above]
- Future: Sprint 52 Sága integrates Skuggi for voice PII (reversible v1)

---

## Sprint 50+ — Production Agent Observability (placeholder, post-LoRA-promote)

**Trigger conditions** (all must hold to start):
- Sprint 39 LoRA adapter promoted to production (Eir agent serving traffic)
- Production traffic ≥100 invocations/day (otherwise observability ROI low)
- At least one tenant beyond `asgard_medical` onboarded (multi-tenant tracing concerns)

### Likely backlog (subject to revisit)

| Candidate ID | Title | Notes |
|---|---|---|
| OBS-1 | Production LLM trace ingest (every Eir call → trace store) | Decision: **Laminar self-hosted** vs **extend Mimir traces** vs **build Rust-native tracer**. Re-evaluate Laminar at decision point — service availability + ops burden + ecosystem state. |
| OBS-2 | Online judge sampling (run gemini-2.5-flash on % of prod traffic, alert on quality drop) | Cost gate: ~$X/month per 1% sampling rate × prod volume. Budget per tenant. |
| OBS-3 | Drift detection (compare prod traffic distribution vs locked HBp items; alert on mismatch) | Statistical primitive on top of trace store. |
| OBS-4 | Multi-tenant trace isolation (each hospital sees only own agent traces) | Inherits Asgard X-Tenant-Id pattern. |
| OBS-5 | LoRA adapter A/B in prod (route 5-10% to new adapter, judge online, auto-rollback if regression) | Coupled with B-35 promotion workflow. |
| **OBS-6** | **Laminar trace API → Mimir `/analytics/llm`** | Per Asgard observability stack ([`Asgard/docs/architecture/observability_stack.md`](../../Asgard/docs/architecture/observability_stack.md)): LLM spans flow OTel → Laminar (lmnr-ai/lmnr) for production traces. This backlog item adds a Mimir-side **adapter + analytics endpoint** that pulls Laminar trace data and surfaces it inside Mimir Dashboard alongside HBp scores. Decouples eval/research view (Mimir) from prod ops view (Laminar) so a clinician + ML eng see the same `agent × model × tenant × time` lens. **Scope:** (a) `GET /api/v1/analytics/llm` route in `ro-ai-bridge` returning paginated rows joined from Laminar API + local `eval_scores`; (b) Laminar API client (`lmnr-ai/lmnr` REST + project key auth); (c) Tokio scheduled poller (default 60s) that materializes a rolling 24h window into `llm_trace_cache` table for fast dashboard queries; (d) Mimir Dashboard `/analytics/llm` page with filters (agent, model, tenant, latency band, error class). **Endpoint URL (intended):** `https://mimir.asgard.internal/analytics/llm`. **Dependencies:** OBS-1 deployment of Laminar itself, OBS-4 multi-tenant trace isolation. |

### Re-evaluation criteria for Laminar at this decision point

When Sprint 50+ triggers fire, re-check:

1. **Laminar ecosystem maturity** — still active maintenance? Stars trajectory? YC-stage funding stable?
2. **Service availability track record** — outage history in 12-month window. (lmnr.ai cloud was inaccessible 2026-05-06; one data point only.)
3. **Self-host operational cost** — Postgres + ClickHouse + RabbitMQ stack. Compare to Mimir-extend cost.
4. **Rust-first compatibility** — has Laminar's Rust core remained primary, or pivoted to Python?
5. **Asgard tenancy fit** — does Laminar's multi-project model align with our X-Tenant-Id JWT pattern?

Decisions deferred until Sprint 50+ trigger; revisit ADR will then be filed as ADR-003.

---

## ADR-002: MLOps Tracking — Mimir-Extend vs MLflow

**Status:** Accepted
**Date:** 2026-05-06
**Context:** Sprint 39 Phase 2 (LoRA training) needs experiment tracking — hyperparameters, loss curves, eval metrics, adapter lineage, promotion status. Volume small (~5-20 runs/sprint, ~50-100/year). Single ML engineer, local MLX training.

**Decision:** Extend Mimir with `lora_training_runs` table + `ai_models` lineage cols. Track via Python wrapper around `mlx_lm.lora` that POSTs to Mimir REST API.

**Options considered:**
- **A. MLflow self-hosted** — proven, 22k stars, but Postgres + S3 backend + 2nd dashboard for ML eng to learn. ~3 days deploy + bridge code.
- **B. opsml (Rust)** — 35 stars, proprietary EULA, tiny adoption — risky.
- **C. Mimir extension (chosen)** — 5-6 days dev, schema parallels existing `eval_runs`, single-pane UX, full lineage native to Mimir DB.
- **D. Aim** — lighter than MLflow but adds same Python service burden.

**Why C:** same logic as ADR-001 — Sprint 39 scope is too small to justify 3rd-party MLOps service complexity. ~50-100 runs/year does not need enterprise MLOps. Revisit if volume grows.

**Revisit triggers:**
1. >50 training runs/quarter sustained
2. Multiple ML engineers comparing experiments
3. Cross-tenant LoRA fleet management (each customer's adapters need lifecycle UI)
4. FDA/PDPA audit requires MLflow-style standard lineage records

References: see `Asgard/docs/architecture/ADR-001-Training-Data-Curator-Build-vs-Buy.md` for the build-vs-buy precedent.

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

**Sprint 43 B-61 + B-62 + B-63** — Round 5 (Qwen-3-32B-Medical-Reasoning) failed
6.6% HBp with 4 unsafe answers. Before Sprint 39 LoRA work eats 2-3 weeks, run
the **MedGemma 27B challenge** (≤2 days end-to-end):

1. **B-61** — Build the 20-prompt safety screen (~2h). Reusable for every future
   model swap. Should have existed before Round 5.
2. **B-62** — Pull `mlx-community/medgemma-27b-text-it-4bit` + register in
   Heimdall (~30min download + 10min DB row).
3. **B-63** — Round 6 eval against same locked items as Round 5 (~25min wall
   clock at 47s/item × 20 = ~16min eval + judge time).

Total time-to-signal: < 1 working day. If MedGemma 27B beats 47.8% at safety=1.0,
new champion → unblocks Sprint 38f/40f n=100 scale work. If it doesn't, LoRA
(Sprint 39) is the next swing — but at least we ruled out the cheaper option
first.

**Why not LoRA first?** Sprint 39 needs ~2 weeks (data prep + train + adapter
merge). MedGemma 27B is 1 day. Cheap option must run first.

---

### Historical recommendation (pre-Round-5)

**Sprint 36 B-16** — wire the existing `cross_encoder_rerank` into Eir's chat
path. Code is already written; estimate 1 day. Expected immediate +3-5pp on Acc
and Rel for free. Verify on locked n=20, decide if it's enough signal to
greenlight Sprint 36 vs jump to Sprint 37 self-consistency for bigger lift.
