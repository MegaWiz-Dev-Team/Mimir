# Release Notes — Mimir

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
