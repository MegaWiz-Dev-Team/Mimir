# SI-02: Software Design Document (SDD)
**Project Name:** Project Mimir

## 1. System Architecture (สถาปัตยกรรมระบบ)
- **Frontend:** Next.js พร้อม TailwindCSS และ shadcn/ui สำหรับส่วน Dashboard (Management UI)
- **Backend (Rust Workspace Monorepo):** 
  - `mimir-core-ai`: Domain-Agnostic Core Platform (จัดการ Ingress, RAG, QA/QC, Vector, Auth)
  - `ro-ai-domain-game`: Game Connector สำหรับเชื่อมต่อกับ rAthena
- **Database:** MariaDB สำหรับ Relational Data และ Qdrant สำหรับ Vector Database
- **Graph Database:** Neo4j Community Edition (Docker) สำหรับ Knowledge Graph
- **Graph Visualization:** Sigma.js + graphology (WebGL, ForceAtlas2 layout)
- **AI/LLM Provider:** รองรับ Google Gemini, Ollama (Local), Qwen API — switchable per tenant
- **Embedding Models:** Configurable (nomic-embed-text / text-embedding-004 / bge-m3) พร้อม pipeline lock

### Pipeline Architecture (Sprint 9-14):
```
Sources ─┬→ File  → Extract → Chunk ─┬→ Embed → Qdrant (Vector Search)
         │                            └→ Entity → Neo4j  (Graph Search)
         ├→ Web   → Crawl → Extract → Chunk → (same)
         ├→ SQL/DB → Schema Sync → MariaDB     (SQL Search)
         └→ MCP   → Protocol → (varies)

Query → Router Agent ─┬→ Vector Agent (Qdrant)
                       ├→ SQL Agent    (MariaDB)
                       └→ Graph Agent  (Neo4j)
                       → Synthesis Agent → Answer
```

### Navigation Structure (Sprint 9+):
```
Overview · Sources · Knowledge · Quality · Playground · Coverage · ⚙️ Admin
```

## 2. Database Design (การออกแบบฐานข้อมูล)
- **Tables (RDBMS MariaDB):** `users`, `tenants`, `tenant_users`, `qa_results`, `pipeline_runs`, `pipeline_steps`, `qa_clusters`, `evaluation_reports`, `data_sources` (ทุกตารางข้อมูลต้องมี `tenant_id` ยกเว้น config กลาง)
- **Sprint 8+ Tables:** `crawled_pages` (web link discovery), `content_fingerprints` (cross-source dedup), `chunks` (chunked content), `embeddings_config` (pipeline lock), `agents` (Agent Studio config), `agent_conversations` (audit log)
- **Vector DB (Qdrant):** ใช้ Per-Tenant Collection เป็น default, รองรับ per-source metadata filter ผ่าน Agent/Search config
- **Graph DB (Neo4j):** Entities (Drug, Symptom, Person, etc.) + Relations (treats, causes, contains) แยก per tenant via property `tenant_id`
- [ER Diagram Placeholder - รอสร้างและนำภาพมาแนบ]

## 3. Subsystem Design (การออกแบบระบบย่อยจาก Sprint 1-14)
- **IAM Module:** จัดการ `tenant_auth_middleware` (Sprint 1)
- **Vector & Pipeline Module:** จัดการ Data Ingestion และ Semantic Search (Sprint 2)
- **Tenant Configuration Module:** จัดการ Settings และ Provisioning Workflow แบบ Centralized (Sprint 3)
- **Quality Control Module:** Background Worker จัดการ Data Clustering (Sprint 4)
- **Ingress Module:** WebSocket/SSE รันสถานะของ Data Crawler (Sprint 5)
- **Evaluation Module:** รันระบบให้คะแนนปัญญาประดิษฐ์อัตโนมัติ (LLM-as-a-judge) ผ่านการเทียบฐานข้อมูลและสร้างสรุป Heatmap (Sprint 6)
- **Pipeline UI/UX Module:** ปรับปรุง Flow การทำงานหลักทั้งหมดให้รองรับ Multi-tenancy ทบสอบระบบโดยรวม และตรวจสอบความถูกต้อง (Traceability) ของผลลัพธ์ (Sprint 7)
- **Upload & Smart Ingress Module:** File/Folder Upload ผ่าน S3, Smart Upload (auto-detect source_type), SQL Import dual-mode (Sprint 8)
- **Extraction & Chunking Module:** Real extraction (PDF/CSV/HTML), Configurable chunking (fixed/recursive/semantic), Web link discovery, Cross-source deduplication (Sprint 9)
- **Embedding & Vector Store Module:** Multi-model embedding service (Ollama/Gemini/Qwen) with pipeline lock, Qdrant per-tenant collection, SQL Schema Registry, Knowledge Base page (Sprint 10)
- **Knowledge Graph Module:** Neo4j entity/relation storage, LLM-based entity extraction (multi-provider), Sigma.js graph visualization, GraphRAG hybrid search (Vector + Graph + SQL) (Sprint 11)
- **Multi-Agent Module:** Tool Registry, Router Agent, Synthesis Agent, ACU Coverage per source, Blind-spot detection, Closed-loop pipeline actions (Sprint 12)
- **Agent Studio Module:** Visual agent builder (no-code), Agent CRUD config, Test Chat, Agent Templates, API endpoint + widget deployment, Conversation logging (Sprint 13)
- **Production Module:** Scheduled re-sync (Cron), OCR integration, External DB connectors (MySQL/PostgreSQL/SQLite), Performance optimization, ISO documentation finalization (Sprint 14)
