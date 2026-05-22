# RO-AI Bridge — Mimir RAG Intelligence Engine

A high-performance Rust backend for **Project Mimir** — the Asgard AI Platform. This engine provides a comprehensive medical RAG (Retrieval-Augmented Generation) pipeline with multi-source search, knowledge graph integration, autonomous evaluation, and agent orchestration.

## 🏗 Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    ro-ai-bridge v0.30.0                  │
├──────────┬──────────┬──────────┬────────────────────────┤
│  Search  │  Agents  │   RAG    │      Evaluation        │
│  Engine  │  Studio  │ Pipeline │      Framework          │
├──────────┴──────────┴──────────┴────────────────────────┤
│           mimir-core-ai (LlmRouter / IAM)               │
├─────────────────────────────────────────────────────────┤
│  Heimdall LLM Gateway  │  Qdrant  │  MariaDB  │  MCP   │
└─────────────────────────────────────────────────────────┘
```

## 🚀 Core Features

### 🔍 Ensemble Search Engine
- **3-Source Parallel Search**: Vector (Qdrant), Knowledge Graph (SQL), and Tree (PageIndex) fired concurrently via `tokio::join!`
- **Configurable Ensemble Weights**: Per-agent vector/tree/graph weight ratios with real-time slider control
- **Cross-Encoder Re-ranking**: ML-based relevance scoring using `BAAI/bge-reranker-v2-m3` via Heimdall TEI endpoint
- **Graceful Fallback**: Cross-Encoder → RRF (Reciprocal Rank Fusion) → weighted linear interpolation
- **Source-Level Filtering**: Narrow search scope by `source_id`, `source_type`, or `date_range`

### 🤖 Agent Studio
- **Per-Agent RAG Configuration**: Each agent stores its own weights, top_k, alpha, threshold, graph hops, and rerank settings as JSON columns
- **Multi-Provider LLM Routing**: Dynamic slot resolution across Heimdall, Ollama, Gemini, OpenAI, Azure, and Flash-MoE
- **Swarm Orchestration**: Multi-agent handoff patterns with Skill & Soul architecture

### 📊 RAG Evaluation Framework
- **Comprehensive Metrics**: Hit Rate@K, MRR, NDCG@K, Precision@K, Recall@K, per-query latency
- **LLM-as-a-Judge**: Faithfulness, Answer Relevancy, and Context Precision scoring (0-10 scale)
- **Per-Source Contribution Tracking**: Tracks which retrieval source (V/T/G) contributed to each hit
- **AI Evaluation Set Generator**: Source-grounded question generation with multi-turn conversation support
- **Visual Comparison Matrix**: Select up to 3 runs for side-by-side scoring comparison with 🏆 best highlighting

### 🪄 Autonomous Auto-Tuner
- **Overseer LLM Agent**: Iteratively analyzes failure modes and suggests optimized weights
- **Background Job System**: `tokio::spawn` worker with progress tracking via polling API
- **Configurable Target Metric**: Optimize for NDCG, MRR, Hit Rate, or custom metrics
- **Deploy-to-Agent**: One-click deployment of winning configuration to any agent

### 🌐 Data Pipeline
- **Multi-Source Ingestion**: Web scraping, file upload, MCP connectors
- **Knowledge Extraction**: Automated Q/A generation + Knowledge Graph entity/relation extraction
- **Vector Indexing**: Heimdall-standard 1024d embeddings via `BAAI/bge-m3`
- **PageIndex Tree**: Native Rust hierarchical Markdown parser for tree-based retrieval

## 🛠 Prerequisites

1. **Rust**: Latest stable toolchain
2. **Database**: MariaDB/MySQL with migrations applied
3. **Vector DB**: Qdrant (collections: `source_chunks`, `golden_qa`)
4. **LLM Gateway**: Heimdall server (for embeddings, generation, and reranking)

## ⚙️ Configuration (`.env`)

```bash
DATABASE_URL=mysql://mimir:password@localhost:3306/mimir
HEIMDALL_API_URL=http://localhost:8000/v1
HEIMDALL_API_KEY=your-key
QDRANT_URL=http://localhost:6333
MONITOR_PORT=8080
```

## 📖 Quick Start

```bash
# 1. Apply database migrations
for f in migrations/*.sql; do mysql -u root mimir < "$f"; done

# 2. Start the backend
cargo run --bin monitor

# 3. Start the dashboard (separate terminal)
cd ../ro-ai-dashboard && npm run dev
```

## 📡 API Endpoints (Key)

### Search
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/search` | Parallel ensemble search with optional cross-encoder reranking |

### Agents
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/agents` | List agent configs |
| `POST` | `/api/v1/agents` | Create agent with RAG params |
| `POST` | `/api/v1/agents/:id/chat` | Chat with ensemble RAG context injection |

### RAG Evaluation
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/rag-eval/run` | Execute full retrieval + generation evaluation |
| `GET` | `/api/v1/rag-eval/runs` | List evaluation runs for comparison |
| `GET` | `/api/v1/rag-eval/runs/:id` | Drill-down with per-query analysis |
| `POST` | `/api/v1/rag-eval/runs/:id/deploy` | Deploy winning config to agent |
| `POST` | `/api/v1/rag-eval/generate-set` | AI-generate evaluation dataset |
| `POST` | `/api/v1/rag-eval/auto-tune` | Start autonomous weight optimization |
| `GET` | `/api/v1/rag-eval/auto-tune/:id` | Poll auto-tuning job progress |

### OCR Layout Evaluation
Region-detection eval (mAP / parity / GriTS) ingested from Syn. Scoped by the
`X-Tenant-Id` header (default `asgard_platform`); non-synthetic runs are
hash-only (PII guard). See [docs/04_evaluation_and_testing/ocr_layout_eval_runbook.md](../docs/04_evaluation_and_testing/ocr_layout_eval_runbook.md).

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/eval/ocr/layout/runs` | Ingest a layout-eval run + per-image items |
| `GET` | `/api/v1/eval/ocr/layout/runs` | List runs (filter `eval_kind`, `syn_version`, `dataset_name`) |
| `GET` | `/api/v1/eval/ocr/layout/runs/:id` | Run detail + per-image items |

## 📁 Project Structure

```
ro-ai-bridge/
├── src/
│   ├── routes/
│   │   ├── search.rs           # Parallel ensemble search
│   │   ├── agents/             # Agent CRUD + Chat
│   │   ├── rag_eval.rs         # RAG evaluation framework
│   │   ├── rag_eval_tuner.rs   # Auto-tuner (Overseer LLM)
│   │   └── evaluations_ext.rs  # Model evaluation + scorecard
│   ├── retrieval/
│   │   ├── ensemble.rs         # Weights + Cross-Encoder reranking
│   │   ├── qdrant.rs           # Vector search
│   │   ├── graph.rs            # Knowledge Graph search
│   │   └── tree.rs             # PageIndex tree search
│   └── swarm/                  # Multi-agent orchestration
├── mimir-core-ai/              # Core: LlmRouter, IAM, DB
├── migrations/                 # SQL schema migrations
└── docs/                       # Security and architecture docs
```

## 🧪 Testing

```bash
# Run all unit tests (requires SQLX_OFFLINE=true if DB not running)
SQLX_OFFLINE=true cargo test --lib

# Run specific test suites
SQLX_OFFLINE=true cargo test --lib retrieval::ensemble
SQLX_OFFLINE=true cargo test --lib routes::search
SQLX_OFFLINE=true cargo test --lib routes::rag_eval
```

---
*Part of the Asgard AI Platform — Project Mimir · v0.30.0*
