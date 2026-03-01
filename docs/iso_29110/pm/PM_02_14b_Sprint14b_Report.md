# PM-02-14b: Sprint 14b Status Report — Deploy & Docs
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

## Test Results

| Metric                     | Value   |
| -------------------------- | ------- |
| New TDD tests (Sprint 14b) | 38      |
| Total backend tests        | 255     |
| Pass rate                  | 100%    |
| Compilation                | ✅ Clean |

## Files Changed

| Category      | Count | Key Files                                                                           |
| ------------- | ----- | ----------------------------------------------------------------------------------- |
| Rust Services | 2     | `backup.rs`, `llm_provider.rs`                                                      |
| Rust Routes   | 2     | `backup.rs`, `docs.rs`                                                              |
| Shell Scripts | 6     | `backup.sh`, `restore.sh`, `update.sh`, `rollback.sh`, `setup.sh`, `deploy-test.sh` |
| Config Files  | 2     | `docker-compose.prod.yml`, `.env.example`                                           |
| Documentation | 2     | `openapi.yaml`, this report                                                         |

## Risks & Issues
- None — all features delivered and tests passing
