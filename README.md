# 🏺 Project Mimir

> **Mimir (มิเมียร์):** ในตำนานนอร์ส Mimir คือเทพแห่งความรู้และสติปัญญา ผู้รักษาบ่อน้ำศักดิ์สิทธิ์ (Mímisbrunnr)
> โครงการนี้จึงมีเป้าหมายเพื่อสร้าง **"บ่อน้ำแห่งความรู้"** — AI Core Platform ที่ใช้ได้กับทุกธุรกิจ

**Project Mimir** คือแพลตฟอร์ม AI แบบ Multi-Tenant ที่ครอบคลุมตั้งแต่ Data Ingestion → RAG Pipeline → Knowledge Graph → Multi-Agent → Model Training โดยเริ่มพัฒนาจากระบบ NPC อัจฉริยะสำหรับ Ragnarok Online แล้วขยายเป็น Domain-Agnostic AI Platform

---

## ✨ Features

### ✅ Implemented (Sprint 1-17)
- 🔐 **Multi-Tenant IAM** — JWT auth, Argon2id password, RBAC with dynamic custom roles
- 📊 **Admin Dashboard** — Tenant switcher, user management, 5-tab settings (General / AI Models / Pipeline / Search / Security)
- 📥 **Unified Data Ingress** — File upload (PDF/CSV/XLSX/HTML), web scraper, MCP connector
- 📁 **Smart Upload** — Auto-detect source type from file extension
- 🗄️ **Dual-mode Tabular Import** — Markdown preview or SQL table creation
- 🧪 **Quality Control** — LLM data clustering, conflict resolution Kanban, iteration guards
- 🎯 **Agent Evaluations** — LLM-as-a-judge, heatmap scoring, human override
- 🧭 **Pipeline Traceability** — Source → Vector → Answer end-to-end tracking with step-level status
- 🎮 **NPC Playground** — Tier 1 (simple chat) & Tier 2 (RAG) with streaming
- 📡 **Real-time Monitoring** — WebSocket/SSE streaming logs
- 🧠 **Dynamic LLM Routing** — Per-tenant slot configuration (chat/rag/judge/embedding) with multi-provider support
- 🔑 **Vault-First Security** — HashiCorp Vault integration for tenant-specific API keys (Vault → ENV fallback)
- 🕸️ **Knowledge Graph** — Neo4j entity extraction, graph visualization, path finding
- 🔍 **Hybrid Search** — Vector + Graph + SQL → merged context with configurable search modes
- ☸️ **K3s Deployment** — Automated build & deploy script for Kubernetes (OrbStack/K3s)

### 🚧 Roadmap
- 🤖 **Multi-Agent System** — Router Agent, Tool Registry, Synthesis Agent
- 📈 **Coverage Intelligence** — ACU per source, blind-spot detection, closed-loop
- 🏭 **AI Agent Studio** — No-code visual builder, templates, deploy API/widget
- 📦 **Dataset Studio** — Export curated data as Alpaca/ShareGPT/DPO for fine-tuning
- 🎓 **Training Integration** — Axolotl/Unsloth, MLflow tracking, Model Registry

---

## 🛠 Tech Stack

| Layer                | Technology                                          |
| -------------------- | --------------------------------------------------- |
| **Backend**          | Rust 🦀 (Axum + [Rig.rs](https://rig.rs))            |
| **Frontend**         | Next.js 16 + TailwindCSS + shadcn/ui                |
| **Database**         | MariaDB (relational)                                |
| **Vector DB**        | Qdrant (semantic search)                            |
| **Graph DB**         | Neo4j Community (knowledge graph)                   |
| **Graph Viz**        | Sigma.js + graphology (WebGL)                       |
| **LLM Gateway**      | Heimdall (self-hosted MLX/GGUF) + Ollama (local)    |
| **LLM Cloud**        | Google Gemini, OpenAI, Azure OpenAI                 |
| **Embedding Models** | BGE-M3 (Heimdall 1024d), nomic-embed-text, text-embedding-004 |
| **Secrets**          | HashiCorp Vault (KV v2)                             |
| **Game Server**      | [rAthena](https://github.com/rathena/rathena) (C++) |
| **Infrastructure**   | K3s (OrbStack) / Docker Compose                     |
| **Hardware Target**  | Mac Air M3 / Mac mini M4 Pro                        |

---

## 🏗 Architecture

```
┌─────────────────────┐     ┌──────────────────────────────────────────────┐
│  ro-ai-dashboard    │     │  ro-ai-bridge (Rust)                         │
│  (Next.js)          │────▶│                                              │
│                     │ API │  ┌─────────┐  ┌──────────┐  ┌────────────┐  │
│  • Overview         │     │  │ Ingress │→│ Extract  │→│ Chunk      │  │
│  • Sources          │     │  │ Service │  │ Worker   │  │ Service    │  │
│  • Knowledge        │     │  └─────────┘  └──────────┘  └─────┬──────┘  │
│  • Quality Control  │     │                                    │         │
│  • Playground       │     │              ┌─────────────────────┼───┐     │
│  • Coverage         │     │              ▼                     ▼   │     │
│  • Agent Studio     │     │  ┌──────────────┐  ┌──────────────┐   │     │
│  • Admin Settings   │     │  │ Embed → Qdrant│  │ Entity→Neo4j │   │     │
└─────────────────────┘     │  └──────────────┘  └──────────────┘   │     │
                            │              │                     │   │     │
┌─────────────────────┐     │              ▼                     ▼   │     │
│  rathena            │     │  ┌─────────────────────────────────┐   │     │
│  (C++ Game Server)  │────▶│  │ Router Agent → Tool Registry   │   │     │
│  Login/Char/Map     │ NPC │  │ ├─ Vector Agent (Qdrant)       │   │     │
└─────────────────────┘     │  │ ├─ SQL Agent    (MariaDB)      │   │     │
                            │  │ └─ Graph Agent  (Neo4j)        │   │     │
                            │  │ → Synthesis Agent → Answer     │   │     │
                            │  └─────────────────────────────────┘   │     │
                            └──────────────────────────────────────────────┘
```

### Agent Tiers
| Tier       | Use Case              | Latency | Data Source          |
| ---------- | --------------------- | ------- | -------------------- |
| **Tier 1** | NPC Chat (simple)     | ≤2s     | LLM only             |
| **Tier 2** | RAG Agent (knowledge) | ≤5s     | Qdrant + Neo4j + SQL |
| **Tier 3** | Background Agent      | Async   | Server analytics     |

---

## 📂 Project Structure

```
Project-Mimir/
├── ro-ai-bridge/              # 🦀 Rust AI Backend (Axum)
│   ├── src/
│   │   ├── routes/            # API endpoints (auth, sources, eval, chat...)
│   │   └── services/          # Business logic (ingress, vault, llm_router...)
│   ├── mimir-core-ai/
│   │   ├── src/models/        # Data models (iam, pipeline, sources...)
│   │   ├── src/services/      # Core services (db, iam, vault, llm_router...)
│   │   └── migrations/        # SQLx database migrations
│   ├── Dockerfile             # Multi-stage Docker build
│   └── Cargo.toml
├── ro-ai-dashboard/           # ⚛️ Next.js Admin Dashboard
│   ├── src/app/               # Pages (sources, playground, settings...)
│   ├── src/components/        # UI components (shadcn/ui based)
│   ├── src/lib/               # API client, utils
│   └── Dockerfile             # Multi-stage Docker build
├── rathena/                   # 🎮 rAthena Game Server (C++)
├── scripts/
│   ├── deploy.sh              # Local development (docker-compose)
│   └── k3s-deploy.sh          # K3s production deploy (build + rollout)
├── docs/
│   ├── deployment/            # Deployment guides
│   ├── iso_29110/             # ISO 29110 compliance documents
│   └── INDEX.md               # Full documentation index
├── docker-compose.yml         # 🐳 Dev stack (MariaDB + Qdrant + Vault + ...)
├── k8s/                       # ☸️ Kubernetes manifests (K3s/OrbStack)
└── README.md
```

---

## 🚀 Quick Start

### Prerequisites
- Docker & Docker Compose
- Rust (1.85+)
- Node.js (22+)
- Ollama or Heimdall (for local LLM)

### Option A: Local Development (Docker Compose)

```bash
# 1. Start infrastructure
docker compose up -d
# MariaDB, Qdrant, Redis, Vault, Neo4j, MinIO

# 2. Start AI Backend
cd ro-ai-bridge
cp .env.example .env        # Configure DB, Qdrant, LLM settings
cargo run

# 3. Start Dashboard
cd ro-ai-dashboard
npm install
npm run dev                  # http://localhost:3000
```

Or use the automated script:
```bash
./scripts/deploy.sh --dev
```

### Option B: K3s Deployment (OrbStack)

For production-like deployment on K3s (OrbStack):

```bash
# Deploy everything (build + rollout)
./scripts/k3s-deploy.sh all

# Or deploy individually
./scripts/k3s-deploy.sh api
./scripts/k3s-deploy.sh dashboard

# Override API URL for non-localhost access
NEXT_PUBLIC_API_URL=http://192.168.x.x:30000/api ./scripts/k3s-deploy.sh dashboard
```

**K3s Service Ports:**

| Service         | NodePort | URL                      |
| --------------- | -------- | ------------------------ |
| Mimir API       | 30000    | http://localhost:30000   |
| Mimir Dashboard | 30001    | http://localhost:30001   |
| Yggdrasil (SSO) | 30085    | http://localhost:30085   |
| Bifrost         | 30100    | http://localhost:30100   |
| Fenrir          | 30200    | http://localhost:30200   |

---

## 📋 Sprint Progress

| Sprint | Theme                                      | Status    |
| ------ | ------------------------------------------ | --------- |
| 1      | Security Foundation & IAM                  | ✅ Done    |
| 2      | Data Isolation & Vector Management         | ✅ Done    |
| 3      | Tenant Configuration & Provisioning        | ✅ Done    |
| 4      | Quality Control & Hallucination Prevention | ✅ Done    |
| 5      | Data Ingress Monitoring                    | ✅ Done    |
| 6      | Agent Evaluations System                   | ✅ Done    |
| 7      | UX/UI Pipeline & Traceability              | ✅ Done    |
| 8      | Unified Data Ingress & File Upload         | ✅ Done    |
| 9      | Real Extraction Pipeline                   | ✅ Done    |
| 10     | Embedding & Vector Store                   | ✅ Done    |
| 11     | Knowledge Graph & GraphRAG                 | ✅ Done    |
| 12     | Hybrid Search & Retrieval                  | ✅ Done    |
| 13     | Asgard K3s Platform & SSO                  | ✅ Done    |
| 14     | Dynamic LLM Routing                        | ✅ Done    |
| 15     | Medical RAG (Eir/OpenEMR)                  | ✅ Done    |
| 16     | Pipeline Orchestration & Step Tracking     | ✅ Done    |
| 17     | Vault-First Security & Settings UX         | ✅ Done    |
| 18     | Multi-Agent System                         | 📋 Planned |
| 19     | AI Agent Studio                            | 📋 Planned |
| 20     | Dataset Studio & Training                  | 📋 Planned |

---

## 📊 Presentation

- **[Sales Deck: Project Mimir - AI-Native Evolution](https://docs.google.com/presentation/d/18Y9XRoT494pGA0wvd6oSRKrkrX8f9205RjT4a7R4nn8/edit?slide=id.SLIDES_API1144777460_45#slide=id.SLIDES_API1144777460_45)**

## 📝 ISO 29110 Compliance

โครงการนี้ปฏิบัติตามมาตรฐาน **ISO/IEC 29110** (Software Engineering for Very Small Entities):

| Document | Description                                          |
| -------- | ---------------------------------------------------- |
| PM-01    | Project Plan (Sprint 1-16 roadmap)                   |
| PM-02.x  | Sprint Reports (per sprint)                          |
| SI-01    | Software Requirements Specification (REQ-001~016)    |
| SI-02    | Software Design Document (architecture + subsystems) |
| SI-03    | Traceability Matrix (requirements ↔ issues ↔ tests)  |
| SI-04.x  | Test Scripts (per sprint)                            |
| SI-05    | User Manual                                          |

---

*Created with ❤️ by Mega Wiz for the Ragnarok Online community and beyond.*
