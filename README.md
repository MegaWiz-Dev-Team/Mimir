# 🏺 Project Mimir

> **Mimir (มิเมียร์):** ในตำนานนอร์ส Mimir คือเทพแห่งความรู้และสติปัญญา ผู้รักษาบ่อน้ำศักดิ์สิทธิ์ (Mímisbrunnr)
> โครงการนี้จึงมีเป้าหมายเพื่อสร้าง **"บ่อน้ำแห่งความรู้"** — AI Core Platform ที่ใช้ได้กับทุกธุรกิจ

**Project Mimir** คือแพลตฟอร์ม AI แบบ Multi-Tenant ที่ครอบคลุมตั้งแต่ Data Ingestion → RAG Pipeline → Knowledge Graph → Multi-Agent → Model Training โดยเริ่มพัฒนาจากระบบ NPC อัจฉริยะสำหรับ Ragnarok Online แล้วขยายเป็น Domain-Agnostic AI Platform

---

## ✨ Features

### ✅ Implemented (Sprint 1-8)
- 🔐 **Multi-Tenant IAM** — JWT auth, Argon2id password, role-based access
- 📊 **Admin Dashboard** — Tenant switcher, user management, settings
- 📥 **Unified Data Ingress** — File upload (PDF/CSV/XLSX/HTML), web scraper, MCP connector
- 📁 **Smart Upload** — Auto-detect source type from file extension
- 🗄️ **Dual-mode Tabular Import** — Markdown preview or SQL table creation
- 🧪 **Quality Control** — LLM data clustering, conflict resolution Kanban
- 🎯 **Agent Evaluations** — LLM-as-a-judge, heatmap scoring, human override
- 🧭 **Pipeline Traceability** — Source → Vector → Answer end-to-end tracking
- 🎮 **NPC Playground** — Tier 1 (simple chat) & Tier 2 (RAG) with streaming
- 📡 **Real-time Monitoring** — WebSocket/SSE streaming logs

### 🚧 Roadmap (Sprint 9-16)
- 🔧 **Real Extraction Pipeline** — PDF/CSV/HTML extraction, configurable chunking
- 🧠 **Embedding Service** — Multi-model (Ollama/Gemini/Qwen), pipeline lock
- 🕸️ **Knowledge Graph** — Neo4j, LLM entity extraction, Sigma.js visualization
- 🔍 **Hybrid Search** — Vector + Graph + SQL → merged context
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
| **Frontend**         | Next.js 14 + TailwindCSS + shadcn/ui                |
| **Database**         | MariaDB (relational)                                |
| **Vector DB**        | Qdrant (semantic search)                            |
| **Graph DB**         | Neo4j Community (knowledge graph) — *Sprint 11*     |
| **Graph Viz**        | Sigma.js + graphology (WebGL) — *Sprint 11*         |
| **LLM Gateway**      | [Heimdall](https://github.com/megacare-dev/mega-llm-server) (Rust) |
| **Embedding Models** | BAAI/bge-m3 (MLX native)                            |
| **Infrastructure**   | Docker Compose + Colima                             |
| **Hardware Target**  | Mac Mini M4 Pro (64GB)                              |

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
├── ro-ai-bridge/              # 🦀 Rust AI Backend (Axum) → :3000
│   ├── src/
│   │   ├── routes/            # API endpoints (auth, sources, eval, chat...)
│   │   └── services/          # Business logic (ingress, extraction, sql_import...)
│   ├── Cargo.toml
│   └── .env
├── ro-ai-dashboard/           # ⚛️ Next.js Admin Dashboard → :3001
│   ├── src/app/               # Pages (sources, playground, settings...)
│   ├── src/components/        # UI components (shadcn/ui based)
│   └── src/lib/               # API client, utils
├── docs/
│   ├── iso_29110/             # 📋 ISO 29110 Documents
│   │   ├── pm/                # PM-01 Project Plan, PM-02 Sprint Reports
│   │   └── si/                # SI-01 SRS, SI-02 SDD, SI-03 Traceability, SI-04 Tests
│   └── ...
├── tests/                     # 🧪 Integration tests
├── docker-compose.yml         # 🐳 Infrastructure services
└── README.md
```

---

## 🏰 Asgard Port Assignments

> Full port map: [Asgard Port Allocation](https://github.com/megacare-dev/Asgard/blob/main/docs/technical/port-allocation-startup.md)

| Port | Service | Description |
|------|---------|-------------|
| `3000` | **Mimir API** | Rust backend |
| `3001` | **Dashboard** | Next.js frontend |
| `3306` | MariaDB | Database |
| `6333` | Qdrant | Vector search |
| `6379` | Redis | Cache |
| `7474` | Neo4j | Graph DB |
| `8201` | Vault | Secrets |
| `9000` | RustFS | Object storage |

## 🚀 Quick Start

### Prerequisites
- Docker & Docker Compose (via [Colima](https://github.com/abiosoft/colima) on macOS)
- Rust (1.75+)
- Node.js (18+)
- [Heimdall](https://github.com/megacare-dev/mega-llm-server) (LLM Gateway)

### 1. Start Infrastructure
```bash
# Using deploy script (recommended)
./scripts/deploy.sh --prod

# Or manually:
docker compose up -d
```

### 2. Start Heimdall (LLM Gateway)
```bash
cd ~/Documents/Heimdall
./scripts/start.sh
# Gateway: http://localhost:8080
```

### 3. Start AI Backend
```bash
cd ro-ai-bridge
cp .env.example .env
./target/release/ro-ai-bridge    # http://localhost:3000
```

### 4. Start Dashboard
```bash
cd ro-ai-dashboard
npm install
npm run dev                      # http://localhost:3001
```

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
| 9      | Real Pipeline & Navigation                 | 📋 Planned |
| 10     | Embedding & Vector Store                   | 📋 Planned |
| 11     | Knowledge Graph & GraphRAG                 | 📋 Planned |
| 12     | Multi-Agent & Coverage Intelligence        | 📋 Planned |
| 13     | AI Agent Studio                            | 📋 Planned |
| 14     | Production Ready                           | 📋 Planned |
| 15     | Dataset Studio                             | 📋 Planned |
| 16     | Training Integration                       | 📋 Planned |

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
