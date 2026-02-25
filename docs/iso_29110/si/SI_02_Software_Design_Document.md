# SI-02: Software Design Document (SDD)
**Project Name:** Project Mimir

## 1. System Architecture (สถาปัตยกรรมระบบ)
- **Frontend:** Next.js พร้อม TailwindCSS และ shadcn/ui สำหรับส่วน Dashboard (Management UI)
- **Backend (Rust Workspace Monorepo):** 
  - `mimir-core-ai`: Domain-Agnostic Core Platform (จัดการ Ingress, RAG, QA/QC, Vector, Auth)
  - `ro-ai-domain-game`: Game Connector สำหรับเชื่อมต่อกับ rAthena
- **Database:** MariaDB สำหรับ Relational Data และ Qdrant สำหรับ Vector Database
- **AI/LLM Provider:** รองรับ Google Gemini, Ollama (Local)

## 2. Database Design (การออกแบบฐานข้อมูล)
- **Tables (RDBMS MariaDB):** `users`, `tenants`, `tenant_users`, `qa_results`, `pipeline_runs`, `pipeline_steps`, `qa_clusters`, `evaluation_reports` (ทุกตารางข้อมูลต้องมี `tenant_id` ยกเว้น config กลาง)
- **Vector DB (Qdrant):** ใช้ Payload-based Partitioning โดยยัด `{"tenant_id": "<value>"}` ลงไปใน Payload ของทุก Vector เพื่อความปลอดภัย
- [ER Diagram Placeholder - รอสร้างและนำภาพมาแนบ]

## 3. Subsystem Design (การออกแบบระบบย่อยจาก Sprint 1-6)
- **IAM Module:** จัดการ `tenant_auth_middleware` (Sprint 1)
- **Vector & Pipeline Module:** จัดการ Data Ingestion และ Semantic Search (Sprint 2)
- **Tenant Configuration Module:** จัดการ Settings และ Provisioning Workflow แบบ Centralized (Sprint 3)
- **Quality Control Module:** Background Worker จัดการ Data Clustering (Sprint 4)
- **Ingress Module:** WebSocket/SSE รันสถานะของ Data Crawler (Sprint 5)
- **Evaluation Module:** รัน Metric Evaluation แบบ Asynchronous (Sprint 6)
