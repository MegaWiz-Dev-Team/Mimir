# PM-02.26: Sprint 31 Status Report (Hybrid RAG Search Architecture)

**Project Name:** Project Mimir
**Sprint:** Sprint 31
**Status:** 🗓️ Planned
**Date:** 2026-03-18

---

## 1. ขอบเขตของ Sprint 31 (Sprint Scope)
Sprint นี้เน้นไปที่การปิดช่องโหว่ (Gap) ของระบบค้นหา เพื่อเปลี่ยนผ่านจากระบบเดิมไปสู่ **Ensemble Retrieval** อย่างเต็มรูปแบบ ตามเอกสาร `02_13_Hybrid_RAG_Search_Architecture.md`:
- **Backend (Vector):** แก้ไขให้ `tenant_query` ยิงเข้า Qdrant จริงๆ แทนการ MOCK ด้วย SQL `LIMIT 5`
- **Backend (Tree):** Refactor คอขวดของ `tree_search` จาก Sequential Loop เป็น Asynchronous Parallel (`join_all`) 
- **Backend (Graph):** เพิ่มการดึงข้อมูลจาก Neo4j เข้ามาเป็นส่วนหนึ่งของการค้นหาแบบ Hybrid
- **Backend (Reranking):** เพิ่มระบบ Cross-Encoder Reranker เพื่อให้คะแนน Context ที่รวมกันก่อนส่งให้ LLM
- **Frontend (UI/UX):** สร้าง RAG Ensemble Playground ให้ผู้ใช้ทดสอบและมองเห็นแหล่งที่มาของข้อมูล (Vector/Graph/Tree) แยกกันชัดเจน
- **Frontend (UX):** เพิ่ม UI สำหรับ Graph Ingestion Progress และหน้า Settings สำหรับปรับ Weight Ratio

## 2. แผนการทดสอบ (Testing Plan)
- Unit Test สำหรับ Route และ Traits ใหม่ใน backend (เช่น `ensemble.rs`)
- Integration Test สำหรับ `tenant_query` ร่วมกับ Qdrant, Neo4j, และ PageIndex Sidecar
- E2E Test สำหรับ Frontend Playground ตรวจสอบการแสดงผลแหล่งที่มาของข้อมูล

## 3. GitHub Synchronization
| Issue/PR | Title                                       | Status    |
| -------- | ------------------------------------------- | --------- |
| #TBD     | True Qdrant Integration in Tenant Query     | Planned   |
| #TBD     | Async PageIndex Tree Search                 | Planned   |
| #TBD     | Neo4j Graph Retrieval Integration           | Planned   |
| #TBD     | Cross-Encoder Reranker Implementation       | Planned   |
| #TBD     | UI: RAG Ensemble Playground & Status        | Planned   |

## 4. ไฟล์ที่คาดว่าจะแก้ไข (Expected Files to Change)
- `ro-ai-bridge/src/routes/tenant_query.rs`
- `ro-ai-bridge/src/retrieval/*` (New Modules)
- `ro-ai-dashboard/src/app/playground/page.tsx`
- `ro-ai-dashboard/src/app/knowledge/page.tsx`
