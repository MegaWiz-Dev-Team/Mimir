# Changelog

All notable changes to the RO-AI Bridge are documented in this file.

## [0.30.0] — 2026-04-03 (Sprint 35)

### ✨ Features

#### RAG Evaluation Framework (Phase 5-6)
- **Full Evaluation System** (`rag_eval.rs`): NDCG@K, Precision@K, Recall@K, Hit Rate, MRR with parameter snapshots
- **LLM-as-a-Judge**: Faithfulness, Answer Relevancy, Context Precision scoring using configurable judge model
- **Per-Source Contribution**: Track which retrieval source (Vector/Tree/Graph) contributed to each query hit
- **AI Eval Set Generator**: Source-grounded question generation with multi-turn conversation context support
- **Evaluation Dashboard** (`rag-eval-dashboard.tsx`): Compare up to 3 runs side-by-side with drill-down per-query analysis
- **Deploy-to-Agent**: One-click deployment of winning RAG configuration to any agent

#### Autonomous Auto-Tuner (Phase 7)
- **Overseer LLM Agent** (`rag_eval_tuner.rs`): Iterative optimization loop analyzing failure modes
- **Background Job System**: `tokio::spawn` worker with `rag_auto_tuner_jobs` tracking table
- **Dashboard Integration**: Real-time progress polling (3s interval) with status badge
- **Configurable Iterations**: 1-10 optimization cycles targeting NDCG, MRR, or Hit Rate

#### Cross-Encoder Re-ranking (Phase 8)
- **`UniversalClient::rerank()`**: TEI-compatible REST API for Cross-Encoder models
- **`LlmRouter::resolve_reranker()`**: Default `BAAI/bge-reranker-v2-m3` via Heimdall
- **`cross_encoder_rerank()`**: Async ML-based reranking in `ensemble.rs`
- **Two-Stage Pipeline**: RRF pre-filter → Cross-Encoder scoring → final top_k
- **Graceful Fallback**: Automatic degradation to RRF if Cross-Encoder is unavailable

#### Agent RAG Integration (Phase 1-4)
- **Agent Chat → Ensemble Search**: `agent_chat()` now calls `run_parallel_search()` with full 3-source retrieval
- **Per-Agent RAG Parameters**: `rag_params` and `rerank_config` JSON columns in `agent_configs`
- **Agent Studio UI**: Weight sliders, advanced params, source filters, re-ranking config panel
- **Search Filtering**: Source-level filtering by `source_id` and `source_type` across all 3 retrieval sources

### 🗃️ Database Migrations
- `sprint35_auto_tuner.sql`: `rag_auto_tuner_jobs` table for background optimization tracking

### 🧪 Tests
- 75 unit tests passing (ensemble: 18, search: 51, rag_eval: 6)
- `cargo check`: 0 compile errors

---

## [0.29.0] — 2026-03-30 (Sprint 34)

### Features
- Swarm multi-agent orchestration (Skill & Soul architecture)
- Axum v0.8 path parameter syntax migration
- Sakura cluster mock models for demo
- Model downloader modal in dashboard

---

*Full history available in git log.*
