# PM-02-14b: Sprint 14b Status Report — Deploy & Docs & Config
| Field      | Value       |
| ---------- | ----------- |
| **Sprint** | 14b         |
| **Period** | Week 15-16  |
| **Date**   | 2026-03-01  |
| **Status** | ✅ Completed |

---

## Sprint Goal
ทำระบบ Deployment, Documentation, Backup/DR, และ MLX/vLLM Phase 2 ให้ครบ ตามแผนใน PM-01 Project Plan

## Deliverables

### 1. Backup & DR (#158) ✅
- **Service**: `mimir-core-ai/src/services/backup.rs` — 15 TDD tests
- **Routes**: `ro-ai-bridge/src/routes/backup.rs` — GET/POST endpoints
- **Scripts**: `scripts/backup.sh` (MariaDB, Qdrant, Config) + `scripts/restore.sh`
- **Features**: Retention policy, timestamped paths, gzip compression

### 2. Update & Rollback (#159) ✅
- **Scripts**: `scripts/update.sh` (auto-backup → pull → restart → health check)
- **Scripts**: `scripts/rollback.sh` (restore from backup, `--auto` mode)

### 3. Setup & Deployment (#160) ✅
- **Docker**: `docker-compose.prod.yml` with resource limits, health checks, logging
- **Template**: `.env.example` with all variables documented
- **Script**: `scripts/setup.sh` — interactive first-time setup

### 4. Deployment Test (#161) ✅
- **Script**: `scripts/deploy-test.sh` — 10+ automated checks
- **Covers**: Service health, API smoke tests, frontend, resource usage

### 5. API Documentation (#162) ✅
- **Spec**: `docs/api/openapi.yaml` — OpenAPI 3.0.3 with 20+ endpoints
- **UI**: `ro-ai-bridge/src/routes/docs.rs` — Swagger UI at `/api/docs`

### 6. MLX + vLLM Phase 2 (#163) ✅
- **Service**: `mimir-core-ai/src/services/llm_provider.rs` — 23 TDD tests
- **Features**: Provider config, OpenAI-compatible request/response, benchmarking, GPU detection

### 7. Configurable Max Crawl Pages (#164) ✅
- **Migration**: `20260302300000_add_max_crawl_pages.sql` — `max_crawl_pages INT NOT NULL DEFAULT 100`
- **Backend**: `models/iam.rs` + `services/iam.rs` + `sources.rs` — reads from tenant config
- **Frontend**: `settings/page.tsx` Pipeline tab — input (10–500) + save button

## Test Results

| Metric                     | Value   |
| -------------------------- | ------- |
| New TDD tests (Sprint 14b) | 38+4    |
| Total backend tests        | 255     |
| Pass rate                  | 100%    |
| Compilation                | ✅ Clean |

## Files Changed

| Category      | Count | Key Files                                                                           |
| ------------- | ----- | ----------------------------------------------------------------------------------- |
| Rust Services | 3     | `backup.rs`, `llm_provider.rs`, `iam.rs`                                            |
| Rust Routes   | 3     | `backup.rs`, `docs.rs`, `sources.rs`                                                |
| Shell Scripts | 6     | `backup.sh`, `restore.sh`, `update.sh`, `rollback.sh`, `setup.sh`, `deploy-test.sh` |
| Config Files  | 2     | `docker-compose.prod.yml`, `.env.example`                                           |
| Documentation | 2     | `openapi.yaml`, this report                                                         |

## Issues Found During Sprint Review

ระหว่าง Sprint 14b review & testing พบ issues ที่ต้องแก้ไขใน Sprint 15:

| #   | Issue                                       | Type          | Severity |
| --- | ------------------------------------------- | ------------- | -------- |
| 165 | JWT Expiry — Admin Settings error           | 🐛 Bug         | 🔴 High   |
| 166 | QA UX — Auto Pipeline + One-Click           | ✨ Enhancement | 🟡 Medium |
| 167 | Sources Group by Type (File/Web/MCP/DB)     | ✨ Enhancement | 🟡 Medium |
| 168 | PDF Upload — No S3 key found                | 🐛 Bug         | 🔴 High   |
| 169 | MCP Source — missing URL input              | 🐛 Bug         | 🔴 High   |
| 170 | Folder upload + .DS_Store not filtered      | 🐛 Bug         | 🔴 High   |
| 171 | External DB — Test Connection fails         | 🐛 Bug         | 🔴 High   |
| 172 | Knowledge Base shows 0 chunks               | 🐛 Bug         | 🔴 High   |
| 173 | QA Generation at Knowledge page (manual)    | ✨ Enhancement | 🟡 Medium |
| 174 | Pipeline Status Bar navigation              | 🐛 Bug         | 🟡 Medium |
| 175 | Provider/Model dropdown + dynamic discovery | ✨ Enhancement | 🟡 Medium |

**สรุป:** 6 bugs (🔴 High) + 4 enhancements (🟡 Medium) + 1 new feature → ทั้งหมดจะแก้ไขใน Sprint 15

## Next Sprint
- **Sprint 15** — Bug Fixes & Data Ingress Hardening + UX Improvements
- **Prompt:** `docs/prompts/sprint_15_prompt.md`
- **Issues:** #165–#175 (11 issues)
