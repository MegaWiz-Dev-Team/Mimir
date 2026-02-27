# SI-04: Test Plan and Test Report (แผนและรายงานผลการทดสอบ)
**Project Name:** Project Mimir

## 1. Test Plan (แผนการทดสอบ)
- **Test Strategy:** TDD approach with Backend/Frontend Unit Tests, and detailed manual UI/API verification per Sprint.
- **Environment:** Local development environment, Docker (MariaDB, Qdrant, Redis), Rust Backend, Next.js Frontend.

## 2. Test Cases & Execution Report (ผลการทดสอบ)
| Test Case ID | Test Description                                | Expected Result                                                      | Actual Result                                   | Status | Date Tested | Tested By |
| ------------ | ----------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------- | ------ | ----------- | --------- |
| TC-001       | (Sprint 1) CRUD Users & Auth                    | Data loads correctly, Add/Edit/Delete works successfully             | Dashboard loads active users.                   | Pass   | 2026-02-22  | AI        |
| TC-002       | (Sprint 2) Vector Management & DB Isolate       | Multi-tenant Vector search and deletion work correctly               | Features work securely.                         | Pass   | 2026-02-23  | AI        |
| TC-003       | (Sprint 3) Tenant Settings & Provisioning       | Centralized configuration and user/DB spawning work                  | Centralized config loads, tenant creates        | Pass   | 2026-02-24  | AI        |
| TC-004       | (Sprint 4) Quality Control & Kanban Dashboard   | System groups conflicting QA pairs & UI handles resolution           | Background job runs and Kanban updates properly | Pass   | 2026-02-25  | Agent     |
| TC-005       | (Sprint 5) Streaming Log output                 | Crawler logs populate UI sequentially in real-time                   | Dashboard live console stream                   | Pass   | 2026-02-25  | Agent     |
| TC-006       | (Sprint 6) Background Evaluation Trigger        | Job is added to queue and progress bar updates via WebSocket         | Dashboard live evaluation updates               | Pass   | 2026-02-26  | AI        |
| TC-007       | (Sprint 7) UX/UI Pipeline Refinement & Trace    | Pipeline components render states securely and visually              | Dashboard UI passes hydration & UX tests        | Pass   | 2026-02-26  | AI        |
| TC-008       | (Sprint 8) Unified Data Ingress & File Upload   | Upload, Extraction, SQL Import, Domain Connector work E2E            | 15/16 tests pass (93.75%), 80 unit tests pass   | Pass   | 2026-02-27  | AI        |
| TC-009       | (Sprint 9) Content Pipeline & Navigation        | Chunking, Link Discovery, Dedup, Nav Restructure, Settings           | 24/24 tests pass (100%), 116+ unit tests pass   | Pass   | 2026-02-27  | AI        |
| TC-010       | (Sprint 10) Dashboard Redesign & Knowledge Base | KPI Cards, Pipeline Bar, Knowledge Base, Search Settings, E2E Wizard | 30/30 tests pass (100%), 6/6 E2E pass           | Pass   | 2026-02-27  | AI        |
| TC-011       | (Sprint 11) LLM Fallback & File Improvements    | LLM extraction, Console logs, File upload, CSV sync fix              | Pending                                         | Open   | —           | —         |

## 3. Historical Test Execution Records (ประวัติการทดสอบระบบก่อนหน้า)

การทดสอบเหล่านี้เป็นระบบ Foundation สมัย **Monolithic Phase 1-2** ก่อนแยกโครงสร้างเป็น Multi-tenant:

| Phase                   | Feature / Component                                    | Build Status | Unit/E2E Result                  | Test Date  | Notes                                  |
| ----------------------- | ------------------------------------------------------ | ------------ | -------------------------------- | ---------- | -------------------------------------- |
| **Phase 1: Sprint 1.1** | Infrastructure Setup (MariaDB, Qdrant, Redis, rAthena) | ✅ SUCCESS    | All 6 services healthy           | 2026-02-19 | Ports properly exposed                 |
| **Phase 1: Sprint 1.2** | Data Pipeline (Wiki QA)                                | ✅ SUCCESS    | Collection `wiki_qa` ok          | 2026-02-19 | -                                      |
| **Phase 1: Sprint 1.3** | Game Data Ingestion & AI Tables                        | ✅ SUCCESS    | DB schema & 31K+ vectors indexed | 2026-02-19 | Ingested `ro_items`, `ro_monsters`     |
| **Phase 2: Sprint 2.1** | Tier 1 Completion Agent (No RAG)                       | ✅ SUCCESS    | 4/4 Unit Tests Passed            | 2026-02-19 | `llama3.2` optimized, zero warnings    |
| **Phase 2: Sprint 2.2** | Tier 2 RAG Agent (rig-core + Qdrant)                   | ✅ SUCCESS    | 6/6 Unit Tests Passed            | 2026-02-19 | Embedded RAG retrieval, DB query tools |
