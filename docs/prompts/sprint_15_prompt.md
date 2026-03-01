# Sprint 15 Session Prompt — Bug Fixes & Data Ingress Hardening + Dataset Studio
**Project:** Project Mimir
**Sprint:** 15 (Week 17-18)
**Branch:** `feat/sprint-15`
**มาตรฐาน:** ISO/IEC 29110 + TDD (Test-Driven Development)

---

## 🎯 Sprint Goal
แก้ bugs ที่ค้างจาก Sprint 14b (Data Ingress, Admin, Knowledge Base) + ปรับปรุง UX (Source grouping, Provider selection, Pipeline nav) + เริ่ม Dataset Studio Module

---

## 📋 Sprint 15 Issues (11 ข้อ — จัดเรียงตาม priority)

### ═══════════════════════════════════════════════
### 🔴 Phase A: Critical Bug Fixes (ต้องแก้ก่อน)
### ═══════════════════════════════════════════════

### A1. JWT Expiry — Admin Settings Error (#165)
**Priority:** 🔴 Critical | **Type:** Bug | **Area:** Backend + Frontend
**ปัญหา:** เข้าหน้า Admin ได้ alert "Failed to load tenant data" เพราะ JWT หมดอายุ 15 นาที
**Scope:**
- Backend: ขยาย JWT expiry จาก 15 min → 24 hr (`iam.rs:86`)
- Frontend: แทน `alert()` ด้วย graceful redirect to `/login` (`settings/page.tsx:103`)
- Backend: ทุก auth-required endpoint ต้อง return 401 consistently (ไม่ใช่ empty 200)
- TDD: test JWT expiry, test 401 response consistency

### A2. File Source Sync Fails — No S3 Key (#168)
**Priority:** 🔴 Critical | **Type:** Bug | **Area:** Backend + Frontend
**ปัญหา:** Add source แบบ File (PDF) → Sync → "No S3 key found"
**Scope:**
- ตรวจสอบ upload flow: frontend → S3 (RustFS) → save s3_key ใน `data_sources`
- แก้ให้ file upload → save S3 → create source record ที่มี s3_key
- ถ้า upload fail → แจ้ง error ตอน add ไม่ใช่ตอน sync
- TDD: test upload + source creation flow, test S3 key persistence

### A3. Folder Upload Fails + .DS_Store (#170)
**Priority:** 🔴 Critical | **Type:** Bug | **Area:** Backend + Frontend
**ปัญหา:** Upload folder → same S3 key error + `.DS_Store` ไม่ถูกกรอง
**Scope:**
- Frontend: กรอง hidden files (`.DS_Store`, `.gitkeep`, `Thumbs.db`, ไฟล์ขึ้นต้นด้วย `.`)
- Backend: multi-file upload → สร้าง 1 source ที่มีหลาย files (save ทุกไฟล์ขึ้น S3)
- TDD: test file filter, test multi-file S3 upload
- **ทำหลัง A2** — share root cause เดียวกัน

### A4. MCP Source Missing URL Input (#169)
**Priority:** 🔴 Critical | **Type:** Bug | **Area:** Frontend
**ปัญหา:** Add Source wizard Step 3 ไม่มี field ให้ใส่ MCP Server URL
**Scope:**
- Frontend: เพิ่ม MCP URL input (required) + Transport type (SSE/Stdio) ใน Step 2 หรือ 3
- Backend: save `mcp_url` ลง `config_json` ของ `data_sources`
- TDD: test MCP source creation with URL, test sync reads mcp_url

### A5. External DB — Test Connection Fails (#171)
**Priority:** 🔴 Critical | **Type:** Bug | **Area:** Backend
**ปัญหา:** External DB → Test Connection → "Failed to fetch"
**Scope:**
- ตรวจสอบ endpoint `/api/v1/db-connectors/test` (หรือ endpoint จริง)
- แก้ connection string parsing (mysql:// vs mariadb://)
- ตรวจสอบ Docker network (localhost vs container hostname)
- TDD: test DB connection with valid/invalid connection strings

### A6. Knowledge Base Shows 0 Chunks (#172)
**Priority:** 🔴 Critical | **Type:** Bug | **Area:** Frontend
**ปัญหา:** Pipeline bar แสดง 585 chunks แต่หน้า Knowledge แสดง 0
**Scope:**
- ตรวจสอบ Knowledge page API call vs Pipeline bar API call
- แก้ query ให้ตรงกัน (อาจ missing tenant_id)
- TDD: test chunks API endpoint with tenant filter

---

### ═══════════════════════════════════════════════
### 🟡 Phase B: UX Improvements
### ═══════════════════════════════════════════════

### B1. Pipeline Status Bar Navigation (#174)
**Priority:** 🟡 Medium | **Type:** Bug/UX | **Area:** Frontend
**ปัญหา:** กด DEDUP ใน Pipeline bar แล้วไปหน้า Sources แทน
**Scope:**
- SOURCES → `/sources`
- CHUNKS → `/knowledge`
- DEDUP → `/knowledge?filter=dedup`
- QA → `/quality`
- VECTOR → `/knowledge?tab=vectors`
- TDD: test navigation links

### B2. Sources Group by Type (#167)
**Priority:** 🟡 Medium | **Type:** Enhancement | **Area:** Frontend
**ปัญหา:** Sources เยอะ flat list ดูยาก
**Scope:**
- จัดกลุ่ม sources ตาม type: 📁 File, 🌐 Web, 🔌 MCP, 🗄️ DB
- Collapsible sections + จำนวนต่อกลุ่ม
- จำ collapse state ไว้ใน localStorage
- Apply กับทั้ง Sources page + Pipeline Status (per source) table ที่ Overview
- TDD: test grouping logic, test collapse state persistence

### B3. Provider/Model Dropdown + Dynamic Discovery (#175)
**Priority:** 🟡 Medium | **Type:** Enhancement | **Area:** Frontend + Backend
**ปัญหา:** Provider/Model เป็น text input → ไม่รู้ว่าเลือกอะไรได้
**Scope:**
- Phase 1: Provider dropdown (Ollama/Gemini/OpenAI/MLX/vLLM/Groq/DeepSeek/Custom)
- Phase 1: Suggested model list per provider (hardcode defaults)
- Phase 2: "Fetch Models" button → เรียก `/v1/models` ของ provider
- Phase 2: Fallback text input ถ้า fetch ไม่ได้
- TDD: test provider config, test model fetch endpoint

### B4. QA UX — Manual Trigger at Knowledge Page (#166, #173)
**Priority:** 🟡 Medium | **Type:** Enhancement | **Area:** Frontend + Backend
**ปัญหา:** QA flow สับสน → ควร manual trigger ที่ Knowledge page
**Scope:**
- ปุ่ม "Generate QA" บนหน้า Knowledge Base
- เลือก scope: All chunks / per source / selected chunks
- แสดง QA status per chunk (มี QA pairs กี่ข้อ)
- ปุ่ม "Run QC Scan" สำหรับ clustering check
- QA source card badge + one-click QA button
- **ต้องทำหลัง A6** (Knowledge page ต้อง show chunks ก่อน)
- TDD: test QA generation endpoint, test QA status display

---

## 🔄 ขั้นตอนการทำงาน (Workflow) — ทำตามลำดับนี้

### Phase 1: Planning & GitHub Setup
1. สร้าง branch `feat/sprint-15` จาก `feat/sprint-14b-deploy`
2. Label issues #165-#175 เป็น `sprint-15`
3. ตรวจสอบ baseline: `cargo test -p mimir-core-ai` (255 tests), `npx next build`

### Phase 2: Implementation (TDD) — Bug Fixes First!

**ลำดับลำดับแนะนำ:**
```
🔴 Bug Fixes (ทำก่อน — ลำดับ dependency)
1. #165  JWT Expiry         → quick fix, ปลดล็อก Admin ใช้งานได้
2. #172  Knowledge 0 chunks → ปลดล็อกหน้า Knowledge ใช้งานได้
3. #168  PDF upload S3 key  → แก้ File source ให้ ingest ได้
4. #170  Folder + .DS_Store → ต่อเนื่องจาก #168 (same root cause)
5. #169  MCP URL input      → แก้ MCP source ให้ sync ได้
6. #171  External DB test   → แก้ DB connector ให้ test ได้

🟡 UX Improvements (ทำหลัง bugs หมด)
7. #174  Pipeline nav       → quick fix, ไม่กระทบ UI มาก
8. #167  Sources group      → frontend only, ไม่กระทบ backend
9. #175  Provider dropdown  → ปรับ Settings UI
10. #166+#173 QA at Knowledge → ต้อง Knowledge page work ก่อน (#172)
```

**สำหรับแต่ละ Issue:**
1. **เขียน Test ก่อน** (Red) — สร้าง test cases ตาม Acceptance Criteria
2. **Implement** (Green) — เขียน code ให้ tests ผ่าน
3. **Refactor** (Refactor) — ปรับปรุง code quality
4. **Verify** — `cargo test` / browser test / manual verify
5. **Commit** — `fix(#xxx): <description>` หรือ `feat(#xxx): <description>`

### Phase 3: Testing (ISO 29110 — SI-04)
1. **สร้าง Test Script** `docs/iso_29110/si/SI_04_15_Sprint15_TestScript.md`
   - ใช้ format เดียวกับ SI-04-14b
   - ตารางทดสอบแบ่งเป็น:
     - ส่วนที่ 1: Bug Fix Tests — JWT, S3 upload, MCP, DB connector, Knowledge (#165-#172)
     - ส่วนที่ 2: UX Tests — Pipeline nav, Source grouping, Provider dropdown (#167, #174, #175)
     - ส่วนที่ 3: QA Feature Tests — Knowledge page QA trigger (#166, #173)
   - ทุกข้อต้องมี: ID, Scenario, Steps, Expected, Result, Issue#/PR#, หมายเหตุ

2. **Execute Tests ตาม Script** — ทดสอบทีละข้อตามลำดับ
   - รัน `cargo test` → บันทึกผล
   - ทดสอบ browser (upload, MCP, DB, Knowledge, Pipeline nav) → screenshot
   - บันทึกทุกข้อเป็น ✅ Pass / ❌ Fail / ⏳ Pending

### Phase 4: ISO Documentation
1. **Update SI-02** (Software Design Document)
   - เพิ่ม Sprint 15 subsystem description

2. **Update SI-03** (Traceability Matrix)
   - เพิ่ม entries สำหรับ 11 issues ของ Sprint 15

3. **สร้าง PM-02.15** (Sprint Report) `docs/iso_29110/pm/PM_02_15_Sprint15_Report.md`
   - Sprint Scope, Testing Summary, GitHub Issues & PRs, Changes Detail

4. **Update PM-02** (Main Status Reports)
   - เพิ่ม Sprint 15 row

### Phase 5: Final Verification & Push
1. รัน full test suite:
   ```bash
   cargo test -p mimir-core-ai 2>&1 | tail -5
   cd ro-ai-dashboard && npx next build 2>&1 | tail -10
   ```
2. Commit all changes
3. Push to `feat/sprint-15`
4. สร้าง PR → merge to main

---

## 📁 Files to Create/Modify

### Modified Files (Bug Fixes)
```
ro-ai-bridge/mimir-core-ai/src/services/iam.rs         — JWT expiry (#165)
ro-ai-dashboard/src/app/settings/page.tsx               — alert → redirect (#165), Provider dropdown (#175)
ro-ai-dashboard/src/lib/api.ts                          — authFetch error handling (#165)
ro-ai-bridge/src/routes/sources.rs                      — S3 upload fix (#168, #170), MCP config (#169)
ro-ai-dashboard/src/app/sources/page.tsx                — file filter (#170), MCP URL (#169), grouping (#167)
ro-ai-bridge/src/routes/db_connector.rs                 — test connection fix (#171)
ro-ai-dashboard/src/app/knowledge/page.tsx              — chunks query fix (#172), QA button (#173)
ro-ai-dashboard/src/components/PipelineStatusBar.tsx    — navigation links (#174)
```

### New Files
```
(ถ้าต้องการ migration)
mimir-core-ai/migrations/XXXXXX_sprint15_fixes.sql
```

### ISO Documents
```
docs/iso_29110/si/SI_04_15_Sprint15_TestScript.md       — NEW
docs/iso_29110/pm/PM_02_15_Sprint15_Report.md           — NEW
docs/iso_29110/si/SI_02_Software_Design_Document.md     — UPDATE
docs/iso_29110/si/SI_03_Traceability_Matrix.md          — UPDATE
docs/iso_29110/pm/PM_02_Status_Reports.md               — UPDATE
```

---

## ⚠️ Important Notes
- **Branch:** สร้าง `feat/sprint-15` จาก `feat/sprint-14b-deploy`
- **Bug First:** แก้ 🔴 Critical bugs ก่อน → แล้วค่อยทำ 🟡 UX enhancements
- **TDD:** เขียน test ก่อน implement ทุก issue
- **ISO:** ทุก issue ต้องมี test case ใน SI-04, traceable ใน SI-03
- **Commit Convention:** `fix(#xxx): <description>` (bugs) / `feat(#xxx): <description>` (enhancements)
- **Sprint 14b Baseline:** 255/255 backend tests, 44/44 feature tests, 0 errors

---

## 📊 Sprint Summary

| Category          | Count  | Issues                             |
| ----------------- | ------ | ---------------------------------- |
| 🔴 Critical Bugs   | 6      | #165, #168, #169, #170, #171, #172 |
| 🟡 UX Improvements | 4      | #166, #167, #174, #175             |
| 🟡 New Feature     | 1      | #173 (QA at Knowledge)             |
| **Total**         | **11** |                                    |

**Estimated Effort:**
- Bug Fixes: ~3-4 sessions
- UX Improvements: ~2-3 sessions
- ISO Documentation: ~1 session
- **Total: ~6-8 sessions**
