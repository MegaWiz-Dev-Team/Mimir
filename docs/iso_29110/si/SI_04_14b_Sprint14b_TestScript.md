# SI-04-14b: Sprint 14b Test Script (Deploy & Docs — Backup, Update, Setup, API Docs, MLX/vLLM)
**Project Name:** Project Mimir
**Sprint:** 14b
**Feature:** Backup & DR (#158), Update & Rollback (#159), Setup & Deployment (#160), Deployment Test (#161), API Documentation (#162), MLX + vLLM Phase 2 (#163), Configurable Max Crawl Pages (#164)
**ทดสอบเมื่อ:** 2026-03-01

## แนวทางการทดสอบตามมาตรฐาน ISO 29110 (Test Instructions & TDD Approach)
กระบวนการนี้อ้างอิงหลักการ **Test-Driven Development (TDD)** โดยต้องดำเนินการทดสอบ Unit Test ให้ผ่านก่อนการทดสอบระบบจริง และให้ทดสอบทีละข้อตามลำดับ (Step-by-Step) เพื่อให้เป็นไปตามมาตรฐานการควบคุมคุณภาพ

1. **เขียนและรัน Unit Test**: รัน Unit Test ของระบบ (ทั้ง Frontend และ Backend) ให้ผ่าน `✅ Pass` ทุกข้อก่อนเริ่มทดสอบ UI (อ้างอิงตามแนวทาง TDD)
2. **รันระบบ Environment**: รัน Database (`docker-compose up -d`), Backend (`cargo run --bin ro-ai-bridge`), และ Frontend (`npm run dev`)
3. **ทดสอบทีละข้อ (Step-by-step)**: ดำเนินการทดสอบตาม Test Scenarios ด้านล่าง **ทีละข้อ** อย่างเคร่งครัด ห้ามข้ามขั้นตอน
4. **บันทึกผลตามมาตรฐาน ISO**: 
   - บันทึกผลในช่อง **"ผลการประเมิน"** (`✅ Pass` หรือ `❌ Fail`)
   - **ต้อง** ระบุหมายเลข **Issue** และ **Pull Request (PR)** ของ GitHub ที่เกี่ยวข้องในแต่ละข้อ เพื่อให้สามารถอ้างอิงย้อนกลับได้ (Traceability) ตามมาตรฐาน ISO 29110

---

## ตารางการทดสอบตามสถานการณ์ (Test Scenarios)

### ส่วนที่ 1: การตรวจสอบระดับ Unit Test (TDD Approach)

#### 1.1 Backend Unit Tests (`cargo check` + `cargo test`)

| ID              | Test Scenario           | Action / Steps (ขั้นตอนการทดสอบ)         | Expected Result (ผลที่คาดหวัง)    | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                  |
| :-------------- | :---------------------- | :------------------------------------- | :----------------------------- | :---------- | :------------- | :--------------------------------------- |
| **TC_SP14b_U1** | Backend compilation     | 1. รัน `cargo check` ใน `ro-ai-bridge/` | Compilation สำเร็จ 0 errors      | ✅ Pass      | All Sprint 14b | warnings only — ไม่กระทบ                  |
| **TC_SP14b_U2** | Backend unit test suite | 1. รัน `cargo test -p mimir-core-ai`    | All 255 tests pass, 0 failures | ✅ Pass      | #158-#163      | 255 passed (217+38 Backup/LLM), 0 failed |

---

### ส่วนที่ 2: การตรวจสอบ Backend Service Tests (TDD via `cargo test`)

#### 2.1 Backup & DR (#158)

| ID              | Test Scenario                    | Action / Steps (ขั้นตอนการทดสอบ)                                                            | Expected Result (ผลที่คาดหวัง)                                | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                    |
| :-------------- | :------------------------------- | :---------------------------------------------------------------------------------------- | :--------------------------------------------------------- | :---------- | :------------- | :----------------------------------------- |
| **TC_SP14b_01** | Backup path generation — MariaDB | 1. ทดสอบ `generate_backup_path(&config, &BackupType::MariaDB)`                            | Path = `{dir}/mariadb/mimir_mariadb_{timestamp}.sql.gz`    | ✅ Pass      | #158           | 4 tests ครอบคลุม MariaDB/Qdrant/Config/Full |
| **TC_SP14b_02** | Backup path generation — Qdrant  | 1. ทดสอบ `generate_backup_path(&config, &BackupType::Qdrant)`                             | Path = `{dir}/qdrant/mimir_qdrant_{timestamp}.snapshot`    | ✅ Pass      | #158           | Extension ถูกต้องตาม type                    |
| **TC_SP14b_03** | Filename parsing — valid         | 1. ทดสอบ `parse_backup_filename("mimir_mariadb_20260301_120000.sql.gz", "/data/backups")` | Returns BackupEntry with type=MariaDB, date="20260301"     | ✅ Pass      | #158           | 2 tests valid (MariaDB, Qdrant)            |
| **TC_SP14b_04** | Filename parsing — invalid       | 1. ทดสอบ `parse_backup_filename("random_file.txt", "/data")`                              | Returns None                                               | ✅ Pass      | #158           | 3 invalid cases ทดสอบ                      |
| **TC_SP14b_05** | Retention — keeps daily          | 1. ทดสอบ `calculate_retention` ด้วย 7 entries, retention=7                                 | to_delete list ว่าง (เก็บทั้งหมด)                              | ✅ Pass      | #158           | Pure function test                         |
| **TC_SP14b_06** | Retention — deletes old          | 1. ทดสอบ `calculate_retention` ด้วย 10 entries, retention=3, weekly=2                      | ลบ entries เก่ากว่า daily+weekly quota                       | ✅ Pass      | #158           | Newest 3 ต้องไม่อยู่ใน delete list             |
| **TC_SP14b_07** | Backup sorting                   | 1. ทดสอบ `list_backups_sorted` ด้วย unsorted entries                                       | Sorted by date descending (newest first)                   | ✅ Pass      | #158           | Pure function test                         |
| **TC_SP14b_08** | Backup status builder            | 1. ทดสอบ `build_backup_status` ด้วย 2 entries                                              | status.total_backups=2, latest=newest entry                | ✅ Pass      | #158           | 2 tests (with data + empty)                |
| **TC_SP14b_09** | Backup API: GET /backup/status   | 1. Code review: route registered in `main.rs`                                             | Endpoint accessible, returns BackupStatus JSON             | ✅ Pass      | #158           | Code review confirmed                      |
| **TC_SP14b_10** | Backup API: POST /backup/trigger | 1. Code review: trigger route + response builder                                          | Returns `{triggered: true, message: ..., backup_dir: ...}` | ✅ Pass      | #158           | Code review confirmed                      |

#### 2.2 MLX + vLLM Phase 2 (#163)

| ID              | Test Scenario                    | Action / Steps (ขั้นตอนการทดสอบ)                                        | Expected Result (ผลที่คาดหวัง)                                               | ผลการประเมิน | Issue # / PR # | หมายเหตุ                          |
| :-------------- | :------------------------------- | :-------------------------------------------------------------------- | :------------------------------------------------------------------------ | :---------- | :------------- | :------------------------------- |
| **TC_SP14b_11** | MLX request builder              | 1. ทดสอบ `build_mlx_request` ด้วย default config + messages            | JSON มี model, messages, max_tokens, temperature, stream=false             | ✅ Pass      | #163           | OpenAI-compatible format         |
| **TC_SP14b_12** | vLLM request builder             | 1. ทดสอบ `build_vllm_request` ด้วย default + custom model              | JSON มี model, messages, n=1                                               | ✅ Pass      | #163           | 2 tests (default + custom model) |
| **TC_SP14b_13** | Chat response parser — success   | 1. ทดสอบ `parse_chat_response` ด้วย valid JSON mock                    | Parsed choices, usage, finish_reason ถูกต้อง                                | ✅ Pass      | #163           | Validates all fields             |
| **TC_SP14b_14** | Chat response parser — error     | 1. ทดสอบ `parse_chat_response` ด้วย JSON ที่ไม่มี choices                  | Returns Err("Missing 'choices'...")                                       | ✅ Pass      | #163           | Handles malformed response       |
| **TC_SP14b_15** | Models response parser           | 1. ทดสอบ `parse_models_response` ด้วย 2 model entries                  | Returns Vec<ModelInfo> len=2, id/owned_by ถูกต้อง                           | ✅ Pass      | #163           | 2 tests (with data + empty)      |
| **TC_SP14b_16** | Provider config validation — OK  | 1. ทดสอบ `validate_provider_config` ด้วย MLX + vLLM defaults           | Ok(()) for both                                                           | ✅ Pass      | #163           | 2 tests                          |
| **TC_SP14b_17** | Provider config validation — Err | 1. ทดสอบ invalid: empty endpoint, Gemini no key, bad temp, 0 tokens   | Err with descriptive message                                              | ✅ Pass      | #163           | 4 error cases                    |
| **TC_SP14b_18** | Benchmark calculation            | 1. ทดสอบ `calculate_benchmark` success + failure                      | Success: tokens_per_second=100.0 (50t/0.5s), Failure: tps=0.0             | ✅ Pass      | #163           | 2 tests                          |
| **TC_SP14b_19** | URL builders (chat/models/embed) | 1. ทดสอบ `build_chat_url`, `build_models_url`, `build_embeddings_url` | MLX: `/v1/chat/completions`, Gemini: `/v1beta/models/...:generateContent` | ✅ Pass      | #163           | 5 tests ครอบคลุม MLX/vLLM/Gemini  |
| **TC_SP14b_20** | Provider enum                    | 1. ทดสอบ `as_str()` + `from_str()` สำหรับ Gemini/MLX/VLLM               | Round-trip conversion ถูกต้อง, unknown→None                                 | ✅ Pass      | #163           | 2 tests                          |
| **TC_SP14b_21** | GPU detection                    | 1. ทดสอบ `detect_gpu_info()`                                          | Returns JSON with apple_silicon, cuda_available, recommended_provider     | ✅ Pass      | #163           | Pure function — no GPU required  |

---

### ส่วนที่ 3: การตรวจสอบ Shell Scripts

#### 3.1 Backup & Restore Scripts (#158)

| ID              | Test Scenario              | Action / Steps (ขั้นตอนการทดสอบ)                               | Expected Result (ผลที่คาดหวัง)                               | ผลการประเมิน | Issue # / PR # | หมายเหตุ                       |
| :-------------- | :------------------------- | :----------------------------------------------------------- | :-------------------------------------------------------- | :---------- | :------------- | :---------------------------- |
| **TC_SP14b_22** | backup.sh — syntax valid   | 1. ตรวจสอบ `bash -n scripts/backup.sh`                       | Exit 0, no syntax errors                                  | ✅ Pass      | #158           | `set -euo pipefail` enforced  |
| **TC_SP14b_23** | backup.sh — functions      | 1. Code review: backup_mariadb, backup_qdrant, backup_config | Functions handle both local mysqldump + Docker fallback   | ✅ Pass      | #158           | Retention cleanup included    |
| **TC_SP14b_24** | restore.sh — syntax valid  | 1. ตรวจสอบ `bash -n scripts/restore.sh`                      | Exit 0, no syntax errors                                  | ✅ Pass      | #158           | Interactive restore with menu |
| **TC_SP14b_25** | restore.sh — safety backup | 1. Code review: restore_config function                      | Creates `.env.pre-restore.{timestamp}` before overwriting | ✅ Pass      | #158           | Safety net ก่อน restore        |

#### 3.2 Update & Rollback Scripts (#159)

| ID              | Test Scenario                | Action / Steps (ขั้นตอนการทดสอบ)             | Expected Result (ผลที่คาดหวัง)                             | ผลการประเมิน | Issue # / PR # | หมายเหตุ                       |
| :-------------- | :--------------------------- | :----------------------------------------- | :------------------------------------------------------ | :---------- | :------------- | :---------------------------- |
| **TC_SP14b_26** | update.sh — syntax valid     | 1. ตรวจสอบ `bash -n scripts/update.sh`     | Exit 0, no syntax errors                                | ✅ Pass      | #159           | Auto-backup before update     |
| **TC_SP14b_27** | update.sh — rollback on fail | 1. Code review: health_check + do_rollback | If health check fails → calls rollback.sh automatically | ✅ Pass      | #159           | Automatic recovery            |
| **TC_SP14b_28** | rollback.sh — syntax valid   | 1. ตรวจสอบ `bash -n scripts/rollback.sh`   | Exit 0, no syntax errors                                | ✅ Pass      | #159           | Supports `--auto` mode        |
| **TC_SP14b_29** | rollback.sh — auto mode      | 1. Code review: `--auto` flag handling     | Skips confirmation prompt in auto mode                  | ✅ Pass      | #159           | Called from update.sh on fail |

#### 3.3 Setup & Deployment Scripts (#160, #161)

| ID              | Test Scenario                 | Action / Steps (ขั้นตอนการทดสอบ)              | Expected Result (ผลที่คาดหวัง)                                            | ผลการประเมิน | Issue # / PR # | หมายเหตุ                       |
| :-------------- | :---------------------------- | :------------------------------------------ | :--------------------------------------------------------------------- | :---------- | :------------- | :---------------------------- |
| **TC_SP14b_30** | setup.sh — syntax valid       | 1. ตรวจสอบ `bash -n scripts/setup.sh`       | Exit 0, no syntax errors                                               | ✅ Pass      | #160           | Interactive first-time setup  |
| **TC_SP14b_31** | setup.sh — dependency check   | 1. Code review: check_dependencies function | Checks Docker, Compose, OrbStack, Rust, Node.js                        | ✅ Pass      | #160           | Required vs optional deps     |
| **TC_SP14b_32** | setup.sh — env validation     | 1. Code review: validate_env function       | Validates DATABASE_URL, MARIADB_ROOT_PASSWORD, JWT_SECRET              | ✅ Pass      | #160           | Auto-generates JWT secret     |
| **TC_SP14b_33** | deploy-test.sh — syntax valid | 1. ตรวจสอบ `bash -n scripts/deploy-test.sh` | Exit 0, no syntax errors                                               | ✅ Pass      | #161           | 10+ automated checks          |
| **TC_SP14b_34** | deploy-test.sh — smoke tests  | 1. Code review: api_smoke_tests function    | Tests /health, /api/v1/auth/me, /api/v1/sources, /api/v1/backup/status | ✅ Pass      | #161           | Reports PASS/FAIL/WARN counts |

---

### ส่วนที่ 4: การตรวจสอบ Configuration & Documentation (#160, #162)

| ID              | Test Scenario                 | Action / Steps (ขั้นตอนการทดสอบ)                                | Expected Result (ผลที่คาดหวัง)                                         | ผลการประเมิน | Issue # / PR # | หมายเหตุ                               |
| :-------------- | :---------------------------- | :------------------------------------------------------------ | :------------------------------------------------------------------ | :---------- | :------------- | :------------------------------------ |
| **TC_SP14b_35** | docker-compose.prod.yml valid | 1. ตรวจสอบ `docker compose -f docker-compose.prod.yml config` | Config valid, 5 services defined                                    | ✅ Pass      | #160           | Resource limits + health checks       |
| **TC_SP14b_36** | .env.example completeness     | 1. Code review: ตรวจสอบทุก section                             | ครอบคลุม DB, Redis, Qdrant, RustFS, Vault, LLM, JWT, Backup          | ✅ Pass      | #160           | All variables documented              |
| **TC_SP14b_37** | OpenAPI spec — valid YAML     | 1. Code review: `docs/api/openapi.yaml`                       | Valid OpenAPI 3.0.3, 20+ endpoints, schemas defined                 | ✅ Pass      | #162           | Covers all `/api/v1/*` routes         |
| **TC_SP14b_38** | Swagger UI route              | 1. Code review: `routes/docs.rs` registered in main.rs        | `/api/docs` serves Swagger UI, `/api/docs/openapi.yaml` serves spec | ✅ Pass      | #162           | CDN-loaded Swagger UI, `include_str!` |

---

### ส่วนที่ 5: การตรวจสอบ Configurable Max Crawl Pages (#164)

| ID              | Test Scenario                         | Action / Steps (ขั้นตอนการทดสอบ)                                                   | Expected Result (ผลที่คาดหวัง)                                                      | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                |
| :-------------- | :------------------------------------ | :------------------------------------------------------------------------------- | :------------------------------------------------------------------------------- | :---------- | :------------- | :------------------------------------- |
| **TC_SP14b_39** | DB migration — max_crawl_pages column | 1. ตรวจสอบ migration `20260302300000_add_max_crawl_pages.sql`                    | `ALTER TABLE tenant_configs ADD COLUMN max_crawl_pages INT NOT NULL DEFAULT 100` | ✅ Pass      | #164           | Idempotent migration (IF NOT EXISTS)   |
| **TC_SP14b_40** | Backend — TenantConfig model          | 1. Code review: `models/iam.rs` — `TenantConfig` + `UpdateTenantConfigRequest`   | `max_crawl_pages: i32` ใน struct, `Option<i32>` ใน update request                | ✅ Pass      | #164           | Field wired through service layer      |
| **TC_SP14b_41** | Backend — discover_hierarchy reads DB | 1. Code review: `sources.rs` line 433 — `discover_hierarchy` reads tenant config | `let tenant_max = sqlx::query_scalar(...)` แทน hardcode 100                      | ✅ Pass      | #164           | Fallback to 100 if not found           |
| **TC_SP14b_42** | Frontend — Pipeline tab UI            | 1. เปิด Admin Settings → Pipeline tab<br>2. ตรวจสอบ Max Crawl Pages input         | Input แสดงค่า 100, range 10–500, ปุ่ม Save Pipeline Settings enabled                | ✅ Pass      | #164           | Save wired to `updateTenantConfig` API |

---

**สรุปผลการทดสอบ Sprint 14b (Sign-off):**
- [x] Backend Compilation ผ่าน (cargo check: 0 errors, warnings only)
- [x] Backend Unit Tests ผ่าน (255/255: cargo test -p mimir-core-ai)
- [x] Backup & DR Tests ผ่าน (15/15: path gen, parsing, retention, status)
- [x] MLX + vLLM Tests ผ่าน (23/23: request builder, parser, validation, benchmark)
- [x] Shell Scripts ผ่าน (6/6 scripts: syntax valid, functions reviewed)
- [x] Config & Docs ผ่าน (docker-compose, .env.example, openapi.yaml, Swagger UI)
- [x] Configurable Max Crawl Pages ผ่าน (4/4: migration, model, backend, frontend UI)

**ผลการทดสอบ 2026-03-01:**
- **Unit Tests (Backend)**: 255/255 ✅ (+38 Backup/LLM Provider tests)
- **Backend Service Tests**: 21/21 ✅ Pass (TC_SP14b_01~21)
- **Shell Script Tests**: 13/13 ✅ Pass (TC_SP14b_22~34)
- **Config & Docs Tests**: 4/4 ✅ Pass (TC_SP14b_35~38)
- **Max Crawl Pages Tests**: 4/4 ✅ Pass (TC_SP14b_39~42)
- **Total**: 2/2 unit suites + 42/42 feature tests = **44/44 all pass**

**หมายเหตุ:**
- TC_SP14b_09, TC_SP14b_10 ยืนยันผ่าน **code review** เนื่องจากต้องมี filesystem + running services เพื่อทดสอบ end-to-end
- TC_SP14b_22~34 (Shell Scripts) ทดสอบ syntax + code review เนื่องจากต้องมี Docker + databases จริงเพื่อรัน

**อ้างอิง (GitHub References):**
- **Issues:** #158, #159, #160, #161, #162, #163, #164
- **Pull Requests:** (pending PR creation from `feat/sprint-14b-deploy` branch)
- **Issues Found During Review:** #165, #166, #167, #168, #169, #170, #171, #172, #173, #174, #175 → จะแก้ไขใน Sprint 15
