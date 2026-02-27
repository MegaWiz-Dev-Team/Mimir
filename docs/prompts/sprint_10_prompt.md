# Sprint 10 Session Prompt — Knowledge Hub & E2E Testing

## 📋 Context
Project Mimir คือระบบ AI Knowledge Management Platform ที่ใช้ Rust backend (axum + sqlx + MariaDB) กับ Next.js frontend

### Architecture (หลัง Sprint 9)
- **Unified Data Pipeline:** Sources → Extraction → Chunking → Dedup → (QA Gen) → (Vector)
- **Multi-source:** File Upload (PDF/CSV/XLSX), Web Scraper, MCP Connection
- **Frontend:** 7-item nav (Overview, Sources, Knowledge, Quality, Playground, Coverage, Admin)
- **Settings:** Tabbed interface — General, AI Models, Pipeline, Knowledge Graph, Search, Security

### Tech Stack
- **Backend:** Rust (axum, sqlx, argon2, mimir-core-ai crate) — port 8080
- **Frontend:** Next.js 15 (Turbopack) — port 3000
- **DB:** MariaDB (docker: mimir_mariadb)
- **Vector:** Qdrant
- **Storage:** RustFS/MinIO (S3-compatible)
- **Auth:** JWT (`admin` / `admin123`)

---

## 🎯 Sprint 10 Issues

### Issue #115: Redesign Overview Page — Knowledge Hub Dashboard
**Priority: High** | Label: `sprint-10`, `enhancement`

เปลี่ยนหน้า Overview จาก "RO-AI Pipeline Monitor" เป็น "Dashboard" ตาม design mockup ที่อยู่ใน `docs/designs/overview_dashboard_mockup.png`

**Backend:**
- [ ] `GET /api/v1/stats` — returns: total_sources, total_chunks, qa_pairs, vector_coverage, source_health (counts by status)
- [ ] `POST /api/v1/sources/sync-all` — trigger sync for all sources

**Frontend Components:**
- [ ] Replace `src/app/page.tsx` — new Dashboard layout
- [ ] `DashboardStats` — 4 KPI cards (Total Sources, Total Chunks, QA Pairs, Vector Coverage %)
- [ ] `RecentActivity` — feed of last 10 source events
- [ ] `SourceHealth` — donut chart (recharts) showing Healthy/Failed/Pending
- [ ] `PipelineStatusTable` — per-source pipeline progress (Ingest → Chunks → Dedup → QA → Vector)
- [ ] `QuickActions` — + Add Source, Sync All, Open Playground

**Remove:**
- ❌ Provider/Model selector (ย้ายไป Settings แล้ว)
- ❌ "Run Pipeline" button
- ❌ Recent Runs table

---

### Issue #114: E2E Browser Testing — Add Source Wizard
**Priority: High** | Label: `sprint-10`

ทดสอบ Add Source wizard ผ่าน browser ทุก option (17 test scenarios):

**File Upload Flow (5 tests):**
- PDF upload → Verify COMPLETED
- CSV (Markdown mode) → Verify raw_markdown
- CSV (SQL mode) → Verify dynamic table
- XLSX upload → Verify extraction
- Folder upload → Verify multiple records

**Web Scraper Flow (2 tests):**
- Enter URL → Sync → Verify chunks/crawled_pages
- Verify DB records

**MCP Connection (2 tests):**
- Enter endpoint → Save → Verify source creation

**Validation (4 tests):**
- Upload >50MB → Client rejection
- Upload .exe → Client rejection
- Corrupted PDF → FAILED status
- Duplicate file → "Duplicate detected"

**Wizard UI (4 tests):**
- Step progression 1→2→3
- Back button works
- Storage mode toggle
- Advanced settings appear correctly

---

### Knowledge Base Page (New Issue — to create)
**Priority: Medium**

หน้า `/knowledge` ตอนนี้เป็น placeholder "Coming in Sprint 10" ต้องสร้างหน้าจริง:
- [ ] แสดงรายการ chunks ทั้งหมดจาก `chunks` table
- [ ] Search/filter chunks by source
- [ ] View chunk content
- [ ] Link back to source

### Search Settings Tab (New Issue — to create)
**Priority: Medium**

Tab "Search" ใน Settings ตอนนี้เป็น "Coming in Sprint 10" ต้องสร้าง:
- [ ] Search configuration settings (embedding model, top-k, similarity threshold)
- [ ] Search API endpoint

---

## 📁 Key Files Reference

### Backend
- `ro-ai-bridge/src/routes/sources.rs` — Sources API routes
- `ro-ai-bridge/src/routes/auth.rs` — Auth login route
- `ro-ai-bridge/mimir-core-ai/src/services/ingress.rs` — Pipeline processing
- `ro-ai-bridge/mimir-core-ai/src/services/chunking.rs` — Chunking service
- `ro-ai-bridge/mimir-core-ai/src/services/dedup.rs` — Dedup service
- `ro-ai-bridge/mimir-core-ai/src/services/link_discovery.rs` — Link discovery
- `ro-ai-bridge/mimir-core-ai/migrations/` — DB migrations

### Frontend
- `ro-ai-dashboard/src/app/page.tsx` — Overview page (TO REDESIGN)
- `ro-ai-dashboard/src/app/knowledge/page.tsx` — Knowledge placeholder
- `ro-ai-dashboard/src/app/sources/page.tsx` — Sources page + Add wizard
- `ro-ai-dashboard/src/app/settings/page.tsx` — Settings tabbed UI
- `ro-ai-dashboard/src/components/layout/Navbar.tsx` — 7-item navigation

### ISO Docs
- `docs/iso_29110/pm/PM_02_9_Sprint9_Report.md` — Sprint 9 report
- `docs/iso_29110/si/SI_04_9_Sprint9_TestScript.md` — Sprint 9 test script
- `docs/designs/overview_dashboard_mockup.png` — Dashboard design mockup

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
2. **ISO 29110** — create test script SI-04-10 and sprint report PM-02-10
3. **PR per issue** — one branch + PR per issue
4. **Commit messages** — `feat:`, `fix:`, `test:`, `docs:` prefixes
5. Follow existing code style and patterns in the codebase
