# PM-01: Project Plan (แผนโครงการ)
**Project Name:** Project Mimir
**Document Version:** 2.0

## 1. Project Scope & Objectives (ขอบเขตและวัตถุประสงค์)
- **เป้าหมาย:** พัฒนาระบบปัญญาประดิษฐ์กลาง (AI Core Platform) แบบ Multi-Tenant Modular Architecture ที่สามารถนำไปประยุกต์ใช้กับธุรกิจได้หลากหลายรูปแบบ (เช่น Mega Care Platform, OumOum AI Agent) โดยเริ่มจากการแยกแพลตฟอร์มออกจากระบบตัวเกม
- **ขอบเขต:** สร้างระบบดูดข้อมูล (Ingress), Real Extraction & Chunking Pipeline, ระบบจัดการเวกเตอร์ข้อมูล (Embedding + Qdrant), Knowledge Graph (Neo4j + GraphRAG), ระบบควบคุมคุณภาพ (QA/QC), Multi-Agent Architecture (Router + Tool Registry + Synthesis), AI Agent Studio (no-code visual builder), ระบบทดสอบประเมินผล (Evaluations), Dataset Studio สำหรับสร้าง Training Dataset, Training Integration สำหรับ Fine-tune LLM/SLM, พร้อมหน้า Dashboard, Setup & Deployment Guide, และเอกสาร ISO 29110

## 2. Project Organization & Resources (โครงสร้างทีมและทรัพยากร)
- **Project Manager:** [ชื่อ PM / Mega Wiz]
- **System Analyst/Designer:** [ชื่อ SA / Mega Wiz]
- **Developer:** ทีมพัฒนา Project Mimir (Backend & Frontend)
- **Tester:** ทีม QA และผู้ประเมินประสิทธิผล LLM

## 3. Project Schedule & Milestones (ตารางเวลาและจุดส่งมอบ)
- **Sprint 1: Security Foundation & IAM** [ระบุช่วงเวลา]
  - Backend API (CRUD Users/Tenants), Auth Middleware, Frontend Dashboard
- **Sprint 2: Data Isolation & Vector Management + Tenant Settings** [ระบุช่วงเวลา]
  - Tenant ID migrations, Ingestion Pipeline update, Vector Search UI, Tenant Settings UI
- **Sprint 3: Tenant Configuration & Provisioning Workflow** [ระบุช่วงเวลา]
  - Centralized Config Schema, New Tenant Provisioning Flow, Tenant Management UI
- **Sprint 4: Quality Control & Hallucination Prevention** [ระบุช่วงเวลา]
  - Data Clustering, LLM Consensus Checker, Conflict Resolution UI
- **Sprint 5: Data Ingress Monitoring** [ระบุช่วงเวลา]
  - Data Source CRUD APIs, Streaming Logs UI, Real-time status websockets
- **Sprint 6: Agent Evaluations System** [ระบุช่วงเวลา]
  - Evaluation Background Job, Real-time Progress Bar, QA Results by Tenant
- **Sprint 7: UX/UI Pipeline Refinement & Traceability** [ระบุช่วงเวลา]
  - Ingress Markdown Preview, ACU Coverage Dashboard, Conflict Resolution UI, Vector End-to-End Traceability
- **Sprint 8: Unified Data Ingress & File Upload** [✅ Completed]
  - File/Folder Upload API (S3), Extraction Worker (stub), SQL Import (dual-mode), Domain Connector, Frontend Upload Wizard, Smart Upload (auto-detect source_type)
- **Sprint 9: Real Pipeline & Navigation** [Week 1-2]
  - Pipeline Wiring (wire existing extraction.rs into full pipeline), Chunking Service (configurable: fixed/recursive/semantic), Web Link Discovery & Preview, Cross-source Content Deduplication (SHA-256), Navigation Restructure (7 items), Settings Tabs (General/AI Models/Pipeline/KG/Search/Security), DB Migration
- **Sprint 10: Embedding & Vector Store** [Week 3-4]
  - Embedding Service (multi-model with pipeline lock), Qdrant Integration (per-tenant collection), SQL Schema Registry, Pipeline Status Bar (7-step), Knowledge Base Page
- **Sprint 11a: KG Foundation** [Week 5-6]
  - Neo4j Setup (Docker + Bolt client), LLM Entity Extraction, LLM Provider Abstraction Phase 1 (Ollama + Gemini + OpenAI), Graph Storage (entities + relations → Neo4j)
- **Sprint 11b: GraphRAG Features** [Week 7-8]
  - Graph Visualization (Sigma.js + graphology + ForceAtlas2), Knowledge Search (entity + path finding), Hybrid Search (Vector + Graph + SQL → merged context)
- **Sprint 12: Multi-Agent & Coverage Intelligence** [Week 9-10]
  - Tool Registry (vector_search, sql_query, graph_search), Router Agent (LLM query analysis), Synthesis Agent, ACU per Source, Blind-spot Detection, Closed-loop Actions, LLM Usage Logging (llm_usage_logs table, instrument all LLM calls), LLM Analytics Dashboard (token in/out per model, latency, cost estimation), Web Hierarchy Loader (site hierarchy discovery, selective page import, duplicate detection)
- **Sprint 13: AI Agent Studio** [Week 11-12]
  - Agent CRUD (no-code), Agent Studio UI (visual builder), Test Chat, Agent Templates (Q&A Bot, Data Analyst, Research Assistant), Agent Deploy (API + widget), Conversation Logging, Chat History, LLM Performance Evaluation (quality scoring per model, A/B comparison, user feedback), Advanced Analytics (daily token budget, usage alerts, model benchmark)
- **Sprint 14a: Production Core** [Week 13-14]
  - Scheduled Re-sync (Cron), OCR Integration (Tesseract/PaddleOCR), External DB Connection (MySQL/PostgreSQL/SQLite), MCP Real Implementation, Performance Optimization, Reversible DB Migrations (.down.sql), Feedback & Bug Report, E2E Test Suite (full pipeline), Structured logging & request tracing, Secrets Management (HashiCorp Vault)
- **Sprint 14b: Deploy & Docs** [Week 15-16]
  - Setup & Deployment (Docker Compose prod, .env templates, setup scripts), Deployment Test (M3 → M4 Pro), Update & Rollback (update.sh + rollback.sh + GHCR + auto-backup), API Documentation (OpenAPI/Swagger), Backup & DR, MLX + vLLM Providers Phase 2 (add + benchmark)
- **Sprint 15: Dataset Studio** [Week 17-18]
  - Dataset CRUD, Data Source Selector, Filter & Transform, Format Converter (Alpaca/ShareGPT/DPO/Raw/Custom), Export (JSONL/Parquet + HuggingFace), Data Augmentation, Dataset Preview
- **Sprint 16: Training Integration + ISO Docs** [Week 19-20]
  - Training Config UI, Axolotl/Unsloth Integration (Docker), MLflow Tracking, Model Registry (version + A/B test), ISO Final Documentation (SI-05 User Manual, SI-06 Release Notes)
- **Sprint 17: Knowledge Graph Implementation** [✅ Completed — 2026-03-04]
  - Neo4j service wrapper (Cypher builders, tenant isolation, 14 tests), LLM entity extraction (prompt builder, JSON parser, dedup, 12 tests), Graph API routes (8 endpoints: stats/search/neighbors/paths/extract/viz/delete/runs, 5 tests), Frontend graph visualization (canvas force-directed layout, search, path finding), Graph navigation, KG Settings tab
- **Sprint 18: Coverage Analytics Dashboard** [✅ Completed — 2026-03-04]
  - Coverage API (3 endpoints: overview/sources/gaps, 14 tests), Pure-function helpers (calculate_coverage_score, detect_blindspots), Coverage Dashboard (KPI cards, pipeline flow, gap analysis panel, sortable per-source table), REQ-012 Coverage Intelligence
- **Sprint 19: Agent Templates & Security** [✅ Completed — 2026-03-04]
  - Agent Templates migration (PERSONAS → DB), Playground → Agent Studio integration, Security & Access Settings tab, Vault Status Dashboard, Custom Roles + ACL Matrix
- **Sprint 20: Custom Roles ACL** [✅ Completed — 2026-03-05]
  - Custom Role CRUD, Editable ACL Matrix UI, Permission module granularity (10 modules × 3 levels), Role assignment in Users page
- **Sprint 21: QA Status & Auto-Refresh** [✅ Completed — 2026-03-05]
  - QA status column in Knowledge Base (QaStatusBadge), Auto-refresh polling (5s interval, auto-stop), Selective Chunk → QA Generation end-to-end
- **Sprint 22: Antigravity Skills & E2E Analysis** [✅ Completed — 2026-03-06]
  - 8 Antigravity Skills (ISO Documentation, Testing Workflow, Rust Backend Patterns, TDD, Agile Scrum, Code Review, Next.js Frontend Patterns, UX Designer), E2E Flow Review (12-step user journey analysis), Product Backlog (15 items for Sprint 23-25)
- **Sprint 23: Code Quality & Refactoring** [✅ Completed — 2026-03-06]
  - Refactor sources.rs (1568 lines → 6 sub-modules), Extract Settings tabs (1500 → 340 lines, 8 components), Split agents.rs (876 lines → 4 sub-modules), 69 tests passing
- **Sprint 24: Graph API Hotfix & KG Import** [✅ Completed — 2026-03-09]
  - Fix 5 Graph API bugs (SQL syntax, FK queries, visualization), deduplicate KG entities (2682→1341), Vector Search switch Ollama→Heimdall (bge-m3), Coverage API detect KG data, bulk import 1,341 entities + 685 relations, 11 tests passing
- **Sprint 25: Vector & Chat Fixes** [✅ Completed — 2026-03-09]
  - QA bulk vector indexing + chunk embedding API (#234), Coverage API detect actual QA/vector data (#236), Chat RAG hybrid search tenant + embedding fix (#238), Medical reference data import (Sleep, ENT, Neurology, Drugs)
- **Sprint 26: Multi-Provider Extraction & Prompt Management** [✅ Completed — 2026-03-10]
  - Multi-provider extraction support (Ollama + Gemini + OpenAI + Heimdall), Versioned prompt management system, Provider-specific extraction pipelines
- **Sprint 27: Evaluation Expansion** [✅ Completed — 2026-03-10]
  - Evaluation expansion — extraction + retrieval tabs, Multi-dimensional evaluation scoring, Provider comparison analysis
- **Sprint 28: Auto-Pipeline & E2E Scorecard** [✅ Completed — 2026-03-11]
  - Auto-Pipeline 1-click endpoint (Source → Chunk → Embed → QA one-click), E2E Pipeline Scorecard + Dashboard Tab, QC scanning infinite loop fix + stop endpoint, Pipeline status bar cleanup, Batch pipeline runner script, Pipeline monitor script

## 4. Risk Management (การจัดการความเสี่ยง)
| Risk (ความเสี่ยง)                                                                | Impact (ผลกระทบ) | Mitigation Strategy (แผนรับมือ)                                                                  |
| ------------------------------------------------------------------------------ | ---------------- | ---------------------------------------------------------------------------------------------- |
| **Cross-Tenant Data Leakage:** ข้อมูลข้าม Tenant รั่วไหลหากเขียน API ผิดพลาด          | High             | บังคับใช้ `tenant_auth_middleware` กับทุก API และใส่ `tenant_id` ลง Filter ของ Qdrant เสมอ           |
| **Noisy Neighbor:** Tenant หนึ่งดึง Traffic LLM จนโควต้าหมด กระทบระบบอื่น            | High             | ทำ Rate Limiting แบบ Token Bucket แยกตาม Tenant ผ่าน Redis                                       |
| **Prompt Injection:** ผู้เล่น/ผู้ใช้หลอกหลอกถาม AI ให้ทำคำสั่งนอกกรอบ                     | Medium           | ใช้ LLM "System Prompt" Armor ครอบป้องกัน และให้ Domain Connector เป็นตัวตรวจสอบ Authority ก่อนรันคำสั่ง  |
| **SQL Injection via Text-to-SQL:** LLM สร้าง SQL อันตราย                         | High             | ใช้ read-only connection, query sandbox, table whitelist, row limit (LIMIT 1000), query logging |
| **Neo4j Resource Usage:** Knowledge Graph ใหญ่ใช้ memory สูง                      | Medium           | จำกัด entity ต่อ tenant, lazy loading, pagination ใน graph visualization                          |
| **LLM Cost Overrun:** Entity extraction + embedding ใช้ token สูง                | Medium           | ทำ pipeline lock ป้องกันเปลี่ยนกลางทาง, batch processing, caching extracted entities                |
| **Embedding Model Lock-in:** เปลี่ยน model กลางทางทำให้ vector ไม่ compatible       | High             | Pipeline lock config — ต้อง re-embed ทั้งหมดหากเปลี่ยน model                                        |
| **Training Data Quality:** Dataset มี noise จาก QA ที่ไม่ผ่าน QC                    | High             | ใช้ quality score filter (min 0.8), human-reviewed only option, near-duplicate removal          |
| **Model Version Conflict:** Fine-tuned model ใหม่ performance แย่กว่าเดิม          | Medium           | Model Registry + A/B test ใน Playground ก่อน deploy, rollback mechanism                         |
| **Data Loss (No Backup):** สูญเสียข้อมูล MariaDB/Neo4j/Qdrant/S3 ถ้า server ล่ม      | High             | Automated backup (daily MariaDB dump, Neo4j export, Qdrant snapshot, S3 versioning)            |
| **Failed Update/Deployment:** Update version แล้วระบบ crash หรือ DB migration ผิด | High             | Auto-backup ก่อน update, reversible migrations (.down.sql), rollback script, health check       |
| **No Observability:** ไม่สามารถ debug production issues ได้                      | Medium           | Structured logging (tracing crate), request tracing, error rate dashboard                      |
| **LLM Model Missing After Update:** Ollama model หายหลัง update binary          | Medium           | Health check step verify `ollama list` หลัง update, document required models                    |
| **Infrastructure Drift:** docker-compose ไม่ตรงกับ codebase (เช่น RustFS missing) | Medium           | Pre-Sprint 9 tech debt (#89-#92), docker-compose review ทุก sprint                              |

