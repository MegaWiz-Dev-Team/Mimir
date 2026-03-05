# PM-02.17: Sprint 17 Status Report (Knowledge Graph & GraphRAG)

**Project Name:** Project Mimir
**Sprint:** Sprint 17
**Status:** ✅ Completed
**Date:** 2026-03-04

---

## 1. ขอบเขตของ Sprint 17 (Sprint Scope)
- **Backend:** Neo4j Service Wrapper — Cypher query builders, tenant isolation, graceful degradation (14 tests)
- **Backend:** LLM Entity Extraction — prompt builder, JSON parser, deduplication logic (12 tests)
- **Backend:** Graph API Routes — 8 REST endpoints at `/api/v1/graph/*` (5 tests)
- **Backend:** Config — เพิ่ม `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD` env vars
- **Frontend:** Graph Visualization Page — canvas-based force-directed layout, entity search, path finding, detail panels
- **Frontend:** Graph API functions (8 interfaces + 8 functions) ใน `api.ts`
- **Frontend:** Graph navigation link + KG Settings tab (แทน Coming Soon)
- **Scope:** รวม Sprint 11a (KG Foundation) + 11b (GraphRAG Features) ที่เลื่อนมา

## 2. สรุปผลการทดสอบ (Testing Verification Summary)

### Backend Unit Tests (31/31 Pass)
| ID         | Description                        | Result |
| ---------- | ---------------------------------- | ------ |
| TC_SP17_U1 | cargo check — 0 errors             | ✅ Pass |
| TC_SP17_U2 | mimir-core-ai tests (27 tests)     | ✅ Pass |
| TC_SP17_U3 | ro-ai-bridge graph tests (5 tests) | ✅ Pass |

### Neo4j Service Tests (14 tests)
| ID          | Description                   | Result |
| ----------- | ----------------------------- | ------ |
| TC_SP17_N1  | Neo4j config from environment | ✅ Pass |
| TC_SP17_N2  | Neo4j config defaults         | ✅ Pass |
| TC_SP17_N3  | Cypher MERGE node builder     | ✅ Pass |
| TC_SP17_N4  | Cypher MERGE relation builder | ✅ Pass |
| TC_SP17_N5  | Cypher match nodes query      | ✅ Pass |
| TC_SP17_N6  | Cypher neighbors query        | ✅ Pass |
| TC_SP17_N7  | Cypher path query             | ✅ Pass |
| TC_SP17_N8  | Stats query builder           | ✅ Pass |
| TC_SP17_N9  | Delete by source query        | ✅ Pass |
| TC_SP17_N10 | Tenant isolation in queries   | ✅ Pass |
| TC_SP17_N11 | Search with type filter       | ✅ Pass |
| TC_SP17_N12 | Visualization query builder   | ✅ Pass |
| TC_SP17_N13 | Depth parameter in neighbors  | ✅ Pass |
| TC_SP17_N14 | Default depth in path query   | ✅ Pass |

### Entity Extractor Tests (12 tests)
| ID          | Description                          | Result |
| ----------- | ------------------------------------ | ------ |
| TC_SP17_E1  | Prompt construction contains text    | ✅ Pass |
| TC_SP17_E2  | Prompt contains JSON schema          | ✅ Pass |
| TC_SP17_E3  | Parse valid extraction JSON          | ✅ Pass |
| TC_SP17_E4  | Parse empty JSON                     | ✅ Pass |
| TC_SP17_E5  | Parse malformed JSON with code block | ✅ Pass |
| TC_SP17_E6  | Parse invalid JSON returns empty     | ✅ Pass |
| TC_SP17_E7  | Deduplicate entities by name+type    | ✅ Pass |
| TC_SP17_E8  | Deduplicate keeps first occurrence   | ✅ Pass |
| TC_SP17_E9  | No dedup for different types         | ✅ Pass |
| TC_SP17_E10 | Case-insensitive dedup               | ✅ Pass |
| TC_SP17_E11 | Extraction result creation           | ✅ Pass |
| TC_SP17_E12 | Entity type enum values              | ✅ Pass |

### Graph Route Tests (5 tests)
| ID         | Description                    | Result |
| ---------- | ------------------------------ | ------ |
| TC_SP17_R1 | Graph routes assembly          | ✅ Pass |
| TC_SP17_R2 | EntitySearchQuery defaults     | ✅ Pass |
| TC_SP17_R3 | ExtractRequest defaults        | ✅ Pass |
| TC_SP17_R4 | VisualizationQuery deserialize | ✅ Pass |
| TC_SP17_R5 | PathQuery required fields      | ✅ Pass |

**Total: 31/31 (100%)**

## 3. GitHub Synchronization & Traceability(
### Pull Requests
| PR # | Title                                                                                         | Status    |
| ---- | --------------------------------------------------------------------------------------------- | --------- |
| #187 | feat(sprint-17): Knowledge Graph system — Neo4j, Entity Extraction, Graph API & Visualization | ✅ Created |

## 4. รายละเอียดการเปลี่ยนแปลง (Changes Detail)

### Backend (Rust) — 12 files, 2631 insertions
1. **`mimir-core-ai/src/services/neo4j.rs`** — NEW: Neo4j service wrapper with Cypher query builders, tenant isolation, graceful degradation
2. **`mimir-core-ai/src/services/entity_extractor.rs`** — NEW: LLM entity extraction with prompt builder, JSON parser, dedup logic
3. **`mimir-core-ai/src/services/mod.rs`** — Registered `neo4j` + `entity_extractor` modules
4. **`src/routes/graph.rs`** — NEW: 8 API endpoints (stats, search, neighbors, paths, extract, viz, delete, runs)
5. **`src/routes/mod.rs`** — Registered `graph_routes`
6. **`src/main.rs`** — Mounted `/api/v1/graph`
7. **`src/config.rs`** — Added `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD`
8. **`src/routes/sources.rs`** — Fixed test Config struct for new fields

### Frontend (Next.js)
1. **`src/app/graph/page.tsx`** — NEW: Canvas-based graph visualization with force-directed layout
2. **`src/lib/api.ts`** — 8 TypeScript interfaces + 8 API functions for KG endpoints
3. **`src/components/navbar.tsx`** — Added Graph navigation link (Share2 icon)
4. **`src/app/settings/page.tsx`** — Replaced KG "Coming Soon" tab with active status panel

### API Endpoints
| Method   | Path                                  | Description              |
| -------- | ------------------------------------- | ------------------------ |
| `GET`    | `/api/v1/graph/stats`                 | Entity/relation counts   |
| `GET`    | `/api/v1/graph/entities`              | Search with filters      |
| `GET`    | `/api/v1/graph/entity/{id}/neighbors` | Neighbor traversal       |
| `GET`    | `/api/v1/graph/paths`                 | Path finding             |
| `POST`   | `/api/v1/graph/extract`               | Trigger extraction       |
| `GET`    | `/api/v1/graph/visualization`         | Sigma.js-compatible data |
| `DELETE` | `/api/v1/graph/source/{id}`           | Remove source entities   |
| `GET`    | `/api/v1/graph/extraction-runs`       | Extraction history       |

## 5. ปัญหาที่พบและวิธีแก้ไข (Issues & Resolutions)
1. **MariaDB vs SQLite method:**
   - *ปัญหา:* ใช้ `last_insert_rowid()` (SQLite) แทน `last_insert_id()` (MariaDB) ใน graph.rs
   - *แก้ปัญหา:* เปลี่ยนเป็น `last_insert_id()`

2. **Rust 2024 unsafe requirement:**
   - *ปัญหา:* `std::env::remove_var` ต้องอยู่ใน `unsafe {}` block ตั้งแต่ Rust 2024 edition
   - *แก้ปัญหา:* ครอบ test calls ด้วย `unsafe {}`

3. **Missing Config fields in tests:**
   - *ปัญหา:* Test helper `test_config_with_heimdall` ไม่มี `neo4j_*` fields
   - *แก้ปัญหา:* เพิ่ม `neo4j_uri`, `neo4j_user`, `neo4j_password` fields

4. **Graph page theme mismatch:**
   - *ปัญหา:* Graph page ใช้ hardcoded dark theme แทน app CSS variable theme
   - *แก้ปัญหา:* Rewrite ใช้ shadcn/ui Card components + CSS variables (bg-background, text-muted-foreground)

## 6. Sprint 18 Planning
| Feature            | Description                                               | Priority |
| ------------------ | --------------------------------------------------------- | -------- |
| Coverage Analytics | Coverage Dashboard (ACU per source, blind-spot detection) | High     |
| Coverage API       | REST endpoints for coverage overview/sources/gaps         | High     |

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
