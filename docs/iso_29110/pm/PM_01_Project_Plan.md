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

## 4. Risk Management (การจัดการความเสี่ยง)
| Risk (ความเสี่ยง)                                                       | Impact (ผลกระทบ) | Mitigation Strategy (แผนรับมือ)                                                                 |
| --------------------------------------------------------------------- | ---------------- | --------------------------------------------------------------------------------------------- |
| **Cross-Tenant Data Leakage:** ข้อมูลข้าม Tenant รั่วไหลหากเขียน API ผิดพลาด | High             | บังคับใช้ `tenant_auth_middleware` กับทุก API และใส่ `tenant_id` ลง Filter ของ Qdrant เสมอ          |
| **Noisy Neighbor:** Tenant หนึ่งดึง Traffic LLM จนโควต้าหมด กระทบระบบอื่น   | High             | ทำ Rate Limiting แบบ Token Bucket แยกตาม Tenant ผ่าน Redis                                      |
| **Prompt Injection:** ผู้เล่น/ผู้ใช้หลอกหลอกถาม AI ให้ทำคำสั่งนอกกรอบ            | Medium           | ใช้ LLM "System Prompt" Armor ครอบป้องกัน และให้ Domain Connector เป็นตัวตรวจสอบ Authority ก่อนรันคำสั่ง |
