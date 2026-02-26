# SI-03: Traceability Matrix (ตารางสอบกลับความต้องการ)
**Project Name:** Project Mimir

ตารางนี้ใช้เพื่อสอบทวนว่า Requirement ทุกข้อได้ถูกออกแบบ พัฒนา และทดสอบครบถ้วน

| Req ID  | Requirement Description              | Design/Module        | Code/Component                   | Test Case ID | Status |
| ------- | ------------------------------------ | -------------------- | -------------------------------- | ------------ | ------ |
| REQ-001 | User & Tenant Management (Sprint 1)  | IAM Module           | `IamService`, `UsersPage`        | TC-001       | Done   |
| REQ-002 | Vector Data Management (Sprint 2)    | Vector Module        | `Qdrant Client`, `Vector UI`     | TC-002       | Done   |
| REQ-006 | Tenant Settings & Prov. (Sprint 3)   | Tenant Config Module | `Settings UI`, `Provision API`   | TC-003       | Done   |
| REQ-004 | Quality Control (Sprint 4)           | QC Module            | `Clustering Worker`, `Kanban UI` | TC-004       | Done   |
| BUG-039 | Auto-scan QC UI Feedback (Issue #39) | QC Module            | `Kanban UI`, `Dashboard API`     | TC-004       | Done   |
| BUG-040 | Auto-scan QC Loop & Progress (#40)   | QC Module            | `clustering.rs`, `qc.rs`, `UI`   | TC-004       | Done   |
| BUG-041 | Vector Stats API 404 (Issue #41)     | Vector Module        | `vector.rs`, `lib/api.ts`        | TC-002       | Done   |
| BUG-043 | Auth Redirect on Admin Pages         | IAM Module           | `login/page.tsx`, `api.ts`       | TC-001       | Done   |
| BUG-046 | Admin Login Authentication Hash      | IAM Module           | `iam.rs`, `Docker MariaDB`       | TC-001       | Done   |
| BUG-051 | "Configure" button on Sources page   | Ingress Module       | `sources/page.tsx`               | TC-005       | Done   |
| SYS-053 | Background Data Sync Worker (#53)    | Ingress Module       | `ingress.rs`, `sources.rs`       | TC-005       | Done   |
| REQ-005 | Data Ingress & Monitoring (Sprint 5) | Ingress Module       | `WebSocket Server`, `Log UI`     | TC-005       | Done   |
| REQ-003 | Agent Evaluations (Sprint 6)         | Eval Module          | `Background Job`, `Wizard UI`    | TC-006       | Done   |
| REQ-007 | Final UI Validation (Sprint 7)       | UX/UI Module         | `Client UI Verification`         | TC-007       | Done   |
| BUG-071 | Sprint 6 Evaluation System (#71)     | Eval Module          | `eval.rs`, `runner.rs`           | TC-006       | Done   |
| BUG-073 | Sprint 7 Final Testing Bug (#73)     | UX/UI Module         | `eval-wizard.test.tsx`           | TC-007       | Done   |
