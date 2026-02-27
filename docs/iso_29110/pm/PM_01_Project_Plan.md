# PM-01: Project Plan (แผนโครงการ)
**Project Name:** Project Mimir
**Document Version:** 1.0

## 1. Project Scope & Objectives (ขอบเขตและวัตถุประสงค์)
- **เป้าหมาย:** พัฒนาระบบปัญญาประดิษฐ์กลาง (AI Core Platform) แบบ Multi-Tenant Modular Architecture ที่สามารถนำไปประยุกต์ใช้กับธุรกิจได้หลากหลายรูปแบบ (เช่น Mega Care Platform, OumOum AI Agent) โดยเริ่มจากการแยกแพลตฟอร์มออกจากระบบตัวเกม
- **ขอบเขต:** สร้างระบบดูดข้อมูล (Ingress), ระบบจัดการเวกเตอร์ข้อมูล (Vector DB), ระบบควบคุมคุณภาพ (QA/QC), เอเจนต์ (RAG), และระบบทดสอบประเมินผล (Evaluations) พร้อมหน้า Dashboard สำหรับผู้ดูแลระบบ

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
- **Sprint 9: Real Pipeline & Navigation** [2 สัปดาห์]
  - Real Extraction (PDF/CSV/HTML), Chunking Service (configurable: fixed/recursive/semantic), Web Link Discovery & Preview, Cross-source Content Deduplication (SHA-256), Navigation Restructure (7 items), Settings Tabs (General/AI Models/Pipeline/KG/Search/Security), DB Migration
- **Sprint 10: Embedding & Vector Store** [2 สัปดาห์]
  - Embedding Service (multi-model: Ollama/Gemini/Qwen with pipeline lock), Qdrant Integration (per-tenant collection), SQL Schema Registry, Pipeline Status Bar (7-step), Knowledge Base Page
- **Sprint 11: Knowledge Graph & GraphRAG** [2 สัปดาห์]
  - Neo4j Setup (Docker), LLM Entity Extraction (multi-provider), LLM Provider Abstraction Layer, Graph Storage (entities + relations), Graph Visualization (Sigma.js + graphology), Knowledge Search, Hybrid Search (Vector + Graph + SQL → merged context)
- **Sprint 12: Multi-Agent & Coverage Intelligence** [2 สัปดาห์]
  - Tool Registry (vector_search, sql_query, graph_search), Router Agent (LLM query analysis), Synthesis Agent, ACU per Source, Blind-spot Detection, Closed-loop Actions (Add Source / Re-chunk / Manual Fact / AI Expand)
- **Sprint 13: AI Agent Studio** [2 สัปดาห์]
  - Agent CRUD (config-based, no-code), Agent Studio UI (visual builder: model + tools + prompt), Test Chat (inline panel), Agent Templates (Q&A Bot, Data Analyst, Research Assistant), Agent Deploy (API endpoint + embeddable widget), Conversation Logging (Agent Studio + Playground), Chat History (per user per agent)
- **Sprint 14: Production Ready** [2 สัปดาห์]
  - Scheduled Re-sync (Cron), OCR Integration (scanned PDF), External DB Connection (MySQL + PostgreSQL + SQLite), MCP Real Implementation, Performance Optimization (embedding cache, query sandbox, batch processing), Setup & Deployment (Docker Compose prod config, .env templates, cloud guide AWS/GCP, one-command setup), API Documentation (OpenAPI/Swagger), Backup & DR (MariaDB + Neo4j + Qdrant + S3), ISO Final Documentation (SI-05 User Manual, SI-06 Release Notes)
- **Sprint 15: Dataset Studio** [2 สัปดาห์]
  - Dataset CRUD (config-based), Data Source Selector (QA pairs, KG triples, chunks, conversations), Filter & Transform (quality score, dedup, language), Format Converter (Alpaca/ShareGPT/DPO/Raw/Custom), Export (JSONL/Parquet + HuggingFace push), Data Augmentation (LLM paraphrase), Dataset Preview
- **Sprint 16: Training Integration** [2 สัปดาห์]
  - Training Config UI (base model, hyperparameters, LoRA rank), Axolotl/Unsloth Integration (Docker), MLflow Tracking (metrics, loss curves), Model Registry (version + A/B test in Playground)

## 4. Risk Management (การจัดการความเสี่ยง)
| Risk (ความเสี่ยง)                                                           | Impact (ผลกระทบ) | Mitigation Strategy (แผนรับมือ)                                                                  |
| ------------------------------------------------------------------------- | ---------------- | ---------------------------------------------------------------------------------------------- |
| **Cross-Tenant Data Leakage:** ข้อมูลข้าม Tenant รั่วไหลหากเขียน API ผิดพลาด     | High             | บังคับใช้ `tenant_auth_middleware` กับทุก API และใส่ `tenant_id` ลง Filter ของ Qdrant เสมอ           |
| **Noisy Neighbor:** Tenant หนึ่งดึง Traffic LLM จนโควต้าหมด กระทบระบบอื่น       | High             | ทำ Rate Limiting แบบ Token Bucket แยกตาม Tenant ผ่าน Redis                                       |
| **Prompt Injection:** ผู้เล่น/ผู้ใช้หลอกหลอกถาม AI ให้ทำคำสั่งนอกกรอบ                | Medium           | ใช้ LLM "System Prompt" Armor ครอบป้องกัน และให้ Domain Connector เป็นตัวตรวจสอบ Authority ก่อนรันคำสั่ง  |
| **SQL Injection via Text-to-SQL:** LLM สร้าง SQL อันตราย                    | High             | ใช้ read-only connection, query sandbox, table whitelist, row limit (LIMIT 1000), query logging |
| **Neo4j Resource Usage:** Knowledge Graph ใหญ่ใช้ memory สูง                 | Medium           | จำกัด entity ต่อ tenant, lazy loading, pagination ใน graph visualization                          |
| **LLM Cost Overrun:** Entity extraction + embedding ใช้ token สูง           | Medium           | ทำ pipeline lock ป้องกันเปลี่ยนกลางทาง, batch processing, caching extracted entities                |
| **Embedding Model Lock-in:** เปลี่ยน model กลางทางทำให้ vector ไม่ compatible  | High             | Pipeline lock config — ต้อง re-embed ทั้งหมดหากเปลี่ยน model                                        |
| **Training Data Quality:** Dataset มี noise จาก QA ที่ไม่ผ่าน QC               | High             | ใช้ quality score filter (min 0.8), human-reviewed only option, near-duplicate removal          |
| **Model Version Conflict:** Fine-tuned model ใหม่ performance แย่กว่าเดิม     | Medium           | Model Registry + A/B test ใน Playground ก่อน deploy, rollback mechanism                         |
| **Data Loss (No Backup):** สูญเสียข้อมูล MariaDB/Neo4j/Qdrant/S3 ถ้า server ล่ม | High             | Automated backup (daily MariaDB dump, Neo4j export, Qdrant snapshot, S3 versioning)            |

