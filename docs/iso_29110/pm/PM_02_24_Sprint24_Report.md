# PM-02.24: Sprint 24 Status Report (Graph API Hotfix & KG Import)

**Project Name:** Project Mimir
**Sprint:** Sprint 24
**Status:** ✅ Completed
**Date:** 2026-03-09

---

## 1. ขอบเขตของ Sprint 24 (Sprint Scope)
- **Issue #222:** แก้ 5 bugs ใน Graph API — SQL syntax (MariaDB), เพิ่ม bulk import API, แก้ vector search TenantContext, แก้ FK-based queries (neighbors/paths/visualization), แก้ relations import logic
- **Issue #223:** Deduplicate KG entities ที่ import ซ้ำ (2,682 → 1,341), เพิ่ม UNIQUE index
- **Issue #224:** แก้ Vector Search ให้ใช้ Heimdall embedding (BAAI/bge-m3) แทน Ollama, recreate wiki_qa collection (768→1024 dim)
- **Issue #225:** แก้ Coverage API ให้ detect KG data จาก `kg_entities` แทน `kg_extraction_runs`
- **Data Import:** Import 1,341 entities + 685 relations ผ่าน Mimir API (bulk endpoints)
- **Scope:** Bug fixes + Enhancement — ไม่มีเปลี่ยนแปลง Frontend

## 2. สรุปผลการทดสอบ (Testing Summary)

| Category                      | Total  | ✅ Pass | ❌ Fail |
| ----------------------------- | ------ | ------ | ------ |
| Graph API Integration Tests   | 5      | 5      | 0      |
| Vector Search Integration     | 2      | 2      | 0      |
| Coverage API Verification     | 1      | 1      | 0      |
| KG Data Import Validation     | 3      | 3      | 0      |
| **Total**                     | **11** | **11** | **0**  |

## 3. GitHub Synchronization

| Item                                                                    | Type  | Status |
| ----------------------------------------------------------------------- | ----- | ------ |
| [Issue #222](https://github.com/megacare-dev/Mimir/issues/222)          | Issue | Open   |
| [Issue #223](https://github.com/megacare-dev/Mimir/issues/223)          | Issue | Open   |
| [Issue #224](https://github.com/megacare-dev/Mimir/issues/224)          | Issue | Open   |
| [Issue #225](https://github.com/megacare-dev/Mimir/issues/225)          | Issue | Open   |
| [PR #226](https://github.com/megacare-dev/Mimir/pull/226)              | PR    | Open   |

## 4. ไฟล์ที่แก้ไข (Files Changed)

| File                                                   | Change Type | Description                                                 |
| ------------------------------------------------------ | ----------- | ----------------------------------------------------------- |
| `ro-ai-bridge/src/routes/graph.rs`                     | Modified    | +5 query fixes, +2 bulk endpoints, +structs (+210, -28)     |
| `ro-ai-bridge/src/routes/vector.rs`                    | Modified    | TenantContext→HeaderMap, Ollama→Heimdall reqwest (+75, -20) |
| `ro-ai-bridge/src/routes/coverage.rs`                  | Modified    | kg_extracted ใช้ sources_with_kg (+1, -1)                    |
| `ro-ai-bridge/mimir-core-ai/migrations/20260309*.sql`  | New         | Dedup entities + UNIQUE index (16 lines)                    |
| `pipeline_data/import_via_mimir_api.py`                | New         | Bulk import script via Mimir API (152 lines)                |
| `pipeline_data/pipeline_results.json`                  | New         | KG/QA data (1,341 entities, 693 relations, 898 QA)          |
| `pipeline_data/pipeline_kg_qa.py`                      | New         | KG extraction pipeline script (135 lines)                   |
| `pipeline_data/embed_chunks.py`                        | New         | Chunk embedding script (133 lines)                          |

## 5. Technical Decisions
- **Heimdall Direct HTTP**: ใช้ `reqwest` HTTP POST ไปที่ `/v1/embeddings` (OpenAI-compatible) แทน `rig::ollama::Client` เพื่อรองรับ embedding server ที่ไม่ใช่ Ollama
- **FK-based Graph Queries**: เขียน query ใหม่ทั้งหมดให้ JOIN ผ่าน `from_entity_id`/`to_entity_id` ตาม schema จริง แทน text columns ที่ไม่มีอยู่
- **UNIQUE Index**: เพิ่ม `idx_kg_entity_unique (name(255), entity_type, tenant_id)` เพื่อป้องกัน entity ซ้ำในอนาคต
- **Coverage Data Source**: ใช้ `kg_entities` table โดยตรงแทน `kg_extraction_runs` table เพราะ extraction runs ไม่ครอบคลุม data ที่ bulk import เข้ามา
- **Qdrant Dimension Fix**: Recreate `wiki_qa` collection จาก 768 → 1024 dim ให้ตรงกับ BAAI/bge-m3
