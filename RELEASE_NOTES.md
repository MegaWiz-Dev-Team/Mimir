# Release Notes — Mimir

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
