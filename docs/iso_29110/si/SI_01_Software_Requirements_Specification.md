# SI-01: Software Requirements Specification (SRS)
**Project Name:** Project Mimir

## 1. Introduction (บทนำ)
- [อธิบายภาพรวมของระบบ แพลตฟอร์มนี้คืออะไร]

## 2. Functional Requirements (ความต้องการด้านฟังก์ชัน)
| Req ID  | Requirement Description                                                                                                                                                                                                                               | Priority |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| REQ-001 | **Security & IAM:** ระบบต้องรองรับการจัดการสิทธิ์แบบ Multi-tenant (CRUD Users/Tenants) และ Authentication ผ่าน JWT Token                                                                                                                                     | High     |
| REQ-002 | **Vector Management:** ระบบต้องสามารถแยกเก็บข้อมูลแยกตาม Tenant กรองข้อมูลเก่า/หมดอายุ และแก้ไข Vector Data ได้จากหน้า UI                                                                                                                                        | High     |
| REQ-004 | **Quality Control:** ระบบต้องมีการใช้ LLM วิเคราะห์ความขัดแย้งของข้อมูล (Clustering) และให้ User สรุป Golden Answer ได้ผ่านหน้าจอ Kanban                                                                                                                            | Medium   |
| REQ-005 | **Data Ingress:** ระบบต้องรองรับการนำเข้าข้อมูล (Web, File, MCP) และแสดงสถานะการดูดข้อมูลแบบ Real-time (Streaming Logs)                                                                                                                                        | High     |
| REQ-003 | **Agent Evaluation:** ระบบต้องสามารถรันประเมินความแม่นยำของ AI (Evaluation) แบบ Background Job, มีหน้าต่าง Wizard ในการเลือก Agent/Model, และแสดง Progress พร้อมคะแนนทดสอบแบบ Heatmap ที่สามารถ Override คะแนนได้                                                  | Medium   |
| REQ-006 | **Tenant Settings:** ระบบต้องมีหน้า Settings ให้ผู้ใช้สามารถแก้ไขชื่อ (Name) และจัดการข้อมูล Tenant ของตนเองได้                                                                                                                                                     | High     |
| REQ-007 | **UX/UI Pipeline & Traceability:** ระบบต้องรองรับการดู Markdown Preview, แสดงผลความคืบหน้าแบบ Real-time, มี UI จัดการข้อมูลขัดแย้ง, และการตรวจสอบย้อนกลับ (Traceability) ไปยัง Source Document URL ได้อย่างง่ายดาย                                                     | High     |
| REQ-008 | **Unified Data Ingress & File Upload:** ระบบต้องรองรับ File/Folder Upload ผ่าน S3, Smart Upload (auto-detect source_type จาก extension), รองรับ dual-mode สำหรับ Tabular Data (Markdown/SQL), และ Domain Connector                                          | High     |
| REQ-009 | **Real Extraction & Chunking Pipeline:** ระบบต้อง extract ข้อมูลจริงจาก PDF/CSV/HTML, Chunking ต้อง configurable (fixed-size/recursive/semantic) พร้อม auto-recommend, รองรับ Cross-source Deduplication ด้วย SHA-256 content hash                            | High     |
| REQ-010 | **Embedding & Vector Store:** ระบบต้อง generate embeddings ด้วย multi-model (Ollama/Gemini/Qwen) พร้อม pipeline lock, เก็บลง Qdrant collection แบบ per-tenant, มี Knowledge Base Page แสดง chunks + vectors                                                | High     |
| REQ-011 | **Knowledge Graph & GraphRAG:** ระบบต้อง extract entities/relations ด้วย LLM เก็บลง Neo4j, แสดง Graph Visualization ด้วย Sigma.js, รองรับ Hybrid Search (Vector + Graph + SQL → merged context)                                                            | High     |
| REQ-012 | **Multi-Agent & Coverage Intelligence:** ระบบต้องมี Router Agent วิเคราะห์ query เพื่อเลือก tool, Tool Registry per tenant, ACU per source, Blind-spot Detection พร้อม Closed-loop Actions (Add Source / Re-chunk / Manual Fact / AI Expand)                  | High     |
| REQ-013 | **AI Agent Studio:** ระบบต้องมี Visual Builder สำหรับสร้าง AI Agent แบบ no-code (เลือก model + tools + prompt + settings), Test Chat, Agent Templates, และ Deploy ผ่าน API endpoint + embeddable widget                                                      | Medium   |
| REQ-014 | **Production Ready:** ระบบต้องรองรับ Scheduled Re-sync (Cron), OCR, External DB Connection (MySQL/PostgreSQL/SQLite), Batch Processing, และ Performance Optimization                                                                                    | Medium   |
| REQ-015 | **Dataset Studio:** ระบบต้องรองรับการสร้าง Training Dataset จากข้อมูลที่ผ่าน QC แล้ว (QA pairs, KG triples, chunks, conversations), Filter ตาม quality score, Export เป็น Alpaca/ShareGPT/DPO/JSONL/Parquet, Data Augmentation ด้วย LLM, และ push ไป HuggingFace | Medium   |
| REQ-016 | **Training Integration:** ระบบต้องรองรับ Training Config UI (base model, LoRA rank, hyperparameters), Integration กับ Axolotl/Unsloth (Docker), MLflow Tracking, และ Model Registry (version + A/B test ใน Playground)                                   | Low      |

## 3. Non-Functional Requirements (ความต้องการด้านอื่นๆ ที่ไม่ใช่ฟังก์ชัน)
- **Security & Multi-Tenancy:**
  - Database ระดับ Relational และ Vector (Qdrant) ต้องมีการแบ่งแยก Tenant อย่างสมบูรณ์ผ่าน Payload filtering / `WHERE tenant_id`.
  - การยืนยันตัวตนสำหรับผู้ดูแลระบบใช้ On-Premise JWT Authentication พร้อมด้วยชั่วโมงหมดอายุ (Access Token 15 นาที, Refresh 7 วัน) และเข้ารหัสรหัสผ่านด้วย Argon2id.
  - ระบบต้องป้องกัน Prompt Injection โดยทำ LLM "System Prompt" Armor.
- **Performance & Scalability:**
  - Rate Limiting แบบ Token Bucket แยกตาม Tenant (เช่น Tenant A 50 RPM, Tenant B 200 RPM) ผ่าน Redis เพื่อป้องกันปัญหา Noisy Neighbor.
  - โครงสร้าง Containerization (Docker/Local) พร้อมขยายตัวสู่ Kubernetes (K8s) สถาปัตยกรรมคลาวด์.
- **Usability:**
  - Dashboard ต้องมี Tenant Switcher แบบ Global สำหรับ Super Admin.
  - ระบบควรมี Global Pipeline Status Bar เพื่อให้เห็นภาพรวมของข้อมูลในแต่ละกระบวนการได้อย่างชัดเจน
  - ดีไซน์ต้อง Responsive ด้วย Next.js และ shadcn/ui.
