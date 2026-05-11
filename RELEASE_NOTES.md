# Release Notes — Mimir

## v1.3.0 — Sprint 50 OCR Lane A (2026-05-11)

> 4-tier OCR foundation lands in Mimir + Dashboard. Image OCR is now a first-class request type with audit, smart-router delegation to Syn, per-tenant cost guard, and clinician-facing upload UI. Cloud tiers remain locked behind PHI strict + Sprint 50b Skuggi PII guardrail.

### 🩺 ro-ai-bridge (1.2.0 → 1.3.0)

- **`ocr_documents` audit table + writer** (B-50e, #264) — every OCR call now writes a row with engine_used, cost_usd, confidence, status, image_sha256 fingerprint, optional Skuggi PII cross-link
- **Path A smart-router delegation** (B-50b, #265) — `/api/v1/ocr/extract` now delegates to Syn's 6-rule router instead of going direct to Gemini; cross-system audit (Syn's row + Mimir's row) preserved
- **Cost guard middleware** (B-50m, #266) — pre-call USD estimate + monthly budget cap enforcement; PHI strict hard-blocks cloud regardless of budget. 402/403 paths fully audited.
- **Admin endpoint** `/api/v1/ocr/admin/policy` (B-50l, #266) — GET/PATCH for tenant cloud opt-in flags + monthly budget cap (validates Pro requires Flash per ADR-006)
- **Recent calls endpoint** `/api/v1/ocr/admin/recent` (#268) — SQL-backed view of `ocr_documents` for dashboard table
- **`ocr_extract` tool allowlist** (B-50g, #269) — added to all 5 clinical Eir variants (`eir` + `eir-cardio/sleep/ent/pediatrics`); router agent excluded
- **Insurance pre-staging** (B-50g+, #269) — sprint52 tool snippet pre-documented in `migrations/notes/insurance_tools.md` so Sprint 52 INS-03 doesn't re-discover the spec

### 🖥️ ro-ai-dashboard (1.2.0 → 1.3.0)

- **OCR Cost Guard tab** in `/analytics/llm` (#267) — monthly spend card with colored progress bar, policy toggles (PHI strict + Tier 2 Flash + Tier 3 Pro with cascading disables), budget input + save, PII mode badge, Laminar (Sága) trace dashboard link-out
- **Recent OCR Calls table** (#268) — per-tenant SQL-backed table with status badges (red on budget_exceeded / pii_strict_block), engine + router_reason, cost, latency, confidence, PII redaction shield, per-row Laminar deep link
- **`/playground` OCR upload** (B-50i, #270) — paperclip button → image/PDF picker → editable text preview with engine/confidence/cost badges → marker-block prepend on Send so the LLM tells document content from typed words; clinicians can fix Thai OCR errors before sending

### 🗺️ Docs

- **Sprint 50 plan** (`03_14_Local_LLM_Optimization_Sprints.md`) — Lane A items marked done with PR references; B-50h.1 (clinician data) remains the open gate for full sprint acceptance
- **B-50g+ note** — `migrations/notes/insurance_tools.md` stages Sprint 52 insurance agent tool seed snippet

### 🔒 Cloud tier remains gated

`B-50k` (Gemini 3 Flash/Pro adapter) is implementable but PHI strict still hard-blocks cloud calls. Cloud OCR unlocks when Sprint 50b Skuggi PII guardrail ships and tenant flips `pii_egress_policy` per the insurance Sprint 54 gate.

### 📊 Open PRs (stacked review train)

| PR | Title |
|----|-------|
| #264 | B-50e audit writer (root of stack) |
| #265 | B-50b Path A delegation |
| #266 | B-50m cost guard + admin endpoint |
| #267 | Dashboard OCR Cost Guard tab |
| #268 | `/ocr/admin/recent` + Recent OCR Calls table |
| #269 | B-50g Eir tool allowlist (+ B-50g+ insurance note) |
| #270 | B-50i `/playground` drag-drop upload |

---

## v1.1.0 — Pipeline Precision & ETA Upgrade (2026-04-10)

### ✨ New Features & Fixes
- **High-Precision Per-Step ETA Engine:** Completely overhauled the ETA math in `auto_pipeline.rs` to group by `step_name`, resulting in hyper-accurate predictions (e.g. tracking PageIndex generation independent of quick ML embedding operations).
- **Step Breakdown UI:** Dashboard now dynamically renders `(~ XXm)` estimates on individual pipeline steps natively in React.
- **QA Extraction Data Loss Fix:** Resolved a critical silent database failure where generated LLM QA Pairs were discarded due to an unbound `chunk_id` SQL parameter in Step 5.
- **Dynamic Gateway Routing:** Standardized the `LlmRouter` architecture to eliminate legacy hardcoded LLM variables across backend services.
- **FastEmbed Isolation:** Fixed Git repository bloat by silencing local model weight caches.


## v1.0.0 — Enterprise RAG & Agentic Release (2026-04-07)

### ✨ New Features (Sprints 31-35)
- **Agentic RAG (Swarm):** Introduced the Overseer Metacontroller enabling multi-step, multi-agent (Skill & Soul) operations directly from the Dashboard.
- **Auto-Tuner:** Automated Hyperparameter Optimization via Genetic Algorithms for optimal vector vs. tree weights and thresholding.
- **Cross-Encoder Re-Ranking:** Highly accurate pairwise re-ranking available in the RAG Playground as a `🚀` strategy option.
- **Flash-MoE Inference:** Qwen3.5-397B ultra-large model operational at 1.5 TPS via new `LlmRouter` integration.
- **Graph Intelligence:** Native Rust integration to analyze God Nodes and Surprising Connections via complex graph SQL computation, powered by the `graph_analyzer` routing slot in Step 6 Auto-Pipeline.
- **2-Hop Knowledge Graph:** Complex query relationship awareness via deep `UNION ALL` SQL traversal.

### 🐛 Fixes & Polish
- **Hybrid Embedding Architecture:** Replaced unstable Python MLX sidecar with native Rust ONNX (Port 8080) for robust background `Auto Pipeline` generation, and integrated `llama.cpp` (Port 8089) for 15ms zero-latency chat retrieval queries. 
- 100% Pass Rate on `test-deploy.sh` (28/28 E2E Scenarios).
- Fixed database deadlocks for the Swarm `ON CONFLICT` JSON checkpoint saves.
- Fully extracted React components with unified Dashboard toggles.
- Sweeps effectively clean up "zombie" `running` pipelines upon service startup via `auto_pipeline.rs`.

---

## v0.29.0 — Sprint 29: Docker Build & Compose (2026-03-13)

### 🐳 Infrastructure
- Dockerfile rewritten: Mimir root context, `SQLX_OFFLINE=true`
- Generated `.sqlx/` offline query cache (28 queries)
- Fixed `include_str!` path for openapi.yaml
- Expanded Docker context to repo root
- Added `.dockerignore` at Mimir root
- Integrated into Asgard unified Docker Compose (:3000)

### 📊 Stats
- **255+ tests**, all passing
- Docker build: 120 → 0 errors
- Image size: 204MB

---

## Sprint 28 — Auto-Pipeline & E2E Scorecard (2026-03-11)

### ✨ New Features
- **Auto-Pipeline** — 1-click pipeline run from Data Sources tab
- **E2E Scorecard** — full pipeline evaluation dashboard
- **Pipeline Monitor** — real-time step tracking with live status
- **QC infinite loop fix** — stall detection + iteration limits + ClusteringGuard

### 📊 Stats
- **255+ tests**
- Sprint 28 complete

---

## Sprint 27 — Evaluation Expansion (2026-03-10)

### ✨ New Features
- Extraction evaluation tab
- Retrieval evaluation tab
- Provider comparison dashboard

---

## Sprint 26 — Multi-Provider Extraction (2026-03-10)

### ✨ New Features
- Multi-provider extraction pipeline
- Versioned prompt management system

---

## Sprint 1-25 — Foundation through Code Quality

> 25 sprints of continuous development building the Mimir knowledge platform.

### Key Milestones
| Sprint | Highlight |
|:--|:--|
| 1-7 | Foundation: IAM, Vector, QC, Ingress, Eval, UX |
| 8-10 | Data Ingress, Pipeline, Embedding |
| 11 | Knowledge Graph + GraphRAG |
| 12-13 | Multi-Agent & AI Agent Studio |
| 14-16 | Production Core, Deploy, Dataset Studio |
| 17-18 | Knowledge Graph (31 tests), Coverage Analytics (14 tests) |
| 19-21 | Agent Templates, Custom Roles ACL, QA Status |
| 22-23 | Antigravity Skills, Code Quality (69 tests) |
| 24-25 | Graph API Hotfix, Vector & Chat Fixes |

---

*Asgard เป็นของทุกคนแล้ว — Asgard belongs to everyone.*
