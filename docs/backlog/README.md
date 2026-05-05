# Product Backlog

> Last updated: 2026-05-05 | Sources: E2E Flow Review (Sprint 22), Local-LLM Tournament (Sprint 36-39 plan)

## Backlog Items

| ID   | Title                                           | Sprint | Size | Priority | Status |
| ---- | ----------------------------------------------- | ------ | ---- | -------- | ------ |
| B-01 | Refactor `sources.rs` (61KB) → sub-modules      | 23     | L    | P1       | 🔲 Open |
| B-02 | Extract Settings tabs into separate components  | 23     | M    | P1       | 🔲 Open |
| B-03 | Split `agents.rs` (36KB) → CRUD + Chat          | 23     | M    | P2       | 🔲 Open |
| B-04 | Reorganize Nav groups to match RAG pipeline     | 24     | S    | P1       | 🔲 Open |
| B-05 | Add "Getting Started" onboarding wizard         | 24     | L    | P2       | 🔲 Open |
| B-06 | Add pipeline status breadcrumb                  | 24     | M    | P2       | 🔲 Open |
| B-07 | Simplify Source wizard (3-step → 2-step)        | 24     | M    | P3       | 🔲 Open |
| B-08 | Add SSE Streaming pattern to frontend skill     | 24*    | S    | P1       | 🔲 Open |
| B-09 | Add Multi-Step Wizard pattern to frontend skill | 24*    | S    | P1       | 🔲 Open |
| B-10 | Add Evaluation Matrix pattern to frontend skill | 24*    | S    | P2       | 🔲 Open |
| B-11 | Create Data Pipeline skill                      | 24*    | M    | P2       | 🔲 Open |
| B-12 | Auto-pipeline: Source → Chunk → QA → Vector     | 25     | L    | P1       | 🔲 Open |
| B-13 | Agent evaluation from Playground                | 25     | M    | P2       | 🔲 Open |
| B-14 | Coverage gap detection                          | 25     | M    | P2       | 🔲 Open |
| B-15 | One-click Agent publish → API key → Embed       | 25     | M    | P3       | 🔲 Open |
| B-16 | Wire `cross_encoder_rerank` into Eir chat path  | 36     | M    | P0       | 🔲 Open |
| B-17 | Tune Eir defaults (top_k=16, temp=0.3, max_tok=4096) | 36 | S    | P0       | 🔲 Open |
| B-18 | CoT system prompt for medical reasoning         | 36     | S    | P0       | 🔲 Open |
| B-19 | Add 4 specialty tools (drug_int, icd10, dosage, calc) | 36 | L  | P1       | 🔲 Open |
| B-20 | Register MedCPT ONNX embedding in Heimdall      | 36     | M    | P1       | 🔲 Open |
| B-21 | Gate "rogue swap" callers during tournaments    | 36     | M    | P1       | 🔲 Open |
| B-22 | Self-consistency runner (replicates_per_item)   | 37     | L    | P1       | 🔲 Open |
| B-23 | Query expansion via LLM rewrite                 | 37     | M    | P1       | 🔲 Open |
| B-24 | Multi-judge ensemble (judges: Vec)              | 37     | M    | P1       | 🔲 Open |
| B-25 | Q5_K_M / Q6_K quantization variants             | 37     | S    | P2       | 🔲 Open |
| B-26 | Larger sample n=20 → n=100 for champion claim   | 37     | S    | P1       | 🔲 Open |
| B-27 | Specialty router agent (Cardio/Sleep/Peds/Gen)  | 38     | XL   | P1       | 🔲 Open |
| B-28 | Reasoning loop: think → search → critique → ans | 38     | L    | P2       | 🔲 Open |
| B-29 | Per-specialty KB partitioning                   | 38     | M    | P2       | 🔲 Open |
| B-30 | Per-specialty HBp% tracking in eval framework   | 38     | M    | P2       | 🔲 Open |
| B-31 | Build LoRA training set (Q-A from PMC + KB)     | 39     | L    | P1       | 🔲 Open |
| B-32 | MLX LoRA training pipeline                      | 39     | XL   | P1       | 🔲 Open |
| B-33 | Adapter merge + Heimdall registry               | 39     | L    | P2       | 🔲 Open |
| B-34 | Training metrics dashboard                      | 39     | M    | P2       | 🔲 Open |
| B-35 | LoRA A/B promotion workflow                     | 39     | M    | P2       | 🔲 Open |
| B-36 | Loader: 5 medical benchmarks → benchmark_datasets | 40   | M    | P0       | 🔲 Open |
| B-36a| UI: benchmark column in runs table              | 40     | S    | P0       | 🔲 Open |
| B-36b| UI: benchmark filter dropdown                   | 40     | S    | P0       | 🔲 Open |
| B-36c| UI: per-benchmark Rank/Champion                 | 40     | M    | P0       | 🔲 Open |
| B-36d| UI: rubric-aware metric column                  | 40     | L    | P0       | 🔲 Open |
| B-36e| UI: cross-benchmark compare guard               | 40     | S    | P1       | 🔲 Open |
| B-36f| UI: /benchmarks registry page                   | 40     | M    | P1       | 🔲 Open |
| B-36g| UI: group/section by benchmark toggle           | 40     | S    | P2       | 🔲 Open |
| B-36h| DB: scoring_fn column on benchmark_datasets     | 40     | S    | P0       | 🔲 Open |
| B-43 | Adapt healthbench_eval.py for Eir+Heimdall      | 41     | M    | P1       | 🔲 Open |
| B-44 | gpt-4.1 grader path                             | 41     | S    | P1       | 🔲 Open |
| B-45 | Paper-comparable scoreboard run                 | 41     | M    | P1       | 🔲 Open |
| B-46 | Publish paper-comparable numbers + cross-ref    | 41     | S    | P1       | 🔲 Open |
| B-47 | HF Open Medical-LLM Leaderboard submission      | 41     | M    | P3       | 🔲 Open |
| B-48 | use_specialty_router flag in EvaluatorParams    | 38f    | M    | P0       | 🔲 Open |
| B-49 | Router-vs-Monolithic A/B eval (gates B-52)      | 38f    | S    | P0       | 🔲 Open |
| B-50 | Decide cardio model: gemma vs flash-lite        | 38f    | S    | P1       | 🔲 Open |
| B-51 | PubMedQA underperformance investigation         | 38f    | M    | P1       | 🔲 Open |
| B-52 | Expand 5 → 28 specialties (gated on B-49)       | 38f    | M    | P1       | 🔲 Open |
| B-53 | Tenant onboarding wizard (atomic specialty bundle) | 38f | M    | P1       | 🔲 Open |
| B-54 | UI: /agents/route live demo page                | 38f    | M    | P2       | 🔲 Open |
| B-55 | Per-specialty HBp% tracking + UI breakdown      | 38f    | M    | P2       | 🔲 Open |

> *Sprint 24 skill items can run parallel with any sprint (documentation only)

## Sprint Themes

| Sprint  | Theme          | Key Outcome                         |
| ------- | -------------- | ----------------------------------- |
| **23**  | 🔴 Code Quality | ไฟล์ใหญ่ถูก split ให้ < 500 lines       |
| **24**  | 🟡 UX Flow      | Onboarding 10 นาที, nav ตาม pipeline |
| **24*** | 🟢 Skills       | 100% pattern coverage (parallel)    |
| **25**  | 🔵 Capabilities | One-click pipeline, auto-evaluate   |
| **36**  | 🟢 LLM Quick Wins ✅ done | CoT + tune + per-model rerank, +6-7pp validated |
| **37**  | 🟡 Score Multipliers 🟡 partial | Code deployed, n≥20 validation pending |
| **38**  | 🟠 Architecture 🟢 PoC done | Specialty router LIVE — 5/5 routing accuracy |
| **38f** | 🟠 Router validation + 28-specialty expand 📋 next | A/B eval (B-49 gates B-52), then expand |
| **39**  | 🔵 ML Pipeline | LoRA fine-tune, +15-25 HBp%, persistent learning |
| **40**  | 🟣 Multi-Benchmark UI/DB ✅ done | Benchmark-aware UI, rubric-aware metrics, /benchmarks page |
| **40f** | 🟣 n=100 scale + native MCQ scoring | Stable rank, paper-comparable Acc% |
| **41**  | 🔴 Paper-Comparable | Run HealthBench paper-original 5K + grader, marketing scoreboard |

> Sprint 36-39 details: [03_14_Local_LLM_Optimization_Sprints.md](../03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md)
> Baseline: [04_03_HealthBench_Pro_Baseline_2026-05-04.md](../04_evaluation_and_testing/04_03_HealthBench_Pro_Baseline_2026-05-04.md)

## Size Legend

| Size  | Lines Changed | Duration     |
| ----- | ------------- | ------------ |
| **S** | < 200 lines   | 1-2 sessions |
| **M** | 200-800 lines | 2-4 sessions |
| **L** | 800+ lines    | 4+ sessions  |
