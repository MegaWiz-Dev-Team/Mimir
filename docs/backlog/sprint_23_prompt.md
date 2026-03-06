# Sprint 23 Prompt: Code Quality & Refactoring

**Theme:** 🔴 Reduce tech debt at the biggest pain points
**Sprint Goal:** ลดขนาดไฟล์ที่ใหญ่ที่สุด 3 ไฟล์ ให้แต่ละ module < 500 lines

---

## B-01: Refactor `sources.rs` (61KB) → Sub-modules [P1, Size: L]

### Problem
`ro-ai-bridge/src/routes/sources.rs` (61KB, ~1800 lines) ทำทุกอย่าง: CRUD, file upload, URL scraping, sitemap parsing, Confluence/Notion integration, folder upload, DB connector, sync orchestration, markdown editing, feature flags

### Proposed Structure
```
ro-ai-bridge/src/routes/sources/
├── mod.rs          # Re-export + sources_routes()
├── crud.rs         # CRUD operations (create, list, get, delete, update)
├── sync.rs         # Sync orchestration (trigger, status, progress)
├── upload.rs       # File/folder upload (S3 integration)
├── connectors.rs   # URL scraper, sitemap, Confluence, Notion
└── config.rs       # Source config, advanced settings, markdown
```

### Acceptance Criteria
- [ ] Each sub-module < 500 lines
- [ ] All existing API endpoints unchanged (no breaking changes)
- [ ] `sources_routes()` still returns single Router
- [ ] All existing tests pass
- [ ] `cargo test` passes
- [ ] `npx next build` passes (frontend unchanged)

---

## B-02: Extract Settings Tabs into Separate Components [P1, Size: M]

### Problem
`ro-ai-dashboard/src/app/settings/page.tsx` (1500 lines, 93KB) มี 8 tabs ใน component เดียว: General, AI Models, Pipeline, Knowledge Graph, Search, Security, Tenants, Users

### Proposed Structure
```
ro-ai-dashboard/src/app/settings/
├── page.tsx              # Main layout + tab navigation only
├── components/
│   ├── GeneralTab.tsx
│   ├── AIModelsTab.tsx
│   ├── PipelineTab.tsx
│   ├── KnowledgeGraphTab.tsx
│   ├── SearchTab.tsx
│   ├── SecurityTab.tsx
│   ├── TenantsTab.tsx
│   └── UsersTab.tsx
```

### Acceptance Criteria
- [ ] Each tab component < 300 lines
- [ ] `page.tsx` < 150 lines (layout + routing only)
- [ ] All tab functionality preserved
- [ ] `npx next build` passes
- [ ] Loading/saving settings unchanged

---

## B-03: Split `agents.rs` → CRUD + Chat [P2, Size: M]

### Problem
`ro-ai-bridge/src/routes/agents.rs` (36KB) ผสม agent CRUD, templates, chat, streaming logic ไว้ด้วยกัน

### Proposed Structure
```
ro-ai-bridge/src/routes/agents/
├── mod.rs          # Re-export + agents_routes()
├── crud.rs         # Agent CRUD (create, list, get, update, delete, publish)
├── templates.rs    # Agent templates (list, get)
└── chat.rs         # Chat + streaming (send_message, stream handler)
```

### Acceptance Criteria
- [ ] Each sub-module < 400 lines
- [ ] All existing API endpoints unchanged
- [ ] Chat streaming works correctly
- [ ] `cargo test` passes

---

## TDD Approach
ตาม TDD skill — ก่อน refactor ให้:
1. ✅ Verify existing tests pass (`cargo test`, `npx next build`)
2. 🔄 Move code to new modules (no logic changes)
3. ✅ Verify all tests still pass
4. 🧹 Cleanup dead code and imports

## ISO Documentation
- [ ] PM-02.23 Sprint Report
- [ ] SI-04.23 Test Script
- [ ] SI-03 Traceability Matrix update
