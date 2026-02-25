# SI-03: Traceability Matrix (ตารางสอบกลับความต้องการ)
**Project Name:** Project Mimir

ตารางนี้ใช้เพื่อสอบทวนว่า Requirement ทุกข้อได้ถูกออกแบบ พัฒนา และทดสอบครบถ้วน

| Req ID  | Requirement Description              | Design/Module        | Code/Component                   | Test Case ID | Status  |
| ------- | ------------------------------------ | -------------------- | -------------------------------- | ------------ | ------- |
| REQ-001 | User & Tenant Management (Sprint 1)  | IAM Module           | `IamService`, `UsersPage`        | TC-001       | Done    |
| REQ-002 | Vector Data Management (Sprint 2)    | Vector Module        | `Qdrant Client`, `Vector UI`     | TC-002       | Pending |
| REQ-006 | Tenant Settings & Prov. (Sprint 3)   | Tenant Config Module | `Settings UI`, `Provision API`   | TC-003       | Done    |
| REQ-003 | Quality Control (Sprint 4)           | QC Module            | `Clustering Worker`, `Kanban UI` | TC-004       | Done    |
| BUG-039 | Auto-scan QC UI Feedback (Issue #39) | QC Module            | `Kanban UI`, `Dashboard API`     | TC-004       | Pending |
| BUG-041 | Vector Stats API 404 (Issue #41)     | Vector Module        | `vector.rs`, `lib/api.ts`        | TC-002       | Pending |
| BUG-043 | Auth Redirect on Admin Pages         | IAM Module           | `login/page.tsx`, `api.ts`       | TC-001       | Done    |
| REQ-004 | Agent Evaluations (Sprint 5)         | Eval Module          | `Background Job`, `Wizard UI`    | TC-005       | Pending |
| REQ-005 | Data Ingress & Monitoring (Sprint 6) | Ingress Module       | `WebSocket Server`, `Log UI`     | TC-006       | Pending |
