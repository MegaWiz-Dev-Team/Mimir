# Sprint 11B Session Prompt — LLM Fallback Extraction, Merge PRs & ISO Docs

## 📋 Context
Project Mimir คือระบบ AI Knowledge Management Platform ที่ใช้ Rust backend (axum + sqlx + MariaDB) กับ Next.js frontend

### Architecture (หลัง Sprint 10)
- **Unified Data Pipeline:** Sources → Extraction → Chunking → Dedup → (QA Gen) → (Vector)
- **Multi-source:** File Upload (PDF/CSV/XLSX/DOCX/DOC/PPTX/PPT/TXT/MD/JSON), Web Scraper, MCP Connection
- **Frontend:** 7-item nav (Overview, Sources, Knowledge, Quality, Playground, Coverage, Admin)
- **Settings:** Tabbed interface — General, AI Models, Pipeline, Knowledge Graph, Search, Security

### Tech Stack
- **Backend:** Rust (axum, sqlx, argon2, mimir-core-ai crate) — port 8080
- **Frontend:** Next.js 15 (Turbopack) — port 3000
- **DB:** MariaDB (docker: mimir_mariadb)
- **Storage:** RustFS/MinIO (S3-compatible)
- **Auth:** JWT (`admin` / `admin123`)

---

## ✅ Sprint 11A — Completed (Previous Session)

PRs พร้อม merge:

| PR       | Issue | Title                                                              | Status  |
| -------- | ----- | ------------------------------------------------------------------ | ------- |
| **#126** | #122  | fix: handle 'file' source_type in sync_source + extraction router  | ✅ Ready |
| **#127** | #123  | fix: replace hardcoded console logs with source-type-aware polling | ✅ Ready |
| **#128** | #121  | feat: add file list with remove buttons to upload wizard           | ✅ Ready |
| **#129** | #124  | feat: support legacy Office formats .doc/.xls/.ppt/.pptx           | ✅ Ready |

### สิ่งที่ทำแล้ว:
1. **#122:** เพิ่ม `"file"` ใน S3 download match arm ของ `sync_source()` + เพิ่ม `"file"` ใน extraction router + 2 unit tests
2. **#123:** เปลี่ยน `handleSync()` จาก `setTimeout` hardcoded logs เป็น polling ทุก 2s + แสดงข้อความตาม source type (Web/File/MCP) + แสดง chunk count/error จริง
3. **#121:** เพิ่ม file list UI พร้อม icon, ชื่อ, ขนาด, ปุ่ม ✕ ลบ, ปุ่ม "Clear all"
4. **#124:** เพิ่ม `.ppt`/`.pptx` ใน backend + เพิ่ม 5 MIME types ใน frontend + `extract_legacy_office()` ใช้ LibreOffice headless

---

## 🎯 Sprint 11B — Tasks

### Step 1: Merge PRs #126, #127, #128, #129 into `main`
- [ ] Review + merge PR #126 (Critical fix first)
- [ ] Merge PR #127
- [ ] Merge PR #128
- [ ] Merge PR #129
- [ ] Resolve merge conflicts if any (PRs touch overlapping files: `sources/page.tsx`, `extraction.rs`)

### Step 2: Issue #125 — LLM Fallback Extraction 🟡
**Priority: Medium** | Label: `sprint-11`, `enhancement`

เมื่อ native extraction fail → ให้ผู้ใช้ใช้ LLM อ่านไฟล์แทน พร้อมเลือก Model + Output Format

**User Flow:**
1. Upload ไฟล์ → Extraction fail → `raw_markdown` แสดง error
2. หน้า Markdown Preview แสดงปุ่ม "🤖 Extract with AI"
3. เลือก Model (gpt-4o, gemini-2.0-flash, custom จาก Settings > AI Models)
4. เลือก Format (Markdown / Table)
5. กด Extract → LLM อ่านไฟล์ → แสดง editable preview → Save
6. Save → อัปเดต `raw_markdown` + trigger re-chunking

**Backend:**
- [ ] `POST /api/v1/sources/:id/extract-ai`
  - Request: `{ "model": "gpt-4o", "output_format": "markdown" | "table" }`
  - Response: `{ "content": "# Extracted...", "tokens_used": 1234 }`
- [ ] Download source file จาก S3 ด้วย `download_from_s3()` (มีอยู่แล้ว)
- [ ] สร้าง prompt ส่ง text/image ไปยัง LLM API
- [ ] Support Vision API สำหรับ image-only PDF, scanned docs
- [ ] ใช้ tenant API key จาก `tenant_configs` table

**Frontend:**
- [ ] "🤖 Extract with AI" section ใน Markdown Preview dialog
  - แสดงเมื่อ `raw_markdown` เป็น empty หรือขึ้นต้นด้วย error message
- [ ] Model selector dropdown (ดึงจาก `fetchModels()` — มีอยู่ใน api.ts)
- [ ] Output format toggle: Markdown / Table
- [ ] Loading state + token usage display
- [ ] Editable textarea preview
- [ ] Save button → `PUT /api/v1/sources/:id` update `raw_markdown`

### Step 3: E2E Browser Testing (#114 — Carryover)
- [ ] ทดสอบ wizard: File Upload, Web Scraper, MCP Connection
- [ ] ทดสอบ validation: >50MB, .exe, corrupted
- [ ] ทดสอบ file removal UI (จาก #121)
- [ ] ทดสอบ sync console logs (จาก #123)

### Step 4: ISO Documentation
- [ ] สร้าง `SI_04_11_Sprint11_TestScript.md` — test cases for all Sprint 11 issues
- [ ] สร้าง `PM_02_11_Sprint11_Report.md` — sprint report

---

## 📁 Key Files Reference

### Backend (Modified in Sprint 11A)
- `ro-ai-bridge/src/routes/sources.rs` — Sources API + `sync_source()` (**#122 fixed**)
  - `download_from_s3()` function for S3 file retrieval — reuse for #125
- `ro-ai-bridge/mimir-core-ai/src/services/extraction.rs` — Extraction router (**#122, #124 done**)
  - Has `"file"` source_type + legacy office support
- `ro-ai-bridge/mimir-core-ai/src/services/ingress.rs` — Pipeline + `process_source_with_data()`
- `ro-ai-bridge/mimir-core-ai/src/services/upload.rs` — Extension validation (**#124 done**)

### Backend (Unchanged — Reference for #125)
- `ro-ai-bridge/src/routes/mod.rs` — Route registration
- `ro-ai-bridge/mimir-core-ai/src/services/mod.rs` — Module registration
- `ro-ai-bridge/mimir-core-ai/src/services/db.rs` — Database access (tenant_configs for API keys)

### Frontend (Modified in Sprint 11A)
- `ro-ai-dashboard/src/app/sources/page.tsx` — Sources page (**#123, #121 done**)
  - `handleSync()` with polling, file list with remove buttons, Markdown Preview dialog
- `ro-ai-dashboard/src/components/upload-dropzone.tsx` — (**#124 done**)
- `ro-ai-dashboard/src/lib/api.ts` — API client (**#123 done** — has `fetchSource()`, `fetchModels()`)

### API Functions Available (api.ts)
```typescript
fetchSources() → DataSource[]
fetchSource(id) → DataSource | null     // Added in #123
syncSource(id) → void
updateSource(id, data) → DataSource
uploadFile(sourceId, file, onProgress) → any
fetchModels() → ModelConfig[]           // For model selector in #125
```

---

## ⚙️ Dev Environment
```bash
# Start services
docker compose up -d
cd ro-ai-bridge && cargo run --bin ro-ai-bridge  # Backend :8080
cd ro-ai-dashboard && npm run dev                 # Frontend :3000

# Run tests
cd ro-ai-bridge && cargo test --workspace

# Login: admin / admin123
```

## 📐 Rules
1. **TDD approach** — write tests first, then implement
2. **ISO 29110** — create test script SI-04-11 and sprint report PM-02-11
3. **PR per issue** — one branch + PR per issue
4. **Commit messages** — `feat:`, `fix:`, `test:`, `docs:` prefixes
5. Follow existing code style and patterns
6. **Priority order:** Merge PRs → #125 → E2E Testing → ISO Docs

## 🗂️ Sprint 11B Summary Table

| #        | Title                           | Type          | Priority | Status      |
| -------- | ------------------------------- | ------------- | -------- | ----------- |
| Merge    | PRs #126-#129                   | 🔧 Merge       | Critical | pending     |
| **#125** | LLM Fallback Extraction         | 🟡 Enhancement | Medium   | not started |
| **#114** | E2E Browser Testing (Carryover) | 🧪 Test        | High     | not started |
| ISO      | Test Script + Sprint Report     | 📋 Docs        | Required | not started |
