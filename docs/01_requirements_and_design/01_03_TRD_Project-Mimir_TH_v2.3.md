# 📖 Technical Requirement Document (TRD) — ฉบับภาษาไทย
## โปรเจกต์ Project-Mimir (Ragnarok Online: AI-Native Evolution)

| ฟิลด์           | ค่า                                                                           |
| ------------- | ---------------------------------------------------------------------------- |
| **เวอร์ชัน**    | 2.3 (Updated - Multi-Tenant Modular Architecture & UX/UI Redesign)           |
| **วันที่**       | 2026-02-21                                                                   |
| **Framework** | Rig (rig-core 0.10.0) + Axum 0.8.8 (Backend) / Next.js + Tailwind (Frontend) |

> เอกสารฉบับนี้เป็น **TRD ฉบับอัปเดต v2.3** สำหรับ Project-Mimir โดยรวบรวมแผนการปรับปรุงระบบจาก v2.2 ให้สอดคล้องกับสถาปัตยกรรม **"Multi-Tenant Modular Architecture"** และสอดแทรก **UX/UI Redesign** ของหน้า Dashboard เพื่อให้พร้อมพัฒนาในเฟสถัดไป

---

## 1. สถาปัตยกรรมระบบ (System Architecture v2.3)

### ภาพรวม: Multi-Tenant Modular Architecture

ระบบปัญญาประดิษฐ์กลาง (AI Core Platform) จะถูกแยกออกจากระบบตัวเกม (Domain Connectors) เพื่อรองรับธุรกิจหลากหลายรูปแบบ (Multi-Tenant) 

### 🔑 Architectural Design Principles (v2.3)
1. **Decoupled Architecture**: แยกระบบดูดข้อมูล (Ingress), ผู้สร้างชุดคำถามคิวซี (QA/QC), ดาต้าเบส (Vector), เอเจนต์ (RAG), และการประเมินผล (Eval) ออกเป็นโมดูลอิสระ
2. **Multi-Tenancy Enforced**: 
   - ฐานข้อมูล (MariaDB) ทุกตารางที่เก็บข้อมูล (ข้อยกเว้นคือ config กลาง) ต้องมี `tenant_id`
   - Vector DB (Qdrant) บังคับใช้ **Payload-based Partitioning** ในการแยก Tenant กั้นข้อมูลออกจากกันอย่างสมบูรณ์
   - API ทุกเส้นที่เปิดเผยจะถูกป้องกันด้วย **Tenant Auth Middleware** ควบคุมสิทธิ์เข้าถึงข้อมูลของแอดมินผ่าน JWT Token
3. **On-Premise IAM & RBAC**: ระบบจัดการสิทธิ์การใช้งาน (User Management) ของแอดมินจะรันในคลาวด์/เซิร์ฟเวอร์แบบ On-Premise ของตนเองทั้งหมด ข้อมูลรหัสผ่านจะถูกเข้ารหัสผ่าน Argon2id 

---

## 2. แผนการอัปเกรดระบบเพื่อมุ่งสู่ v2.3 (Gap Analysis Implementation Plans)

จากการวิเคราะห์เทียบเคียงระหว่างโค้ดปัจจุบันกับการออกแบบใหม่ แผนปฏิบัตินี้คือ Roadmap เพื่อการพัฒนาระบบที่มีประสิทธิภาพสูงและปลอดภัย โดยแยกแผนผังกระบวนการออกแบบเป็นกลุ่มได้ดังนี้:

### 2.1 🌐 Sources / Ingress Service
- **Ref Document**: `01_05_Sources_Implementation_Plan_Project-Mimir.md`
- **เป้าหมาย**: อัปเกรดระบบนำเข้าข้อมูลและแสดงผลให้โปร่งใสเรียลไทม์
- **ฟีเจอร์ใหม่ & UX/UI**:
  - สร้างหน้าจอ Real-time Monitoring / Console Logs ดักจับสถานะของการดูดข้อมูล (Ingestion) แบบติดขอบจอโดยไม่ต้องโหลดหน้าเว็บใหม่
  - รองรับแหล่งข้อมูลทั้งเว็บไซต์ องค์กรเอกสาร และ MCP Servers (เพื่อต่อกับระบบ Third-party)

### 2.2 🛡️ Quality Control (QA / Clustering)
- **Ref Document**: `01_06_Quality_Control_Implementation_Plan_Project-Mimir.md`
- **เป้าหมาย**: ป้องกันปัญหาหลอน (Hallucination) จากฐานข้อมูลซ้ำซ้อน
- **ฟีเจอร์ใหม่ & UX/UI**:
  - เครื่องมือ Cluster Viewer (รูปแบบ Kanban/Masonry Board) แสดงคำถามที่เนื้อหาใกล้เคียงกันเป็นกลุ่มๆ
  - ระบบตรวจจับ Conflict ด้วย LLM-as-a-judge เพื่อชี้จุดที่เอกสารให้ข้อมูลมาขัดแย้งกัน
  - UI สำหรับให้มนุษย์ (Admin) กด "Merge" (ควบรวม) หรือ "Resolve Conflict" (แก้ไขเนื้อหาที่ขัดแย้ง) ก่อนส่งลง Vector DB เพื่อให้เป็น Golden Answer

### 2.3 🗄️ Vector Database Management
- **Ref Document**: `01_07_Vector_Management_Implementation_Plan_Project-Mimir.md`
- **เป้าหมาย**: นำระบบ Multi-Tenancy ไปใช้จริง ปิดช่องโหว่ความปลอดภัย Qdrant 
- **ฟีเจอร์ใหม่ & UX/UI**:
  - Interactive Table ที่คลิกกางหน้าเนื้อหาดิบออกมาได้ (Expandable Rows) ทำให้ไม่ต้องงมหน้าหาแหล่งที่มา
  - Similarity Score Badges (เขียว เหลือง แดง) ให้ทราบระดับความคล้ายคลึงของข้อความ
  - ฟีเจอร์ "Delete Vector" เพื่อสะสาง Garbage Vectors ที่หมดอายุ หรือตอบคำถามผิดเพี้ยนออกจากระบบโดยตรงจากหน้า UI 

### 2.4 📊 Evaluations & Benchmarking
- **Ref Document**: `01_08_Evaluations_Implementation_Plan_Project-Mimir.md`
- **เป้าหมาย**: เครื่องมือวัดประสิทธิภาพโมเดลว่าสามารถใช้งานร่วมกันได้แค่ไหน
- **ฟีเจอร์ใหม่ & UX/UI**:
  - New Evaluation Wizard เพื่อตั้งค่า รัน ทดสอบเอเจนต์ได้ตรงจาก Dashboard (แทนการใช้ Command line) พร้อมใส่ Progress bar ระหว่างรัน
  - ระบบ Split-pane View สำหรับเทียบคำตอบจริง (Actual) และคำตอบที่ควรได้ (Expected) แบบบรรทัดต่อบรรทัด
  - "Override Score" Button ให้แอดมินใช้ดุลพินิจขัดเกลาคะแนนหากปัญญาประดิษฐ์ให้คะแนนประเมินผิด

### 2.5 👥 IAM / User Management
- **Ref Document**: `01_09_User_Management_Implementation_Plan_Project-Mimir.md`
- **เป้าหมาย**: พื้นฐานระบบ Access Control List สำหรับลูกค้าและ Tenant
- **ฟีเจอร์ใหม่ & UX/UI**:
  - เปลี่ยนหน้า Mock ให้ดึง API ฐานข้อมูลอย่างเป็นทางการ
  - เพิ่ม Add/Edit User ด้วย Sliding Drawer/Sheet แทน Inline Form เกะกะหน้าจอ
  - ผูก User เข้ากับ Role (Admin, Editor, Viewer) ของ Tenant ผ่าน Dynamic Dropdown Selection และมีปุ่ม Action ที่ปลอดภัยครบจบในหน้าเดียว

---

## 3. Deployment Strategy (Kubernetes Cloud Architecture)

เพื่อให้สอดรับกับ Multi-Tenant Architecture ที่รองรับจำนวนแอดมินและโมเดลที่มากขึ้น Deployment หลักใน Phase 3-4 จะย้ายจากการรัน Docker Compose ทั่วไปไปสู่ **Kubernetes (K8s)**

- **Ingress Service & Qdrant**: ขยายตัวเองตาม Traffic Scale (HPA) กรณีมีข้อมูลถูกป้อนเข้าสู่ระบบมากขึ้น
- **Domain Connectors**: แยก Namespace สำหรับแต่ละอุตสาหกรรม และให้มี Resource Quota ที่ชัดแจ้ง (ป้องกันปัญหา Noisy Neighbor ฉุดสแตสต์โมดูลอื่น)
- **GitOps Management**: จัดการเวิร์คโฟลว์ CI/CD สำหรับแก้ไขฐานข้อมูลด้วย GitHub Actions และ Kustomize

---

*สิ้นสุดเอกสาร TRD v2.3 — รวมศูนย์แผนอัปเกรด Multi-Tenant Modules — อัปเดตเมื่อ 2026-02-21*
