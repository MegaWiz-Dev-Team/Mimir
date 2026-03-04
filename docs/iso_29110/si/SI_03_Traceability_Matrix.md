# SI-03: Traceability Matrix (ตารางสอบกลับความต้องการ)
**Project Name:** Project Mimir

ตารางนี้ใช้เพื่อสอบทวนว่า Requirement ทุกข้อได้ถูกออกแบบ พัฒนา และทดสอบครบถ้วน

| Req ID   | Requirement Description                       | Design/Module        | Code/Component                                                      | Test Case ID | Status |
| -------- | --------------------------------------------- | -------------------- | ------------------------------------------------------------------- | ------------ | ------ |
| REQ-001  | User & Tenant Management (Sprint 1)           | IAM Module           | `IamService`, `UsersPage`                                           | TC-001       | Done   |
| REQ-002  | Vector Data Management (Sprint 2)             | Vector Module        | `Qdrant Client`, `Vector UI`                                        | TC-002       | Done   |
| REQ-003  | Tenant Settings & Prov. (Sprint 3)            | Tenant Config Module | `Settings UI`, `Provision API`                                      | TC-003       | Done   |
| REQ-004  | Quality Control (Sprint 4)                    | QC Module            | `Clustering Worker`, `Kanban UI`                                    | TC-004       | Done   |
| REQ-005  | Data Ingress & Monitoring (Sprint 5)          | Ingress Module       | `WebSocket Server`, `Log UI`                                        | TC-005       | Done   |
| REQ-006  | Agent Evaluations (Sprint 6)                  | Eval Module          | `Background Job`, `Wizard UI`                                       | TC-006       | Done   |
| REQ-007  | Final UI Validation (Sprint 7)                | UX/UI Module         | `Client UI Verification`                                            | TC-007       | Done   |
| REQ-008  | File/Folder Upload API (Sprint 8)             | Upload Module        | `upload.rs`, `sources.rs`                                           | TC-008       | Done   |
| REQ-008a | Extraction Worker (#74)                       | Extraction Module    | `extraction.rs`, `ingress.rs`                                       | TC-008       | Done   |
| REQ-008b | Domain Connector Architecture (#76)           | Domain Module        | `domain.rs`, `iam.rs`                                               | TC-008       | Done   |
| REQ-008c | SQL Import Module (#75)                       | SQL Import Module    | `sql_import.rs`, `ingress.rs`                                       | TC-008       | Done   |
| REQ-008d | Frontend Upload Components (#77)              | Upload UI Module     | `UploadDropzone`, `FolderUpload`                                    | TC-008       | Done   |
| REQ-009  | Extraction & Chunking (Sprint 9)              | Pipeline Module      | `extraction.rs`, `chunking.rs`                                      | TC-009       | Done   |
| REQ-010  | Embedding & Vector Store (Sprint 10)          | Embedding Module     | `embedding.rs`, `qdrant.rs`                                         | TC-010       | Open   |
| REQ-011a | KG Foundation (Sprint 11a → 17)               | KG Module            | `neo4j.rs`, `entity_extractor.rs`, `graph.rs`                       | TC-017       | Done   |
| REQ-011b | GraphRAG Features (Sprint 11b → 17)           | KG Module            | `graph/page.tsx`, `api.ts`, `canvas visualization`                  | TC-017       | Done   |
| REQ-012  | Multi-Agent & LLM Observability (Sprint 12)   | Multi-Agent Module   | `llm_usage.rs`, `sources.rs`                                        | TC-012       | Done   |
| REQ-013  | Agent Studio & Performance (Sprint 13)        | Agent Studio Module  | `agents.rs`, `budget.rs`                                            | TC-013       | Done   |
| BUG-039  | Auto-scan QC UI Feedback (Issue #39)          | QC Module            | `Kanban UI`, `Dashboard API`                                        | TC-004       | Done   |
| BUG-040  | Auto-scan QC Loop & Progress (#40)            | QC Module            | `clustering.rs`, `qc.rs`, `UI`                                      | TC-004       | Done   |
| BUG-041  | Vector Stats API 404 (Issue #41)              | Vector Module        | `vector.rs`, `lib/api.ts`                                           | TC-002       | Done   |
| BUG-043  | Auth Redirect on Admin Pages                  | IAM Module           | `login/page.tsx`, `api.ts`                                          | TC-001       | Done   |
| BUG-046  | Admin Login Authentication Hash               | IAM Module           | `iam.rs`, `Docker MariaDB`                                          | TC-001       | Done   |
| BUG-051  | "Configure" button on Sources page            | Ingress Module       | `sources/page.tsx`                                                  | TC-005       | Done   |
| SYS-053  | Background Data Sync Worker (#53)             | Ingress Module       | `ingress.rs`, `sources.rs`                                          | TC-005       | Done   |
| BUG-055  | Sprint 7 UX/UI Refinement (#55)               | UX/UI Module         | `sources`, `vector`, `pipeline`                                     | TC-007       | Done   |
| BUG-057  | TypeError in QC Clusters (#57)                | QC Module            | `qc.ts`, `PipelineStatusBar`                                        | TC-004       | Done   |
| BUG-059  | TypeError across Dashboard (#59)              | Dashboard UI         | `page.tsx`, `api.ts`                                                | TC-007       | Done   |
| BUG-061  | Sprint 7 TDD Unit Tests (#61)                 | Testing              | `*.test.tsx`                                                        | TC-007       | Done   |
| BUG-062  | Users page: Failed to load (#62)              | IAM Module           | `users/page.tsx`, `api.ts`                                          | TC-001       | Done   |
| BUG-064  | StatusBar visible on login (#64)              | UX/UI Module         | `layout.tsx`, `PipelineStatusBar`                                   | TC-007       | Done   |
| BUG-066  | React Error in StatusBar (#66)                | UX/UI Module         | `PipelineStatusBar`                                                 | TC-007       | Done   |
| BUG-068  | Hydration Mismatch Navbar (#68)               | UX/UI Module         | `Navbar`, `PipelineStatusBar`                                       | TC-007       | Done   |
| BUG-071  | Sprint 6 Evaluation System (#71)              | Eval Module          | `eval.rs`, `runner.rs`                                              | TC-006       | Done   |
| BUG-073  | Sprint 7 Final Testing Bug (#73)              | UX/UI Module         | `eval-wizard.test.tsx`                                              | TC-007       | Done   |
| BUG-084  | Missing avatar_url in Game Crate              | Game Module          | `simple_npc.rs`                                                     | TC-008       | Done   |
| ENH-086  | Connect IngressManager to Extraction          | Ingress Module       | `ingress.rs`, `sources.rs`                                          | TC-008       | Open   |
| ENH-134  | Web Hierarchy Loader Backend (#134)           | Hierarchy Module     | `sources.rs`, `discover-hierarchy`                                  | TC-012       | Done   |
| ENH-135  | Web Hierarchy Loader Frontend (#135)          | Hierarchy UI Module  | `sources/page.tsx`, checkbox tree                                   | TC-012       | Done   |
| ENH-136  | LLM Usage Logging Backend (#136)              | LLM Observability    | `llm_usage.rs`, `llm_usage_logs`                                    | TC-012       | Done   |
| ENH-137  | LLM Analytics Dashboard (#137)                | Analytics UI Module  | `analytics/llm/page.tsx`                                            | TC-012       | Done   |
| ENH-138  | Search Settings Persistence (#138)            | Settings Module      | `tenant_configs.search_settings`                                    | TC-012       | Done   |
| ENH-144  | Agent Studio Backend CRUD (#144)              | Agent Studio Module  | `agents.rs`, `agent_configs`                                        | TC-013       | Done   |
| ENH-145  | Agent Studio Frontend UI (#145)               | Agent Studio UI      | `agents/page.tsx`, 5-tab builder                                    | TC-013       | Done   |
| ENH-146  | Conversation Logging (#146)                   | Conversations Module | `conversations.rs`, `page.tsx`                                      | TC-013       | Done   |
| ENH-147  | LLM Eval Dashboard (#147)                     | Eval Performance     | `evaluations_ext.rs`, A/B compare                                   | TC-013       | Done   |
| ENH-148  | Advanced Analytics (#148)                     | Budget Analytics     | `budget.rs`, benchmark reports                                      | TC-013       | Done   |
| ENH-150  | Cron Worker — Scheduled Re-sync (#150)        | Cron Module          | `cron.rs`, `CronScheduleSelector`                                   | TC-014       | Done   |
| ENH-151  | OCR Integration (#151)                        | OCR Module           | `ocr.rs`, Gemini Vision API                                         | TC-014       | Done   |
| ENH-152  | External DB Connectors (#152)                 | DB Connector Module  | `db_connector.rs`, `DbConnectorWizard`                              | TC-014       | Done   |
| ENH-153  | Feedback & Bug Report (#153)                  | Feedback Module      | `feedback.rs`, `FeedbackButton`                                     | TC-014       | Done   |
| ENH-154  | E2E Test Suite (#154)                         | Testing Module       | `e2e_tests.rs`, 8 integration tests                                 | TC-014       | Done   |
| ENH-157  | Vault Secrets Management (#157)               | Vault Module         | `vault.rs`, KV v2 + rotation                                        | TC-014       | Done   |
| ENH-155  | MCP Real Implementation                       | MCP Server Module    | `mcp_server.rs`, tool registry                                      | TC-014       | Done   |
| ENH-156  | Performance Optimization                      | Performance Module   | `performance.rs`, cache + pool                                      | TC-014       | Done   |
| ENH-158  | Structured Logging & Request Tracing          | Logging Module       | `request_id.rs`, JSON tracing                                       | TC-014       | Done   |
| ENH-159  | Reversible DB Migrations (.down.sql)          | Migration Module     | `migrations/down/*.down.sql`                                        | TC-014       | Done   |
| ENH-164  | Configurable Max Crawl Pages (#164)           | Settings Module      | `tenant_configs.max_crawl_pages`, `sources.rs`, `settings/page.tsx` | TC-014b      | Done   |
| ENH-186  | Neo4j Service + Entity Extraction (Sprint 17) | KG Module            | `neo4j.rs` (14 tests), `entity_extractor.rs` (12 tests)             | TC-017       | Done   |
| ENH-187  | Graph API Routes (Sprint 17)                  | KG Module            | `routes/graph.rs` (8 endpoints, 5 tests)                            | TC-017       | Done   |
| ENH-188  | Graph Visualization Frontend (Sprint 17)      | KG UI Module         | `graph/page.tsx`, `api.ts`, `navbar.tsx`                            | TC-017       | Done   |
| ENH-189  | KG Settings Tab (Sprint 17)                   | Settings Module      | `settings/page.tsx` (replace Coming Soon)                           | TC-017       | Done   |
