# SI-04: Test Plan and Test Report (แผนและรายงานผลการทดสอบ)
**Project Name:** Project Mimir

## 1. Test Plan (แผนการทดสอบ)
- **Test Strategy:** [เช่น Focus on Unit Tests for core services, UAT for Dashboard]
- **Environment:** [เช่น Local development environment, MariaDB]

## 2. Test Cases & Execution Report (ผลการทดสอบ)
| Test Case ID | Test Description                         | Expected Result                                              | Actual Result                 | Status  | Date Tested | Tested By |
| ------------ | ---------------------------------------- | ------------------------------------------------------------ | ----------------------------- | ------- | ----------- | --------- |
| TC-001       | (Sprint 1) CRUD Users & Auth             | Data loads correctly, Add/Edit/Delete works successfully     | Dashboard loads active users. | Pass    | [Date]      | [Tester]  |
| TC-002       | (Sprint 2) Delete Vector from UI         | The selected vector is removed from Qdrant                   | -                             | Pending | -           | -         |
| TC-003       | (Sprint 3) Conflict Resolution Merge     | Golden Answer is saved and cluster is marked resolved        | -                             | Pending | -           | -         |
| TC-004       | (Sprint 4) Background Evaluation Trigger | Job is added to queue and progress bar updates via WebSocket | -                             | Pending | -           | -         |
| TC-005       | (Sprint 5) Streaming Log output          | Crawler logs populate UI sequentially in real-time           | -                             | Pending | -           | -         |

## 3. Historical Test Execution Records (ประวัติการทดสอบระบบก่อนหน้า)

การทดสอบเหล่านี้เป็นระบบ Foundation สมัย **Monolithic Phase 1-2** ก่อนแยกโครงสร้างเป็น Multi-tenant:

| Phase                   | Feature / Component                                    | Build Status | Unit/E2E Result                  | Test Date  | Notes                                  |
| ----------------------- | ------------------------------------------------------ | ------------ | -------------------------------- | ---------- | -------------------------------------- |
| **Phase 1: Sprint 1.1** | Infrastructure Setup (MariaDB, Qdrant, Redis, rAthena) | ✅ SUCCESS    | All 6 services healthy           | 2026-02-19 | Ports properly exposed                 |
| **Phase 1: Sprint 1.2** | Data Pipeline (Wiki QA)                                | ✅ SUCCESS    | Collection `wiki_qa` ok          | 2026-02-19 | -                                      |
| **Phase 1: Sprint 1.3** | Game Data Ingestion & AI Tables                        | ✅ SUCCESS    | DB schema & 31K+ vectors indexed | 2026-02-19 | Ingested `ro_items`, `ro_monsters`     |
| **Phase 2: Sprint 2.1** | Tier 1 Completion Agent (No RAG)                       | ✅ SUCCESS    | 4/4 Unit Tests Passed            | 2026-02-19 | `llama3.2` optimized, zero warnings    |
| **Phase 2: Sprint 2.2** | Tier 2 RAG Agent (rig-core + Qdrant)                   | ✅ SUCCESS    | 6/6 Unit Tests Passed            | 2026-02-19 | Embedded RAG retrieval, DB query tools |
