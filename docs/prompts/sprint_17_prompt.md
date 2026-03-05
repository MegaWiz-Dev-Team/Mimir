# Sprint 17 Session Prompt — Knowledge Graph & GraphRAG
**Project:** Project Mimir
**Sprint:** 17
**Branch:** `feature/sprint-17-knowledge-graph`
**มาตรฐาน:** ISO/IEC 29110 + TDD (Test-Driven Development)
**Issue:** #185 (ต่อเนื่อง) + สร้าง issue ใหม่สำหรับ KG

---

## 🎯 Sprint Goal
สร้างระบบ Knowledge Graph ครบวงจร: ตั้งแต่ Neo4j infrastructure → LLM entity extraction → graph storage → graph visualization (Sigma.js) → Hybrid Search (Vector + Graph + SQL) ตามขอบเขตเดิมของ Sprint 11a + 11b ที่เลื่อนมา

---

## 📋 Sprint 17 Features (แบ่ง 2 Phase)

### ═══════════════════════════════════════════════
### 🔵 Phase 1: KG Foundation (Sprint 11a scope)
### ═══════════════════════════════════════════════

### 1.1 Infrastructure — Neo4j Docker + Rust Client
**Priority:** 🔴 Critical | **Type:** Infrastructure | **Area:** DevOps + Backend
**Scope:**
- เพิ่ม Neo4j 5 Community Edition ใน `docker-compose.yml` (Bolt 7687, Browser 7474, APOC plugin)
- เพิ่ม `neo4rs = "0.8"` ใน workspace `Cargo.toml` + `mimir-core-ai/Cargo.toml`
- สร้าง `Neo4jService` wrapper (`services/neo4j.rs`):
  - `new(uri, user, pass)` → connect + create indexes/constraints
  - `upsert_entity(tenant_id, name, entity_type, properties)` → MERGE node
  - `upsert_relation(tenant_id, from, to, type, properties)` → MERGE edge
  - `search_entities(tenant_id, query, limit)` → text MATCH
  - `find_paths(tenant_id, from, to, max_depth)` → shortest path
  - `get_neighbors(tenant_id, entity, depth)` → subgraph expansion
  - `get_graph_stats(tenant_id)` → counts by type
  - `delete_entities_by_source(tenant_id, source_id)` → cleanup
- Environment variables: `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD`
- **TDD:** test connection, test upsert/query round-trip, test tenant isolation, test path finding

### 1.2 DB Migration — KG Tables (MariaDB)
**Priority:** 🔴 Critical | **Type:** Schema | **Area:** Backend
**Scope:**
- `kg_entities`: id, tenant_id, name, entity_type, properties(JSON), source_id, chunk_id, neo4j_node_id, created_at
- `kg_relations`: id, tenant_id, from_entity_id, to_entity_id, relation_type, properties(JSON), source_id, neo4j_rel_id, created_at
- `kg_extraction_runs`: id, tenant_id, source_id, status, entities_found, relations_found, chunks_processed, error_message, started_at, finished_at
- Down migration สำหรับ rollback
- **TDD:** test migration up/down

### 1.3 LLM Entity Extraction Service
**Priority:** 🔴 Critical | **Type:** Feature | **Area:** Backend
**Scope:**
- สร้าง `EntityExtractor` service (`services/entity_extractor.rs`):
  - ใช้ LLM (resolve จาก `llm_config.rag` slot) สำหรับ extract entities/relations จาก text
  - Structured JSON output via prompt engineering:
    ```json
    {
      "entities": [{"name": "Aspirin", "type": "Drug", "properties": {"category": "NSAID"}}],
      "relations": [{"from": "Aspirin", "to": "Headache", "type": "treats"}]
    }
    ```
  - `extract_from_chunk(text, tenant_id, source_id, chunk_id)` → parsed entities + relations
  - `batch_extract(chunks[], tenant_id, source_id)` → process multiple chunks with progress
  - Entity types: Person, Organization, Location, Concept, Event, Product (+ domain-specific: Drug, Symptom, Item, Monster)
  - De-duplication: merge entities by name + type (case-insensitive)
  - Error handling: retry on LLM failure, skip chunk on persistent error
- **TDD:** test prompt construction, test JSON parsing (valid/invalid/partial), test dedup logic, test error handling

### 1.4 API Routes — Graph Endpoints
**Priority:** 🟡 Medium | **Type:** Feature | **Area:** Backend
**Scope:**
- สร้าง `routes/graph.rs` + register ใน `mod.rs`:
  - `GET /api/v1/graph/stats` → node/edge counts by type
  - `GET /api/v1/graph/entities?q=&type=&limit=&page=` → search entities (paginated)
  - `GET /api/v1/graph/entity/:id/neighbors?depth=` → subgraph for visualization
  - `GET /api/v1/graph/paths?from=&to=&depth=` → path finding
  - `POST /api/v1/graph/extract` → trigger extraction for source(s) `{ source_ids: [1,2,3] }`
  - `GET /api/v1/graph/visualization?limit=&type=` → nodes + edges for Sigma.js
  - `DELETE /api/v1/graph/source/:id` → cleanup source entities
- ทุก endpoint ต้องผ่าน `tenant_auth_middleware`
- **TDD:** test route assembly, test tenant isolation in graph queries

### 1.5 Pipeline Integration (Optional KG Step)
**Priority:** 🟡 Medium | **Type:** Enhancement | **Area:** Backend
**Scope:**
- เพิ่ม KG extraction step หลัง chunking ใน pipeline (parallel กับ embedding)
- ตรวจสอบ tenant config `enable_kg` flag ก่อนรัน extraction
- บันทึก extraction results ลง `kg_extraction_runs`
- ถ้า Neo4j ไม่พร้อม → skip gracefully (log warning, ไม่ fail pipeline)
- **TDD:** test pipeline with/without KG enabled, test Neo4j unavailable scenario

---

### ═══════════════════════════════════════════════
### 🟢 Phase 2: GraphRAG Features (Sprint 11b scope)
### ═══════════════════════════════════════════════

### 2.1 Frontend — Graph Visualization Page
**Priority:** 🔵 High | **Type:** Feature | **Area:** Frontend
**Scope:**
- สร้าง `app/graph/page.tsx` (หรือ tab ใน Knowledge page):
  - Interactive graph: `@sigma/react` + `graphology` + ForceAtlas2 layout
  - Node colors ตาม entity type (Person=blue, Org=green, Location=orange, etc.)
  - Click node → details panel (name, type, properties, related entities)
  - Search bar → highlight matching nodes
  - Filter by entity type / source
  - Stats sidebar (total nodes, edges, entity type breakdown)
  - Pull graph data จาก `GET /api/v1/graph/visualization`
- เพิ่ม Navigation item "Graph" ใน sidebar
- **TDD:** test component renders, test API data mapping

### 2.2 Frontend — Graph API Functions
**Priority:** 🔵 High | **Type:** Feature | **Area:** Frontend
**Scope:**
- เพิ่มใน `api.ts`:
  - `fetchGraphStats()` → graph stats
  - `fetchGraphData(params)` → nodes + edges for visualization
  - `searchEntities(query, type, limit)` → entity search
  - `findPaths(from, to, depth)` → path results
  - `triggerExtraction(sourceIds)` → start extraction
  - `getExtractionRuns(sourceId?)` → extraction history
- TypeScript interfaces: `GraphEntity`, `GraphRelation`, `GraphStats`, `ExtractionRun`

### 2.3 Settings — KG Tab (Replace Coming Soon)
**Priority:** 🟡 Medium | **Type:** Enhancement | **Area:** Frontend
**Scope:**
- แทน "Coming Soon Sprint 11" ด้วย real KG settings:
  - Toggle: Enable Knowledge Graph extraction
  - Entity types config (checkboxes)
  - Max entities per chunk (slider: 5-50)
  - Neo4j connection status indicator (green/red dot)
  - "Test Connection" button
  - Stats: total entities, relations, last extraction

### 2.4 Hybrid Search — Vector + Graph + SQL
**Priority:** 🟡 Medium | **Type:** Feature | **Area:** Backend
**Scope:**
- เพิ่ม `graph_search()` ใน `rag_engine/mod.rs`:
  - Query → extract key entities → search Neo4j neighbors → return context
- Implement `hybrid_search()`:
  - Run Vector, Graph, SQL search in parallel
  - Merge & re-rank results (RRF — Reciprocal Rank Fusion)
  - Return combined context with source attribution
- Wire MCP `graph_search` tool ให้เรียก Neo4j จริง (replace stub ใน `mcp_server.rs`)
- **TDD:** test graph search, test hybrid merge/rank, test MCP dispatch

---

## 🔄 ขั้นตอนการทำงาน (Workflow) — ทำตามลำดับนี้

### Phase 0: Planning & GitHub Setup
1. สร้าง branch `feature/sprint-17-knowledge-graph` จาก `feature/185-centralized-llm-config`
2. สร้าง GitHub Issues สำหรับ Sprint 17 (1 issue ต่อ feature area)
3. ตรวจสอบ baseline: `cargo check`, `cargo test -p mimir-core-ai`, `npx next build`

### Phase 1: Implementation (TDD) — KG Foundation

**ลำดับการทำงาน:**
```
🔴 Infrastructure (ทำก่อน — เป็น dependency ของทุกอย่าง)
1. docker-compose.yml    → Neo4j service
2. Cargo.toml            → neo4rs dependency
3. neo4j.rs              → Neo4j service wrapper (TDD: test connection, CRUD)
4. DB migration          → kg_entities, kg_relations, kg_extraction_runs

🔴 Core Services (ทำหลัง infrastructure)
5. entity_extractor.rs   → LLM entity extraction (TDD: test prompts, parsing, dedup)
6. routes/graph.rs       → API endpoints (TDD: test routes, tenant isolation)
7. pipeline.rs           → KG step integration (TDD: test pipeline with/without KG)
```

**สำหรับแต่ละ Feature:**
1. **เขียน Test ก่อน** (Red) — สร้าง test cases ตาม scope
2. **Implement** (Green) — เขียน code ให้ tests ผ่าน
3. **Refactor** (Refactor) — ปรับปรุง code quality
4. **Verify** — `cargo test` / `cargo check`
5. **Commit** — `feat(#xxx): <description>`

### Phase 2: Implementation (TDD) — GraphRAG Features

**ลำดับการทำงาน:**
```
🔵 Frontend (ทำหลัง API routes พร้อม)
8. api.ts                → Graph API functions + TypeScript interfaces
9. graph/page.tsx        → Sigma.js visualization page
10. settings/page.tsx    → KG Settings tab (replace Coming Soon)

🟡 Hybrid Search (ทำหลัง frontend)
11. rag_engine/mod.rs    → graph_search + hybrid_search
12. mcp_server.rs        → Wire graph_search tool
```

### Phase 3: Testing (ISO 29110 — SI-04)
1. **สร้าง Test Script** `docs/iso_29110/si/SI_04_17_Sprint17_TestScript.md`
   - ส่วนที่ 1: Unit Tests — Neo4j service, entity extractor, API routes
   - ส่วนที่ 2: KG Foundation — entity extraction, graph CRUD, pipeline integration
   - ส่วนที่ 3: GraphRAG — visualization, hybrid search, MCP
   - ส่วนที่ 4: Frontend — graph page, settings KG tab
   - ทุกข้อต้องมี: ID, Scenario, Steps, Expected, Result, Issue#/PR#, หมายเหตุ

2. **Execute Tests ตาม Script** — ทดสอบทีละข้อตามลำดับ

### Phase 4: ISO Documentation
1. **Update SI-02** — เพิ่ม Knowledge Graph Module subsystem description
2. **Update SI-03** — เพิ่ม traceability entries
3. **สร้าง PM-02.17** — Sprint 17 Status Report
4. **Update PM-02** — เพิ่ม Sprint 17 row
5. **Update PM-01** — Update Sprint 17 description ใน Project Plan

### Phase 5: Final Verification & Push
1. รัน full test suite:
   ```bash
   cargo test -p mimir-core-ai 2>&1 | tail -5
   cargo check 2>&1 | tail -5
   cd ro-ai-dashboard && npx next build 2>&1 | tail -10
   ```
2. Commit all changes
3. Push + Create PR → merge to main

---

## 📁 Files to Create/Modify

### New Files (Backend)
```
ro-ai-bridge/mimir-core-ai/src/services/neo4j.rs              — Neo4j service wrapper
ro-ai-bridge/mimir-core-ai/src/services/entity_extractor.rs    — LLM entity extraction
ro-ai-bridge/mimir-core-ai/migrations/20260304160000_add_kg_tables.sql  — ✅ Already created
ro-ai-bridge/mimir-core-ai/migrations/down/20260304160000_add_kg_tables.down.sql — ✅ Already created
ro-ai-bridge/src/routes/graph.rs                               — Graph API routes
```

### Modified Files (Backend)
```
docker-compose.yml                                             — ✅ Neo4j service added
ro-ai-bridge/Cargo.toml                                        — ✅ neo4rs workspace dep
ro-ai-bridge/mimir-core-ai/Cargo.toml                          — ✅ neo4rs dep
ro-ai-bridge/mimir-core-ai/src/services/mod.rs                 — register neo4j + entity_extractor modules
ro-ai-bridge/src/routes/mod.rs                                 — register graph_routes
ro-ai-bridge/mimir-core-ai/src/qa_qc/pipeline.rs              — optional KG extraction step
ro-ai-bridge/mimir-core-ai/src/rag_engine/mod.rs               — hybrid search
ro-ai-bridge/mimir-core-ai/src/services/mcp_server.rs          — wire graph_search
```

### New Files (Frontend)
```
ro-ai-dashboard/src/app/graph/page.tsx                         — Graph visualization
```

### Modified Files (Frontend)
```
ro-ai-dashboard/src/lib/api.ts                                 — Graph API functions
ro-ai-dashboard/src/app/settings/page.tsx                      — KG Settings tab
```

### ISO Documents
```
docs/iso_29110/si/SI_04_17_Sprint17_TestScript.md              — NEW
docs/iso_29110/pm/PM_02_17_Sprint17_Report.md                  — NEW
docs/iso_29110/si/SI_02_Software_Design_Document.md            — UPDATE
docs/iso_29110/si/SI_03_Traceability_Matrix.md                 — UPDATE
docs/iso_29110/pm/PM_01_Project_Plan.md                        — UPDATE
docs/prompts/sprint_17_prompt.md                               — THIS FILE
```

---

## ⚠️ Important Notes
- **Branch:** `feature/sprint-17-knowledge-graph` จาก `feature/185-centralized-llm-config` (current)
- **TDD:** เขียน test ก่อน implement ทุก feature
- **ISO:** ทุก feature ต้องมี test case ใน SI-04, traceable ใน SI-03
- **Commit Convention:** `feat(#xxx): <description>`
- **Neo4j Graceful Degradation:** ถ้า Neo4j ไม่พร้อม → warn + skip, ไม่ fail pipeline
- **Tenant Isolation:** ทุก Cypher query ต้องมี `WHERE n.tenant_id = $tenant_id`
- **LLM Config:** Entity extraction ใช้ `llm_config.rag` slot (centralized config จาก Sprint 16)
- **Already Done (from this session):**
  - ✅ `docker-compose.yml` — Neo4j service added
  - ✅ `Cargo.toml` — neo4rs dependency added (workspace + mimir-core-ai)
  - ✅ DB migration — `kg_entities`, `kg_relations`, `kg_extraction_runs` tables created

---

## 📊 Sprint Summary

| Category         | Count  | Items                                   |
| ---------------- | ------ | --------------------------------------- |
| 🔴 Infrastructure | 2      | Docker + Cargo deps, DB migration       |
| 🔴 Core Services  | 3      | Neo4j service, Entity extractor, Routes |
| 🔵 Frontend       | 3      | Graph page, API functions, Settings KG  |
| 🟡 Integration    | 3      | Pipeline, Hybrid Search, MCP wiring     |
| **Total**        | **11** |                                         |

**Estimated Effort:**
- Phase 1 (KG Foundation): ~4-5 sessions
- Phase 2 (GraphRAG): ~3-4 sessions
- Testing + ISO: ~1-2 sessions
- **Total: ~8-11 sessions**
