# PM-02: Sprint 29 Report — Docker Build Fix & sqlx Offline
**Project Name:** Project Mimir
**Sprint:** 29 (Docker & Infrastructure)
**Date:** 2026-03-13
**Standard:** ISO/IEC 29110 — PM Process

---

## Sprint Goal
แก้ปัญหา Mimir API Docker build ที่ fail 120 errors ให้ build ผ่าน และ integrate เข้ากับ Asgard unified Docker Compose

## Deliverables

| Item | Status |
|:--|:--|
| Fix Dockerfile — remove broken dep caching | ✅ Done |
| Generate `.sqlx/` offline query cache (28 queries) | ✅ Done |
| Set `SQLX_OFFLINE=true` for Docker build | ✅ Done |
| Fix `include_str!` openapi.yaml path resolution | ✅ Done |
| Expand Docker context to Mimir root | ✅ Done |
| Add `.dockerignore` at Mimir root | ✅ Done |
| `docker compose build mimir-api` passes | ✅ Done |
| `docker compose up mimir-api` healthy | ✅ Done |

## Root Causes Fixed

| # | Error Count | Root Cause | Fix |
|:--|:--|:--|:--|
| 1 | 120 errors (cascade) | Dummy `lib.rs` dep caching trick broke Cargo workspace rebuild | Removed dep caching, copy full source |
| 2 | 22 errors | `sqlx::query!` macros need live DB at compile time | `SQLX_OFFLINE=true` + `.sqlx/` query cache |
| 3 | 1 error | `include_str!("../../../docs/api/openapi.yaml")` outside WORKDIR | WORKDIR `/build/ro-ai-bridge` + copy `docs/` |

## Files Changed

| File | Change |
|:--|:--|
| `ro-ai-bridge/Dockerfile` | Rewritten — Mimir root context, SQLX_OFFLINE, correct WORKDIR |
| `ro-ai-bridge/.sqlx/*.json` | 28 new query cache files for offline compile |
| `.dockerignore` | New — exclude target/, dashboard, .git |

## Metrics

| Metric | Value |
|:--|:--|
| Duration | ~3 hours |
| Docker Image Size | 204MB |
| Build Time | ~15 min (Rust release) |
| Errors Fixed | 120 → 0 |
| PR | [#250](https://github.com/megacare-dev/Mimir/pull/250) (squash-merged) |

## Docker Compose Integration

| Variable | Value |
|:--|:--|
| Build context | `../Mimir` (repo root) |
| Dockerfile | `ro-ai-bridge/Dockerfile` |
| Internal port | 8080 |
| External port | `${MIMIR_API_PORT:-3000}` |
| `DATABASE_URL` | `mysql://mimir:***@mariadb:3306/mimir` |
| Healthcheck | `curl -f http://localhost:8080/health` |

---

*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
