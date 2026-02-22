# SI-01: Software Requirements Specification (SRS)
**Project Name:** Project Mimir

## 1. Introduction (บทนำ)
- [อธิบายภาพรวมของระบบ แพลตฟอร์มนี้คืออะไร]

## 2. Functional Requirements (ความต้องการด้านฟังก์ชัน)
| Req ID  | Requirement Description                                                                                                    | Priority |
| ------- | -------------------------------------------------------------------------------------------------------------------------- | -------- |
| REQ-001 | **Security & IAM:** ระบบต้องรองรับการจัดการสิทธิ์แบบ Multi-tenant (CRUD Users/Tenants) และ Authentication ผ่าน JWT Token          | High     |
| REQ-002 | **Vector Management:** ระบบต้องสามารถแยกเก็บข้อมูลแยกตาม Tenant กรองข้อมูลเก่า/หมดอายุ และแก้ไข Vector Data ได้จากหน้า UI             | High     |
| REQ-003 | **Quality Control:** ระบบต้องมีการใช้ LLM วิเคราะห์ความขัดแย้งของข้อมูล (Clustering) และให้ User สรุป Golden Answer ได้ผ่านหน้าจอ Kanban | Medium   |
| REQ-004 | **Agent Evaluation:** ระบบต้องสามารถรันประเมินความแม่นยำของ AI (Evaluation) แบบ Background Job และแสดงผล Progress/Heatmap       | Medium   |
| REQ-005 | **Data Ingress:** ระบบต้องรองรับการนำเข้าข้อมูล (Web, File, MCP) และแสดงสถานะการดูดข้อมูลแบบ Real-time (Streaming Logs)             | High     |

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
  - ดีไซน์ต้อง Responsive ด้วย Next.js และ shadcn/ui.
