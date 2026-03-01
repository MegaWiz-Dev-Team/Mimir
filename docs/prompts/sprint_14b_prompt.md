# Sprint 14b Session Prompt — Deploy & Docs
**Project:** Project Mimir
**Sprint:** 14b (Week 15-16)
**Branch:** `feat/sprint-14b-deploy`
**มาตรฐาน:** ISO/IEC 29110 + TDD (Test-Driven Development)

---

## 🎯 Sprint Goal
ทำระบบ Deployment, Documentation, Backup/DR, และ MLX/vLLM Phase 2 ให้ครบ ตามแผนใน PM-01 Project Plan

## 📋 Sprint 14b Features (6 ข้อ)

### 1. Setup & Deployment (Docker Compose prod)
**Scope:**
- สร้าง `docker-compose.prod.yml` สำหรับ production deployment
- สร้าง `.env.example` template พร้อม comments ทุก variable
- สร้าง `scripts/setup.sh` — interactive first-time setup script
  - ตรวจสอบ dependencies (Docker, OrbStack)
  - สร้าง `.env` จาก template
  - Pull images + build + start containers
  - Run migrations + health check
- TDD: test script functions (validate_env, check_dependencies)

### 2. Deployment Test (M3 → M4 Pro)
**Scope:**
- สร้าง `scripts/deploy-test.sh` — automated deployment verification
  - Health check all services (Bridge, Dashboard, MariaDB, Qdrant, Neo4j, Redis)
  - API endpoint smoke tests (auth, sources, agents, mcp)
  - Verify frontend build + pages accessible
  - Log system resource usage (CPU, RAM, disk)
- TDD: test health check functions, endpoint validation

### 3. Update & Rollback
**Scope:**
- สร้าง `scripts/update.sh`
  - Auto-backup before update (DB dump + config snapshot)
  - Pull latest images from GHCR
  - Run new migrations
  - Health check after update
  - Rollback if health check fails
- สร้าง `scripts/rollback.sh`
  - Restore from latest backup
  - Rollback migrations (using .down.sql)
  - Restart services
- TDD: test backup/restore functions, rollback logic

### 4. API Documentation (OpenAPI/Swagger)
**Scope:**
- สร้าง `docs/api/openapi.yaml` — OpenAPI 3.0 spec
  - Document all /api/v1/* endpoints (iam, auth, sources, pipeline, agents, mcp, vault ฯลฯ)
  - Request/Response schemas + examples
  - Authentication (Bearer token)
- เพิ่ม Swagger UI route ที่ `/api/docs` (ใช้ `utoipa` crate หรือ static Swagger UI)
- TDD: test schema validation, endpoint coverage

### 5. Backup & DR (Disaster Recovery)
**Scope:**
- สร้าง `scripts/backup.sh`
  - MariaDB: `mysqldump` → compressed file
  - Qdrant: snapshot API
  - Neo4j: `neo4j-admin dump`
  - Config: `.env` + `docker-compose` backup
  - Retention policy (keep last 7 daily, 4 weekly)
- สร้าง `scripts/restore.sh`
  - Select backup to restore
  - Restore DB + config
  - Health check after restore
- Backend Rust: `services/backup.rs` — backup status API
- TDD: test backup path generation, retention cleanup

### 6. MLX + vLLM Providers Phase 2
**Scope:**
- เพิ่ม MLX provider ใน `services/llm_provider.rs` (หรือสร้างใหม่)
  - MLX Server endpoint (localhost:8080, OpenAI-compatible)
  - Model listing, chat completion, embedding
- เพิ่ม vLLM provider
  - vLLM endpoint (OpenAI-compatible)
  - GPU detection, model loading
- Benchmark service: compare latency/throughput across providers
- TDD: test provider config, request builder, response parser

---

## 🔄 ขั้นตอนการทำงาน (Workflow) — ทำตามลำดับนี้

### Phase 1: Planning & GitHub Issues
1. สร้าง GitHub Issues สำหรับแต่ละ feature (6 issues)
2. สร้าง branch `feat/sprint-14b-deploy` จาก `feat/sprint-14-phase-1`

### Phase 2: Implementation (TDD)
**ทำทีละ feature ตามลำดับ:**

สำหรับแต่ละ feature:
1. **เขียน Test ก่อน** (Red) — สร้าง test file พร้อม test cases
2. **Implement** (Green) — เขียน code ให้ tests ผ่าน
3. **Refactor** (Refactor) — ปรับปรุง code quality
4. **Verify** — `cargo test` / `npx jest` ผ่านทุกข้อ
5. **Commit** — commit ด้วยข้อความที่อ้างอิง Issue #

**ลำดับแนะนำ:**
```
1. Backup & DR         → พื้นฐานที่ต้องมีก่อน update/rollback
2. Update & Rollback   → ใช้ backup เป็น dependency
3. Setup & Deployment  → Docker Compose prod + setup script
4. Deployment Test     → ทดสอบ deployment ข้างบน
5. API Documentation   → Document ทุก endpoint ที่มี
6. MLX + vLLM Phase 2  → เพิ่ม LLM providers
```

### Phase 3: Testing (ISO 29110 — SI-04)
1. **สร้าง Test Script** `docs/iso_29110/si/SI_04_14b_Sprint14b_TestScript.md`
   - ใช้ format เดียวกับ SI-04-14 (Sprint 14a)
   - ตารางทดสอบแบ่งเป็น:
     - ส่วนที่ 1: Unit Tests (cargo test, npx jest)
     - ส่วนที่ 2: Script Tests (setup.sh, backup.sh, update.sh)
     - ส่วนที่ 3: API Documentation Coverage
     - ส่วนที่ 4: Provider Tests (MLX, vLLM)
   - ทุกข้อต้องมี: ID, Scenario, Steps, Expected, Result, Issue#/PR#, หมายเหตุ

2. **Execute Tests ตาม Script** — ทดสอบทีละข้อตามลำดับ
   - รัน `cargo test` → บันทึกผล
   - รัน scripts → บันทึกผล
   - ทดสอบ API docs accessible → บันทึกผล
   - บันทึกทุกข้อเป็น ✅ Pass / ❌ Fail / ⏳ Pending

### Phase 4: ISO Documentation
1. **Update SI-02** (Software Design Document)
   - เพิ่ม Sprint 14b subsystem description
   - อัปเดต deployment architecture

2. **Update SI-03** (Traceability Matrix)
   - เพิ่ม entries สำหรับ 6 features ของ 14b

3. **สร้าง PM-02.14b** (Sprint Report) `docs/iso_29110/pm/PM_02_14b_Sprint14b_Report.md`
   - Sprint Scope
   - Testing Summary (อ้างอิง SI-04-14b)
   - GitHub Issues & PRs
   - Changes Detail
   - Issues & Resolutions

4. **Update PM-02** (Main Status Reports)
   - เพิ่ม Sprint 14b row

### Phase 5: Final Verification & Push
1. รัน full test suite:
   ```bash
   cargo test -p mimir-core-ai 2>&1 | tail -5
   cd ro-ai-dashboard && npx jest --no-coverage 2>&1 | tail -5
   npx next build 2>&1 | tail -10
   ```
2. Commit all changes
3. Push to `feat/sprint-14b-deploy`
4. สร้าง PR → merge to main

---

## 📁 Files to Create/Modify

### New Files
```
scripts/setup.sh                    — First-time setup
scripts/deploy-test.sh              — Deployment verification
scripts/update.sh                   — Update with auto-backup
scripts/rollback.sh                 — Rollback to backup
scripts/backup.sh                   — Automated backup
scripts/restore.sh                  — Restore from backup
docker-compose.prod.yml             — Production compose
.env.example                        — Environment template
docs/api/openapi.yaml               — OpenAPI 3.0 spec
```

### New Rust Services
```
mimir-core-ai/src/services/backup.rs        — Backup status API
mimir-core-ai/src/services/llm_provider.rs  — MLX + vLLM providers (หรือ extend existing)
```

### New Routes
```
ro-ai-bridge/src/routes/backup.rs  — GET /backup/status, POST /backup/trigger
ro-ai-bridge/src/routes/docs.rs    — GET /api/docs (Swagger UI)
```

### ISO Documents
```
docs/iso_29110/si/SI_04_14b_Sprint14b_TestScript.md
docs/iso_29110/pm/PM_02_14b_Sprint14b_Report.md
docs/iso_29110/si/SI_02_Software_Design_Document.md  (update)
docs/iso_29110/si/SI_03_Traceability_Matrix.md       (update)
docs/iso_29110/pm/PM_02_Status_Reports.md             (update)
```

---

## ⚠️ Important Notes
- **Branch:** สร้าง `feat/sprint-14b-deploy` จาก `feat/sprint-14-phase-1`
- **TDD:** เขียน test ก่อน implement ทุก feature
- **ISO:** ทุก feature ต้องมี test case ใน SI-04, traceable ใน SI-03
- **Commit Convention:** `feat(sprint-14b): <description> #<issue>`
- **Sprint 14a Reference:** 217/217 backend tests, 64/66 frontend tests เป็น baseline
- **Existing Infra:** Docker Compose dev อยู่ที่ `docker-compose.yml`, `.env` อยู่ที่ root
