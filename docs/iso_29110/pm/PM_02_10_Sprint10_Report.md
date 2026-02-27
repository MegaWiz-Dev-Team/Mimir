# PM-02.10: Sprint 10 Status Report (Dashboard Redesign & Knowledge Base)

**Project Name:** Project Mimir
**Sprint:** Sprint 10
**Status:** In Progress
**Date:** 2026-02-27

---

## 1. ขอบเขตของ Sprint 10 (Sprint Scope)
- **Backend:** Stats API (`GET /api/v1/stats`) — aggregated dashboard statistics
- **Backend:** Sync All Sources (`POST /api/v1/sources/sync-all`) — batch sync endpoint
- **Backend:** Chunks API (`GET /api/v1/chunks`, `GET /api/v1/chunks/:id`) — paginated, searchable chunk browser
- **Frontend:** Dashboard Redesign (#115) — KPI cards, Recent Activity, Source Health donut, Pipeline Status table, Quick Actions
- **Frontend:** Knowledge Base Page (#116) — chunk browser with search, filter, pagination, detail modal
- **Frontend:** Search Settings Tab (#117) — Embedding Model, Top-K, Similarity Threshold, Search Mode
- **Frontend:** Dashboard UX Fixes (#118) — KPI fallback data, redundant pills removed, language standardization
- **Frontend:** Pipeline Status Table UX (#119) — icon alignment, empty state icons, clickable source names, type badges, summary footer
- **Frontend:** Global Pipeline Status Bar (#120) — consistent stage names, real-time data sync from sources API
- **Frontend:** Tenant dropdown fix — replaced hardcoded mock data with `fetchTenants()` API
- **Frontend:** Vector Coverage fix — corrected calculation to 0% (vectorization not yet implemented)

## 2. สรุปผลการทดสอบ (Testing Verification Summary)
อ้างอิงจากแผนการทดสอบ `SI_04_10_Sprint10_TestScript.md`:

### Unit/API Tests (5/5 Pass)
| ID         | Description                     | Result |
| ---------- | ------------------------------- | ------ |
| TC_SP10_U1 | Stats API endpoint              | ✅ Pass |
| TC_SP10_U2 | Sync All Sources endpoint       | ✅ Pass |
| TC_SP10_U3 | Chunks List API (search/filter) | ✅ Pass |
| TC_SP10_U4 | Chunk Detail API                | ✅ Pass |
| TC_SP10_U5 | Frontend Build — All Routes     | ✅ Pass |

### Dashboard UI Tests (12/12 Pass)
| ID         | Description                             | Result |
| ---------- | --------------------------------------- | ------ |
| TC_SP10_01 | KPI Cards with fallback data            | ✅ Pass |
| TC_SP10_02 | Recent Activity (English language)      | ✅ Pass |
| TC_SP10_03 | Source Health donut chart               | ✅ Pass |
| TC_SP10_04 | Quick Actions (3 buttons)               | ✅ Pass |
| TC_SP10_05 | Redundant pills removed                 | ✅ Pass |
| TC_SP10_06 | Global Bar — consistent stage names     | ✅ Pass |
| TC_SP10_07 | Global Bar — data matches KPI           | ✅ Pass |
| TC_SP10_08 | Pipeline Table — icon alignment         | ✅ Pass |
| TC_SP10_09 | Pipeline Table — empty state icons      | ✅ Pass |
| TC_SP10_10 | Pipeline Table — clickable source names | ✅ Pass |
| TC_SP10_11 | Pipeline Table — colored type badges    | ✅ Pass |
| TC_SP10_12 | Pipeline Table — summary footer         | ✅ Pass |

### Feature Tests (7/7 Pass)
| ID         | Description                        | Result |
| ---------- | ---------------------------------- | ------ |
| TC_SP10_13 | Knowledge Base page renders        | ✅ Pass |
| TC_SP10_14 | Knowledge — chunk table with data  | ✅ Pass |
| TC_SP10_15 | Knowledge — search & source filter | ✅ Pass |
| TC_SP10_16 | Search Settings — no placeholder   | ✅ Pass |
| TC_SP10_17 | Search Settings — all form fields  | ✅ Pass |
| TC_SP10_18 | Tenant dropdown — real API data    | ✅ Pass |
| TC_SP10_19 | Vector Coverage — correct 0%       | ✅ Pass |

**Total: 24/24 (100%)**

## 3. GitHub Synchronization & Traceability
### Issues
| Issue # | Title                                                                        | Status  |
| ------- | ---------------------------------------------------------------------------- | ------- |
| #115    | Feat: Redesign Overview page — Knowledge Hub Dashboard                       | 🔧 Fixed |
| #116    | Feat: Knowledge Base Page — chunk browser with search & filter               | 🔧 Fixed |
| #117    | Feat: Search Settings Tab — embedding model, top-k, similarity threshold     | 🔧 Fixed |
| #118    | Fix: Dashboard UX — KPI fallback, redundant pills, mixed language            | 🔧 Fixed |
| #119    | Fix: Pipeline Status Table — alignment, empty states, clickable source names | 🔧 Fixed |
| #120    | Fix: Global Pipeline Status bar — data mismatch, inconsistent stage names    | 🔧 Fixed |
| #114    | Test: E2E browser testing for Add Source wizard                              | 🔓 Open  |

### Pull Requests
| PR # | Title                          | Status  |
| ---- | ------------------------------ | ------- |
| —    | Sprint 10 changes (pending PR) | Pending |

## 4. รายละเอียดการเปลี่ยนแปลง (Changes Detail)

### Backend (Rust)
1. **`routes/stats.rs`** — New module: `GET /api/v1/stats` aggregating total_sources, total_chunks, qa_pairs, vector_coverage, source_health
2. **`routes/chunks.rs`** — New module: `GET /api/v1/chunks` (filterable, searchable, paginated) + `GET /api/v1/chunks/:id`
3. **`routes/sources.rs`** — Added `POST /sync-all` endpoint
4. **`main.rs` + `routes/mod.rs`** — Registered new route modules

### Frontend (Next.js)
1. **`src/app/page.tsx`** — Complete rewrite: KPI cards with fallback, removed color pills, Vector Coverage = 0%
2. **`src/components/dashboard/DashboardStats.tsx`** — KPI cards component
3. **`src/components/dashboard/RecentActivity.tsx`** — Activity feed (fixed Thai→English language)
4. **`src/components/dashboard/SourceHealth.tsx`** — Donut chart with recharts
5. **`src/components/dashboard/PipelineStatusTable.tsx`** — Rewritten: center-aligned icons, gray ○ for pending, 🔒 for locked, clickable names, type badges, summary
6. **`src/components/dashboard/QuickActions.tsx`** — Add Source, Sync All, Open Playground
7. **`src/components/pipeline-status-bar.tsx`** — Rewritten: consistent stage names (Sources→Ingested→Chunked→QA Ready→Vectorized), real data from sources API
8. **`src/components/navbar.tsx`** — Tenant dropdown now uses `fetchTenants()` API instead of hardcoded values
9. **`src/app/knowledge/page.tsx`** — New: Knowledge Base page with chunk browser
10. **`src/app/settings/page.tsx`** — Search tab with Embedding Model, Top-K, Similarity Threshold, Search Mode
11. **`src/lib/api.ts`** — Added fetchStats, syncAllSources, fetchChunks, fetchChunk functions

## 5. ปัญหาที่พบและวิธีแก้ไข (Issues & Resolutions)
1. **KPI Cards showing "—" (Issue #118):**
   - *ปัญหา:* stats API ยังไม่พร้อม ทำให้ KPI แสดง "—" ทั้งที่มี sources
   - *แก้ปัญหา:* เพิ่ม fallback logic ใน `useMemo` — คำนวณจาก sources data โดยตรง

2. **Global Pipeline Status bar data mismatch (Issue #120):**
   - *ปัญหา:* Bar แสดง "SOURCES 3 items" ไม่ตรงกับ KPI (4), ชื่อ stage คนละชุดกับ table
   - *แก้ปัญหา:* Rewrite bar ให้ใช้ `fetchSources()` เดียวกัน, ชื่อ stage ตรงกัน (Sources→Ingested→Chunked→QA Ready→Vectorized)

3. **Tenant dropdown hardcoded (No issue):**
   - *ปัญหา:* Mock tenants (Ragnarok TH, Medical Clinic A) ไม่มีอยู่จริง
   - *แก้ปัญหา:* ใช้ `fetchTenants()` API แทน hardcoded array

4. **Vector Coverage 25% incorrect (No issue):**
   - *ปัญหา:* คำนวณเป็น sourcesWithChunks/totalSources — ไม่ใช่ vector coverage จริง
   - *แก้ปัญหา:* ตั้งเป็น 0% เพราะ vectorization ยังไม่ implement

## 6. Sprint 11 Planning
| Issue # | Title                                                   | Priority |
| ------- | ------------------------------------------------------- | -------- |
| #114    | E2E browser testing for Add Source wizard (all options) | High     |
| —       | Knowledge Graph settings implementation                 | Medium   |
| —       | Coverage Analytics page                                 | Medium   |
| —       | Search settings persistence (backend)                   | Low      |

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
