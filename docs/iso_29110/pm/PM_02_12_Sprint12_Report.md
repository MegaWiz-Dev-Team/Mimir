# PM-02.12: Sprint 12 Status Report (Web Hierarchy & LLM Observability)

**Project Name:** Project Mimir
**Sprint:** Sprint 12
**Status:** ✅ Completed
**Date:** 2026-02-28

---

## 1. ขอบเขตของ Sprint 12 (Sprint Scope)
- **Backend:** Web Hierarchy Loader — BFS crawl discovery (`discover-hierarchy`) + selective page import (`import-pages`) (#134)
- **Backend:** LLM Usage Logging — auto-log model/tokens/latency/cost per API call, daily token limit, usage summary endpoint (#136)
- **Backend:** Search Settings Persistence — save/load search config via `tenant_configs.search_settings` JSON column (#138)
- **Frontend:** Web Hierarchy UI — Discover Pages button, checkbox tree with status badges (🆕/🔄/✅/🔁), Select All/Deselect All, Import Selected (#135)
- **Frontend:** LLM Analytics Dashboard — KPI cards, model comparison table, recent calls log, date range filter (#137)
- **Frontend:** Search Settings — enabled Save button, load from API, persist on reload (#138)

## 2. สรุปผลการทดสอบ (Testing Verification Summary)

### Backend Unit Tests (4/4 Pass)
| ID         | Description                         | Result |
| ---------- | ----------------------------------- | ------ |
| TC_SP12_U1 | Password hashing — valid            | ✅ Pass |
| TC_SP12_U2 | Password hashing — invalid          | ✅ Pass |
| TC_SP12_U3 | Password hashing — malformed hash   | ✅ Pass |
| TC_SP12_U4 | Full backend test suite (118 tests) | ✅ Pass |

### Frontend Unit Tests (5/5 Pass)
| ID         | Description                         | Result |
| ---------- | ----------------------------------- | ------ |
| TC_SP12_U5 | API — discoverHierarchy             | ✅ Pass |
| TC_SP12_U6 | API — importPages                   | ✅ Pass |
| TC_SP12_U7 | API — fetchLlmUsage                 | ✅ Pass |
| TC_SP12_U8 | API — fetchLlmUsageSummary          | ✅ Pass |
| TC_SP12_U9 | Full frontend test suite (48 tests) | ✅ Pass |

### UI/Feature Tests (16/16 Pass)
| ID         | Description                      | Result |
| ---------- | -------------------------------- | ------ |
| TC_SP12_01 | Discover Pages button visibility | ✅ Pass |
| TC_SP12_02 | Discover Pages — loading state   | ✅ Pass |
| TC_SP12_03 | Checkbox tree display            | ✅ Pass |
| TC_SP12_04 | Select All / Deselect All        | ✅ Pass |
| TC_SP12_05 | Import Selected                  | ✅ Pass |
| TC_SP12_06 | LLM call creates usage log       | ✅ Pass |
| TC_SP12_07 | Daily token limit enforcement    | ✅ Pass |
| TC_SP12_08 | LLM usage summary endpoint       | ✅ Pass |
| TC_SP12_09 | Analytics navbar link            | ✅ Pass |
| TC_SP12_10 | KPI cards display                | ✅ Pass |
| TC_SP12_11 | Model comparison table           | ✅ Pass |
| TC_SP12_12 | Recent calls log                 | ✅ Pass |
| TC_SP12_13 | Date range filter                | ✅ Pass |
| TC_SP12_14 | Search tab loads saved settings  | ✅ Pass |
| TC_SP12_15 | Save button enabled              | ✅ Pass |
| TC_SP12_16 | Save and reload persists         | ✅ Pass |

**Total: 25/25 (100%)**

## 3. GitHub Synchronization & Traceability
### Issues
| Issue # | Title                                                             | Status |
| ------- | ----------------------------------------------------------------- | ------ |
| #134    | Feat: Web Hierarchy Loader — Backend API (BFS crawl + import)     | ✅ Open |
| #135    | Feat: Web Hierarchy Loader — Frontend UI (checkbox tree + badges) | ✅ Open |
| #136    | Feat: LLM Usage Logging — Backend (auto-log + daily limit)        | ✅ Open |
| #137    | Feat: LLM Analytics Dashboard — Frontend (KPI + tables)           | ✅ Open |
| #138    | Feat: Search Settings Backend Persistence (save/load config)      | ✅ Open |

### Pull Requests
| PR # | Title                                                       | Status   |
| ---- | ----------------------------------------------------------- | -------- |
| #139 | docs: Sprint 11 test script + Sprint 12 planning prompt     | ✅ Merged |
| #140 | feat: Sprint 12 Backend — LLM Usage Logging + Web Hierarchy | ✅ Merged |
| #141 | feat: Web Hierarchy Loader Frontend UI (#135)               | ✅ Merged |
| #142 | feat: LLM Analytics Dashboard (#137)                        | ✅ Merged |
| #143 | feat: Search Settings Backend Persistence (#138)            | ✅ Merged |

## 4. รายละเอียดการเปลี่ยนแปลง (Changes Detail)

### Backend (Rust)
1. **`migrations/20260301000000_llm_usage_logs.sql`** — NEW: `llm_usage_logs` table with `model_id`, `input_tokens`, `output_tokens`, `latency_ms`, `status`, `caller` columns
2. **`migrations/20260301100000_hierarchy_fields.sql`** — NEW: Add `title`, `depth`, `parent_url` to `crawled_pages`
3. **`migrations/20260301200000_search_settings.sql`** — NEW: Add `search_settings` JSON column to `tenant_configs`
4. **`src/routes/llm_usage.rs`** — NEW: `GET /llm-usage` (paginated), `GET /llm-usage/summary` (aggregated stats)
5. **`src/routes/sources.rs`** — `POST /{id}/discover-hierarchy` (BFS crawl), `POST /{id}/import-pages`, `call_llm_api_with_logging()` with daily limit

### Frontend (Next.js)
1. **`src/app/sources/page.tsx`** — Discover Pages button, checkbox tree with 🆕 badges, Select All/Deselect All, Import Selected
2. **`src/app/analytics/llm/page.tsx`** — NEW: KPI cards, model comparison table, recent calls log, date range filter
3. **`src/app/settings/page.tsx`** — Search tab loads from `config.search_settings`, Save button enabled
4. **`src/components/navbar.tsx`** — Added Analytics nav item with Activity icon

## 5. ปัญหาที่พบและวิธีแก้ไข (Issues & Resolutions)
1. **Migration FK type mismatch:**
   - *ปัญหา:* `llm_usage_logs.tenant_id` กำหนดเป็น `BIGINT` แต่ `tenants.id` เป็น `VARCHAR(50)` — ทำให้ foreign key constraint ล้มเหลว
   - *แก้ปัญหา:* เปลี่ยนเป็น `VARCHAR(50) NOT NULL` ใน migration file, ล้าง `_sqlx_migrations` record

2. **Duplicate column error:**
   - *ปัญหา:* `ALTER TABLE ADD COLUMN search_settings` ล้มเหลวเพราะ column มีอยู่แล้ว
   - *แก้ปัญหา:* เพิ่ม `IF NOT EXISTS` ใน ALTER statement

3. **Rust type mismatch:**
   - *ปัญหา:* `LlmUsageLog.tenant_id` เป็น `i64` ไม่ตรงกับ `VARCHAR(50)` ในฐานข้อมูล
   - *แก้ปัญหา:* เปลี่ยน `tenant_id` เป็น `String`/`&str` ใน struct และ function parameters

## 6. Sprint 13 Planning
| Feature          | Description                                            | Priority |
| ---------------- | ------------------------------------------------------ | -------- |
| Multi-Embedding  | Multi-model embedding + Qdrant per-tenant vector store | High     |
| Agent Chat       | Conversational AI agent with RAG context               | High     |
| Role-Based Auth  | JWT-based authentication with role permissions         | Medium   |
| Quality Pipeline | Automated QA generation from knowledge base            | Medium   |

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
