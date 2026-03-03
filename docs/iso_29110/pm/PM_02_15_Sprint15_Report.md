# PM-02-15: Sprint 15 Status Report — Heimdall LLM Provider
| Field      | Value       |
| ---------- | ----------- |
| **Sprint** | 15          |
| **Period** | Week 17     |
| **Date**   | 2026-03-04  |
| **Status** | ✅ Completed |

---

## Sprint Goal
เพิ่ม Heimdall Self-Hosted LLM Gateway เป็น Provider ใหม่ พร้อม 5 models ใน Backend + Frontend ให้ใช้ได้ทุก feature (Chat, RAG, QA Generation, Evaluation)

## Deliverables

### 1. Heimdall LLM Provider — Backend (#180) ✅
- **llm_provider.rs**: `LlmProvider::Heimdall` enum, `ProviderConfig::heimdall_default()`, `build_heimdall_request()`, `HEIMDALL_MODELS` (5 models), updated URL builders + validation + `detect_gpu_info()`
- **rag_engine/mod.rs**: `AgentBackend::Heimdall` with `reqwest::Client` (OpenAI-compatible), auto-default when `HEIMDALL_API_URL` set
- **generator.rs**: `GeneratorClient::Heimdall` for QA generation + missing QA
- **pipeline.rs**: Heimdall match arms in 3 provider-selection locations
- **monitor.rs**: Heimdall match arms in chat + stream handlers
- **config.rs**: `heimdall_api_url`, `heimdall_api_key`, `heimdall_model` fields

### 2. Heimdall LLM Provider — Frontend (#180) ✅
- **settings/page.tsx**: Heimdall in provider dropdown (top position) + 5 model options with descriptive labels

### 3. Configuration (#180) ✅
- **.env.example**: `HEIMDALL_API_URL`, `HEIMDALL_API_KEY`, `HEIMDALL_MODEL`

## Test Results

| Metric                 | Value                                             |
| :--------------------- | :------------------------------------------------ |
| **Backend Unit Tests** | 33/33 ✅                                           |
| **Frontend Build**     | Exit 0 ✅                                          |
| **Feature Tests**      | 22/22 ✅                                           |
| **Total**              | 25/25 ✅                                           |
| **Test Script**        | [SI-04-15](../si/SI_04_15_Sprint15_TestScript.md) |

## Heimdall Models Registry

| Model ID                                     | Size         | Purpose                 |
| :------------------------------------------- | :----------- | :---------------------- |
| `mlx-community/Qwen3.5-35B-A3B-4bit`         | 35B (MoE 3B) | Primary — RAG, Chat, QA |
| `mlx-community/Qwen3.5-27B-4bit`             | 27B          | Complex reasoning       |
| `mlx-community/Qwen3.5-9B-MLX-4bit`          | 9B           | Fast / low latency      |
| `mlx-community/Qwen3-0.6B-4bit`              | 0.6B         | Smoke test              |
| `lmstudio-community/medgemma-4b-it-MLX-4bit` | 4B           | Medical domain          |

## Files Changed

| File                | Type     | Changes                                          |
| :------------------ | :------- | :----------------------------------------------- |
| `llm_provider.rs`   | Modified | Heimdall enum + config + builders + 10 TDD tests |
| `rag_engine/mod.rs` | Modified | AgentBackend::Heimdall (reqwest)                 |
| `generator.rs`      | Modified | GeneratorClient::Heimdall                        |
| `pipeline.rs`       | Modified | 3 provider match arms                            |
| `config.rs`         | Modified | Heimdall config fields                           |
| `monitor.rs`        | Modified | Chat/stream handler match arms                   |
| `settings/page.tsx` | Modified | Provider dropdown + 5 models                     |
| `.env.example`      | Modified | Heimdall env vars                                |
