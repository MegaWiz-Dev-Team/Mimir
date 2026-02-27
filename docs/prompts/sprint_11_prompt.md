# Sprint 11 Session Prompt — Data Ingress Enhancement & LLM Extraction

## 📋 Context
Project Mimir คือระบบ AI Knowledge Management Platform ที่ใช้ Rust backend (axum + sqlx + MariaDB) กับ Next.js frontend

### Architecture (หลัง Sprint 10)
- **Unified Data Pipeline:** Sources → Extraction → Chunking → Dedup → (QA Gen) → (Vector)
- **Multi-source:** File Upload (PDF/CSV/XLSX/DOCX/TXT/MD/JSON), Web Scraper, MCP Connection
- **Frontend:** 7-item nav (Overview, Sources, Knowledge, Quality, Playground, Coverage, Admin)
- **Dashboard:** Redesigned — KPI Cards (fallback), Source Health donut, Pipeline Status table, Quick Actions
- **Settings:** Tabbed interface — General, AI Models, Pipeline, Knowledge Graph, Search, Security

### Tech Stack
- **Backend:** Rust (axum, sqlx, argon2, mimir-core-ai crate) — port 8080
- **Frontend:** Next.js 15 (Turbopack) — port 3000
- **DB:** MariaDB (docker: mimir_mariadb)
- **Vector:** Qdrant (not yet integrated)
- **Storage:** RustFS/MinIO (S3-compatible)
- **Auth:** JWT (`admin` / `admin123`)

### Known Issues จาก Sprint 10
- Vector Coverage = 0% (vectorization ยังไม่ implement)
- Tenant dropdown ดึงจาก API แล้ว (ไม่ hardcode)
- QA/Vector stages แสดง 🔒 lock icon ใน Pipeline table

---

## 🎯 Sprint 11 Issues

### Issue #122: CSV Upload Sync Fails — "Unsupported source type: file" 🔴 Critical
**Priority: Critical** | Label: `sprint-11`, `bug`

Sync route ไม่ handle `source_type = "file"` — ต้องดาวน์โหลดจาก S3 ก่อนส่งเข้า extraction

**Root Cause:** `sync_source()` เรียก `process_source()` ซึ่ง match `"file"` → error
**Fix:** sync route ต้อง download จาก RustFS/S3 แล้วใช้ `process_source_with_data()`

**Backend:**
- [ ] `routes/sources.rs` — `sync_source()` เพิ่ม S3 download สำหรับ file/document/tabular
- [ ] ใช้ `s3_key` ที่บันทึกตอน upload → download bytes → `process_source_with_data()`
- [ ] Unit test: sync file source with S3 data → success

---

### Issue #123: Ingress Live Console — Hardcoded Fake Logs 🔴 High
**Priority: High** | Label: `sprint-11`, `bug`

Log messages ใน Ingress Console เป็น `setTimeout` hardcoded ไม่ตรงกับ pipeline จริง

**ปัญหา:**
- "Fetching URL..." แสดงทุก source type (แม้ File upload)
- "Parsing DOM..." แสดงสำหรับ CSV/PDF
- "Check Vector space" — Vector ยังไม่ implement

**Fix Phase 1 (Quick):**
- [ ] อ่าน `source.source_type` แล้วแสดงข้อความที่ตรง (Web/File/MCP)
- [ ] ลบ "Check Vector space" → ใช้ "Completed!"
- [ ] แสดง error ถ้า sync fail
- [ ] แสดง chunk count + dedup count หลัง sync เสร็จ

**Fix Phase 2 (Future):**
- [ ] Backend ส่ง SSE/WebSocket log events
- [ ] Frontend subscribe real-time

---

### Issue #121: File Upload Wizard — Cannot Remove Selected Files 🟡
**Priority: Medium** | Label: `sprint-11`, `enhancement`

เลือกไฟล์แล้วไม่มีทางลบออก — แสดงแค่ "2 file(s) selected" เป็น plain text

**Frontend:**
- [ ] แสดงรายชื่อไฟล์ + ขนาด (e.g., "report.csv — 1.2 MB")
- [ ] ปุ่ม ✕ ข้างแต่ละไฟล์ลบทีละตัว
- [ ] ปุ่ม "Clear all" ลบทั้งหมด
- [ ] Count อัปเดตหลังลบ

---

### Issue #124: Support Legacy Office Formats (.doc, .xls, .ppt) 🟡
**Priority: Medium** | Label: `sprint-11`, `enhancement`

Frontend ไม่ accept + Backend ไม่มี parser สำหรับ legacy Office binary formats

**Frontend:**
- [ ] เพิ่ม MIME types: `application/msword`, `application/vnd.ms-excel`, `application/vnd.ms-powerpoint` ใน `upload-dropzone.tsx`
- [ ] อัปเดต description text

**Backend (2 options):**
- **Option A:** LibreOffice headless (`soffice --convert-to`) แปลงเป็น modern format ก่อน extract
- **Option B:** Rust crates — `calamine` (.xls), `doc-rs` (.doc)
- [ ] เพิ่ม extraction handlers ใน `extraction.rs`

---

### Issue #125: LLM Fallback Extraction — AI-Powered Document Reading 🟡
**Priority: Medium** | Label: `sprint-11`, `enhancement`

เมื่อ native extraction fail → ให้ผู้ใช้ใช้ LLM อ่านไฟล์แทน พร้อมเลือก Model + Output Format

**User Flow:**
1. Upload ไฟล์ → Extraction fail → raw_markdown แสดง error
2. หน้า Markdown Preview แสดงปุ่ม "🤖 Extract with AI"
3. เลือก Model (gpt-4o, gpt-4o-mini, gemini-2.0-flash, custom)
4. เลือก Format (Markdown / Table)
5. กด Extract → LLM อ่านไฟล์ → แสดง preview → edit ได้ → Save

**Backend:**
- [ ] `POST /api/v1/sources/:id/extract-ai` — Send file to LLM
- [ ] Support Vision API สำหรับ image-only PDF, scanned docs
- [ ] ใช้ tenant API key จาก Settings > AI Models

**Frontend:**
- [ ] "Extract with AI" section ใน Markdown Preview dialog
- [ ] Model selector dropdown
- [ ] Output format toggle (Markdown / Table)
- [ ] Editable preview + Save button
- [ ] Token usage display

---

### Issue #114: E2E Browser Testing — Add Source Wizard (Carryover)
**Priority: High** | Label: `sprint-10` → carryover

ทดสอบ wizard ผ่าน browser ทุก option:
- File Upload (PDF, CSV markdown/SQL, XLSX, folder)
- Web Scraper (URL → sync → verify)
- MCP Connection (endpoint → save)
- Validation (>50MB, .exe, corrupted, duplicate)
- Wizard UI (step progression, back, storage mode, advanced settings)

---

## 📁 Key Files Reference

### Backend
- `ro-ai-bridge/src/routes/sources.rs` — Sources API + sync_source (🔴 #122 fix here)
- `ro-ai-bridge/mimir-core-ai/src/services/ingress.rs` — Pipeline processing
- `ro-ai-bridge/mimir-core-ai/src/services/extraction.rs` — File extraction (🟡 #124, #125)
- `ro-ai-bridge/mimir-core-ai/src/services/chunking.rs` — Chunking service
- `ro-ai-bridge/mimir-core-ai/src/services/dedup.rs` — Dedup service
- `ro-ai-bridge/mimir-core-ai/migrations/` — DB migrations

### Frontend
- `ro-ai-dashboard/src/app/sources/page.tsx` — Sources page + wizard + handleSync (🔴 #123)
- `ro-ai-dashboard/src/components/upload-dropzone.tsx` — File upload (🟡 #121, #124)
- `ro-ai-dashboard/src/components/advanced-settings.tsx` — Wizard step 3
- `ro-ai-dashboard/src/app/page.tsx` — Dashboard overview
- `ro-ai-dashboard/src/app/knowledge/page.tsx` — Knowledge Base
- `ro-ai-dashboard/src/app/settings/page.tsx` — Settings tabs
- `ro-ai-dashboard/src/lib/api.ts` — API client functions

### ISO Docs
- `docs/iso_29110/pm/PM_02_10_Sprint10_Report.md` — Sprint 10 report
- `docs/iso_29110/si/SI_04_10_Sprint10_TestScript.md` — Sprint 10 test script

---

## ⚙️ Dev Environment Setup
```bash
# Start services
docker compose up -d
cd ro-ai-bridge && cargo run --bin ro-ai-bridge  # Backend :8080
cd ro-ai-dashboard && npm run dev                 # Frontend :3000

# Login: admin / admin123
```

## 📐 Rules
1. **TDD approach** — write tests first, then implement
2. **ISO 29110** — create test script SI-04-11 and sprint report PM-02-11
3. **PR per issue** — one branch + PR per issue
4. **Commit messages** — `feat:`, `fix:`, `test:`, `docs:` prefixes
5. Follow existing code style and patterns in the codebase
6. **Priority order:** #122 (Critical) → #123 (High) → #121 → #124 → #125

## 🗂️ Sprint 11 Summary Table

| #        | Title                                     | Type          | Priority |
| -------- | ----------------------------------------- | ------------- | -------- |
| **#122** | CSV sync fails — missing S3 download      | 🔴 Bug         | Critical |
| **#123** | Ingress Console — fake hardcoded logs     | 🔴 Bug         | High     |
| **#114** | E2E browser testing wizard (carryover)    | 🧪 Test        | High     |
| **#121** | File upload — cannot remove files         | 🟡 Enhancement | Medium   |
| **#124** | Legacy Office formats (.doc, .xls, .ppt)  | 🟡 Enhancement | Medium   |
| **#125** | LLM fallback extraction + model selection | 🟡 Enhancement | Medium   |
