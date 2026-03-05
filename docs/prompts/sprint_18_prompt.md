# Sprint 18 Session Prompt — Coverage Analytics Dashboard
**Project:** Project Mimir
**Sprint:** 18
**Branch:** `feature/sprint-18-coverage-analytics`
**มาตรฐาน:** ISO/IEC 29110 + TDD (Test-Driven Development)
**Requirement:** REQ-012 (Coverage Intelligence — ACU per source, Blind-spot Detection, Closed-loop Actions)

---

## 🎯 Sprint Goal
สร้างหน้า Coverage Analytics `/coverage` ครบวงจร: แทน Coming Soon placeholder ด้วย Dashboard แสดงความครอบคลุมของข้อมูลในทุกมิติ (Source → Chunk → QA → Vector → KG) พร้อม Blind-spot Detection และ Gap Analysis

---

## 📋 ISO Requirement Traceability

### REQ-012 (SI-01 SRS — Sprint 12 scope)
> **Multi-Agent & Coverage Intelligence:** ระบบต้องมี Router Agent วิเคราะห์ query เพื่อเลือก tool, Tool Registry per tenant, **ACU per source**, **Blind-spot Detection** พร้อม **Closed-loop Actions**, LLM Usage Logging, LLM Analytics Dashboard, Web Hierarchy Loader

**สถานะปัจจุบัน:**
- ✅ LLM Usage Logging → `llm_usage.rs` (Sprint 12)
- ✅ LLM Analytics Dashboard → `analytics/llm/page.tsx` (Sprint 12)
- ✅ Web Hierarchy Loader → `sources.rs` discover-hierarchy (Sprint 12)
- ❌ ACU Coverage per source → **ยังไม่ implement**
- ❌ Blind-spot Detection → **ยังไม่ implement**
- ❌ Closed-loop Actions → **ยังไม่ implement**
- ❌ Coverage Analytics Page → **ยังเป็น Coming Soon placeholder**

### Sprint 7 Report (PM-02.7) — Previously Designed
> ปรับแต่ง Coverage Dashboard เพิ่มตัวเลข % Progress แบบวงกลมและ Blind-spot Highlighter

### Existing Infrastructure
| Component                    | Location                                | Data Available                                                                               |
| ---------------------------- | --------------------------------------- | -------------------------------------------------------------------------------------------- |
| Stats API                    | `routes/stats.rs` → `GET /api/v1/stats` | `total_sources`, `total_chunks`, `qa_pairs`, `vector_coverage`, `source_health`              |
| `data_sources` table         | MariaDB                                 | `id`, `tenant_id`, `name`, `source_type`, `last_sync_status`, `total_chunks`, `last_sync_at` |
| `chunks` table               | MariaDB                                 | `id`, `source_id`, `chunk_index`, `content`, `token_count`                                   |
| `pipeline_steps` table       | MariaDB                                 | `step_name` (qa_generation, embedding, etc.), `status`, `source_id`                          |
| `content_fingerprints` table | MariaDB                                 | `content_hash`, `source_id`, `chunk_id` — dedup data                                         |
| `kg_entities` table          | MariaDB                                 | KG entity counts per source                                                                  |
| `kg_extraction_runs` table   | MariaDB                                 | KG extraction status per source                                                              |

---

## 📋 Sprint 18 Features

### ═══════════════════════════════════════════════
### 🔵 Phase 1: Coverage API (Backend)
### ═══════════════════════════════════════════════

### 1.1 Coverage Stats API — Per-Source Breakdown
**Priority:** 🔴 Critical | **Type:** Feature | **Area:** Backend
**Scope:**
- สร้าง `routes/coverage.rs` + register ใน `mod.rs`:
  - `GET /api/v1/coverage/overview` → tenant-level coverage summary:
    ```json
    {
      "total_sources": 6,
      "sources_with_chunks": 4,
      "sources_with_qa": 2,
      "sources_with_vectors": 3,
      "sources_with_kg": 1,
      "overall_score": 67.5,
      "pipeline_stages": {
        "ingested": 6, "chunked": 4, "qa_generated": 2,
        "vectorized": 3, "kg_extracted": 1
      }
    }
    ```
  - `GET /api/v1/coverage/sources` → per-source coverage detail:
    ```json
    [
      {
        "source_id": 1,
        "name": "Medical FAQ",
        "source_type": "web",
        "status": "COMPLETED",
        "chunk_count": 150,
        "qa_count": 42,
        "vector_coverage_pct": 85.0,
        "kg_entity_count": 28,
        "dedup_ratio": 0.02,
        "blindspots": ["no_qa_pairs", "low_vector_coverage"],
        "coverage_score": 72.0,
        "last_sync_at": "2026-03-04T..."
      }
    ]
    ```
  - `GET /api/v1/coverage/gaps` → blind-spot analysis:
    ```json
    {
      "sources_missing_chunks": [...],
      "sources_missing_qa": [...],
      "sources_missing_vectors": [...],
      "sources_missing_kg": [...],
      "stale_sources": [...],
      "high_dedup_sources": [...]
    }
    ```
- ทุก endpoint ต้องผ่าน `extract_tenant_id` สำหรับ tenant isolation
- **Coverage Score Formula:** `(chunks > 0 ? 25 : 0) + (qa > 0 ? 25 : 0) + (vectors > 0 ? 25 : 0) + (kg > 0 ? 25 : 0)`
- **TDD:** test coverage calculation, test tenant isolation, test gap detection logic
- **Note:** Query from existing tables — no new migrations needed

---

### ═══════════════════════════════════════════════
### 🟢 Phase 2: Coverage Analytics Page (Frontend)
### ═══════════════════════════════════════════════

### 2.1 Coverage Analytics Dashboard
**Priority:** 🔴 Critical | **Type:** Feature | **Area:** Frontend
**Scope:**
- แทน `app/coverage/page.tsx` (Coming Soon placeholder) ด้วย Dashboard จริง:
  - **Header:** "Coverage Analytics" + BarChart3 icon + Refresh button
  - **KPI Cards** (4 cards, row บนสุด):
    - Source Coverage (% sources ที่มี chunks)
    - QA Coverage (% sources ที่มี QA pairs)
    - Vector Coverage (% sources ที่ vectorized)
    - KG Coverage (% sources ที่มี KG entities)
  - **Pipeline Flow Diagram** (horizontal bar showing stage progression):
    - `Ingested → Chunked → QA Generated → Vectorized → KG Extracted`
    - แต่ละ stage แสดง count + percentage bar
  - **Per-Source Coverage Table:**
    - Columns: Name | Type | Status | Chunks | QA | Vectors | KG | Score | Blind-spots
    - Score = coverage_score (color-coded: red < 25, amber < 50, yellow < 75, green ≥ 75)
    - Blind-spots = badge list (e.g., "No QA", "No Vectors")
    - Sortable by each column
  - **Gap Analysis Panel:**
    - Card showing count of sources per gap type
    - Click gap → filter table to show affected sources
  - ใช้ shadcn/ui `Card`, `Table`, `Badge` components — match app theme
  - Pull data จาก `GET /api/v1/coverage/overview` + `GET /api/v1/coverage/sources`
- **TDD:** test component renders, test data mapping

### 2.2 API Functions
**Priority:** 🔴 Critical | **Type:** Feature | **Area:** Frontend
**Scope:**
- เพิ่มใน `api.ts`:
  - `fetchCoverageOverview()` → coverage summary
  - `fetchCoverageSources()` → per-source detail
  - `fetchCoverageGaps()` → blind-spot analysis
- TypeScript interfaces: `CoverageOverview`, `SourceCoverage`, `CoverageGaps`

### 2.3 Navigation Update
**Priority:** 🟡 Medium | **Type:** Enhancement | **Area:** Frontend
**Scope:**
- Update `coverage/page.tsx` (แทน Coming Soon placeholder เป็น Dashboard จริง)
- Navbar link "Coverage" → BarChart3 icon (มีอยู่แล้ว)

---

## 🔄 ขั้นตอนการทำงาน (Workflow) — ทำตามลำดับนี้

### Phase 0: Planning & GitHub Setup
1. สร้าง branch `feature/sprint-18-coverage-analytics` จาก `feature/sprint-17-knowledge-graph`
2. สร้าง GitHub Issue สำหรับ Sprint 18
3. ตรวจสอบ baseline: `cargo check`, `cargo test`

### Phase 1: Implementation (TDD) — Backend
**ลำดับการทำงาน:**
```
🔴 Coverage API (ทำก่อน — Frontend ต้องใช้ API)
1. routes/coverage.rs   → Coverage endpoints (TDD: test queries, tenant isolation)
2. routes/mod.rs        → register coverage_routes
3. main.rs              → mount at /api/v1/coverage
```

**สำหรับแต่ละ Feature:**
1. **เขียน Test ก่อน** (Red) — สร้าง test cases ตาม scope
2. **Implement** (Green) — เขียน code ให้ tests ผ่าน
3. **Refactor** (Refactor) — ปรับปรุง code quality
4. **Verify** — `cargo test` / `cargo check`

### Phase 2: Implementation — Frontend
**ลำดับการทำงาน:**
```
🟢 Frontend (ทำหลัง API routes พร้อม)
4. api.ts              → Coverage API functions + TypeScript interfaces
5. coverage/page.tsx   → Coverage Analytics Dashboard (replace Coming Soon)
```

### Phase 3: Testing (ISO 29110 — SI-04)
1. **สร้าง Test Script** `docs/iso_29110/si/SI_04_18_Sprint18_TestScript.md`
2. **Execute Tests ตาม Script**

### Phase 4: ISO Documentation
1. **Update SI-02** — เพิ่ม Coverage Module description
2. **Update SI-03** — เพิ่ม traceability entry สำหรับ REQ-012 Coverage portion
3. **สร้าง PM-02.18** — Sprint 18 Status Report
4. **Update PM-01** — เพิ่ม Sprint 18 row

### Phase 5: Final Verification & Push
1. รัน full test suite:
   ```bash
   cargo test -p mimir-core-ai 2>&1 | tail -5
   cargo test -p ro-ai-bridge -- coverage 2>&1 | tail -5
   cargo check 2>&1 | tail -5
   ```
2. Commit all changes
3. Push + Create PR

---

## 📁 Files to Create/Modify

### New Files (Backend)
```
ro-ai-bridge/src/routes/coverage.rs               — Coverage API routes
```

### Modified Files (Backend)
```
ro-ai-bridge/src/routes/mod.rs                     — register coverage_routes
ro-ai-bridge/src/main.rs                           — mount /api/v1/coverage
```

### Modified Files (Frontend)
```
ro-ai-dashboard/src/lib/api.ts                     — Coverage API functions
ro-ai-dashboard/src/app/coverage/page.tsx           — Replace Coming Soon → Dashboard
```

### ISO Documents
```
docs/iso_29110/si/SI_04_18_Sprint18_TestScript.md   — NEW
docs/iso_29110/pm/PM_02_18_Sprint18_Report.md       — NEW
docs/iso_29110/si/SI_02_Software_Design_Document.md — UPDATE
docs/iso_29110/si/SI_03_Traceability_Matrix.md      — UPDATE
docs/iso_29110/pm/PM_01_Project_Plan.md             — UPDATE
docs/prompts/sprint_18_prompt.md                    — THIS FILE
```

---

## ⚠️ Important Notes
- **Branch:** `feature/sprint-18-coverage-analytics` จาก `feature/sprint-17-knowledge-graph`
- **TDD:** เขียน test ก่อน implement ทุก feature
- **ISO:** ทุก feature ต้องมี test case ใน SI-04, traceable ใน SI-03
- **Commit Convention:** `feat(#xxx): <description>`
- **No New Migrations:** Coverage queries aggregate from existing tables (`data_sources`, `chunks`, `pipeline_steps`, `content_fingerprints`, `kg_entities`, `kg_extraction_runs`)
- **Tenant Isolation:** ทุก query ต้องมี `WHERE tenant_id = $tenant_id`
- **Design System:** ใช้ shadcn/ui `Card`, `Table`, `Badge` ตาม theme ของ app (ดูตัวอย่าง `analytics/llm/page.tsx`)

---

## 📊 Sprint Summary

| Category      | Count | Items                                |
| ------------- | ----- | ------------------------------------ |
| 🔴 Backend API | 1     | Coverage routes (3 endpoints, tests) |
| 🟢 Frontend    | 2     | Coverage page, API functions         |
| 📝 ISO Docs    | 4     | SI-04, PM-02, SI-02, SI-03 updates   |
| **Total**     | **7** |                                      |

**Estimated Effort:**
- Phase 1 (Backend): ~1 session
- Phase 2 (Frontend): ~1-2 sessions
- Testing + ISO: ~1 session
- **Total: ~3-4 sessions**
