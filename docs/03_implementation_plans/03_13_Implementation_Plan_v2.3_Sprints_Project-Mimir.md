# 🚀 Implementation Plan v2.3: Multi-Tenant Modular Architecture & UX/UI Redesign

## 🎯 Overview
เอกสารฉบับนี้เป็นการสรุปรวบยอดแผนงาน (Implementation Plan) ทั้งหมด 5 โมดูลที่ได้ถูกวิเคราะห์ Gap Analysis ไว้ตามสเปกของ **TRD v2.3** เพื่อเปลี่ยนผ่านระบบจากการเป็น *Single-Tenant Monolithic* ไปสู่ **Multi-Tenant Modular Architecture** พร้อมกับการยกระดับประสบการณ์ผู้ใช้งาน (UX/UI Redesign) อย่างเต็มรูปแบบ

---

## 📅 Sprint Roadmap (ระยะเวลาโดยประมาณ: 5 Sprints)

ในการนำระบบขึ้นสู่โปรดักชันจริงภายใต้สถาปัตยกรรมใหม่ จำเป็นต้องทำตามลำดับความสำคัญ (Dependency) ดังนี้:

### 🏃 Sprint 1: Security Foundation & IAM (ระบบยืนยันตัวตนและจัดการสิทธิ์)
**เป้าหมาย:** วางรากฐานระบบ Multi-Tenancy ให้แน่นหนา เพื่อให้ทุกระบบที่จะพัฒนาต่อจากนี้สามารถดึง `tenant_id` มากรองข้อมูลได้อย่างถูกต้อง
**อ้างอิง:** `01_09_User_Management_Implementation_Plan_Project-Mimir.md`

- **Backend (API & Database)**:
  - สร้าง CRUD APIs (`GET, POST, PATCH, DELETE`) สำหรับจัดการ Users, Tenants, และ Roles (Admin, Editor, Viewer) บนฐานข้อมูล `MariaDB`
  - ใช้ `Argon2id` เข้ารหัสผ่าน และออก `JWT` ที่บรรจุค่า `tenant_id`
  - สร้าง `tenant_auth_middleware` เพื่อปกป้อง API เส้นอื่นๆ ของระบบ
- **Frontend (UX/UI)**:
  - รื้อกระดาน Mock Data ทิ้ง ทำหน้า Dashboard สำหรับจัดการผูัใช้งานจริง
  - เปลี่ยนฟอร์มกรอกข้อมูลยาวๆ เป็น **Sliding Drawer / Modal** 
  - เพิ่ม **Interactive Data Table** ที่สามารถ Filter ค้นหาตาม Tenant ID

---

### 🏃 Sprint 2: Data Isolation & Vector Management + Tenant Settings (การปกป้องและจัดการฐานข้อมูลจำลองและตั้งค่าผู้ใช้งาน)
**เป้าหมาย:** บังคับใช้การแยกข้อมูลของลูกค้าแต่ละรายออกจากกันอย่างเด็ดขาดทั้งใน RDBMS และ Vector DB พร้อมทั้งให้แอดมินจัดการขยะใน Qdrant ได้ง่ายขึ้น และเพิ่มหน้าต่าง Settings สำหรับจัดการชื่อและข้อมูลของ Tenant
**อ้างอิง:** `01_07_Vector_Management_Implementation_Plan_Project-Mimir.md`

- **Backend (API & Database)**:
  - ทำ Database Migrations (`ALTER TABLE`) เติมคอลัมน์ `tenant_id` ลงในทุกตารางที่ยังตกหล่น (เช่น `pipeline_runs`, `eval_runs`, etc.)
  - อัปเดต Ingestion Pipeline ให้ใส่ Payload `tenant_id` และ `is_active` ลงใน Qdrant ทุกครั้ง
  - แก้ไขโค้ด `POST /api/vector/search` ให้เลิกฮาร์ดโค้ด `"default_tenant"` และให้อิงตาม Token ผู้ใช้แทน
  - สร้าง API `GET/PUT /api/v1/tenants/me` สำหรับแก้ไขข้อมูล Name และจัดการ Tenant ของผู้ใช้ปัจจุบัน
- **Frontend (UX/UI)**:
  - เพิ่มหน้าต่าง **Settings** สำหรับแก้ไขชื่อผู้ใช้งานและจัดการข้อมูล Tenant 
  - เพิ่มปุ่ม **"Delete Vector" 🗑️** ในหน้าผลการค้นหา เพื่อให้แอดมินลบข้อมูลเพี้ยนจากระบบ RAG ทิ้งได้ทันที
  - ทำ **Expandable Rows** คลิกเพื่อกางดูเนื้อหา Document ต้นฉบับเต็มรูปแบบ
  - เพิ่ม **Similarity Score Badges** นอกเหนือจากตัวเลขเฉยๆ (เขียว/เหลือง/แดง)

### 🏃 Sprint 4: Quality Control & Hallucination Prevention (ระบบตีย่อยและรักษาคุณภาพข้อมูล)
**เป้าหมาย:** ป้องกันไม่ให้แอดมินทำ RAG ด้วยชุดข้อมูลที่ขัดแย้งกันเอง (Conflict) หรือข้อมูลซ้ำซ้อนจนล้น (Duplicate)
**อ้างอิง:** `01_06_Quality_Control_Implementation_Plan_Project-Mimir.md`

- **Backend (QA/QC Service)**:
  - สร้าง Background Worker สำหรับทำ **Data Clustering** (จัดกลุ่มคำถาม-คำตอบที่มีความหมายเดียวกันด้วย Embedding)
  - นำ LLM (เช่น Gemini) มาทำหน้าที่ **Consensus / Conflict Checker**
- **Frontend (QC Dashboard - 🌟 Feature ใหม่)**:
  - สร้างหน้าจอ Kanban/Masonry Grid แสดงกลุ่มของคำถาม
  - สร้าง UI ฝั่ง **Conflict Resolution**: แสดงหน้าต่างแยกซ้าย-ขวา เปรียบเทียบข้อมูลที่ขัดแย้งให้มนุษย์รีวิวและเลือกทางที่ถูก
  - สร้าง UI ฝั่ง **Duplicate Merge**: ยุบรวมข้อมูลสร้างเป็นตัวเลือก Golden Answer เดียวส่งลง Vector DB

---

### 🏃 Sprint 5: Data Ingress Monitoring (ระบบดูดข้อมูลและแจ้งเตือน)
**เป้าหมาย:** ปรับปรุงขั้นตอนการเริ่มต้นระบบให้ลื่นไหล มั่นใจได้ว่าการดูดข้อมูลหน้าเว็บไซต์หรือเอกสารไม่ตายกลางทาง
**อ้างอิง:** `01_05_Sources_Implementation_Plan_Project-Mimir.md`

- **Backend (Ingress API)**:
  - สร้าง CRUD API รองรับแหล่งข้อมูลหลายประเภท (Web URL, File Upload, MCP Connection)
  - ใช้ `tokio::mpsc` หรือ Redis Pub/Sub ดักจับ Output จาก Ingestion Script พ่นออกเป็น Server-Sent Events (SSE) หรือ WebSockets
- **Frontend (UX/UI)**:
  - ลบ UI เก่าที่เป็นแค่หน้า Mock ทิ้ง
  - สร้าง **Real-time Console / Streaming Logs UI** สำหรับดูการทำงานของ Bot Crawler ว่าทำงานถึงไหน ดึงมากี่เพจ เจอ Error ตรงไหน ทันทีโดยไม่ต้องกด F5

---

### 🏃 Sprint 6: Agent Evaluations System (ระบบสถิติและวัดผลปัญญาประดิษฐ์)
**เป้าหมาย:** ทำให้แอดมินเลือกใช้งานโมเดลราคาถูกหรือแพงได้อย่างเหมาะสม และพิสูจน์ความฉลาดของระบบได้อย่างเป็นรูปธรรม
**อ้างอิง:** `01_08_Evaluations_Implementation_Plan_Project-Mimir.md`

- **Backend (Eval Service)**:
  - รื้อ Command line script `run_eval.rs` ออกมาสร้างเป็นตัว Background Job Endpoint (`POST /api/v1/eval/run`)
  - อัปเดตให้ Evaluator ดึงข้อมูลชุดทดสอบโดยคัดกรองจากตาราง `qa_results` อิงตาม `tenant_id` ของค่ายนั้น
- **Frontend (UX/UI)**:
  - หน้าต่าง **"New Evaluation Wizard"** สไตล์ Step-by-Step (เลือก Agent -> เลือก Model -> คอนเฟิร์ม) แทนคำสั่ง Terminal
  - **Real-time Progress Bar** ขณะคอยให้ระบบรันคะแนนโมเดลนับ백ๆ คำถาม
  - **Inline Override Score**: ให้สิทธิมนุษย์ในการตบตีคะแนนของ LLM-judge หากมันให้คะแนนลำเอียง
  - อัปเกรดตาราง **Heatmap Tooltips** (Agent x Model) โชว์ดาว 🌟 Best in Class

---

## 🛠️ สรุปแผนปฏิบัติการสำหรับนักพัฒนา (Developer Checklist)
1. **[  ] Database Migration:** เขียนไฟล์ SQL สร้างตาราง (ผู้ใช้) และอัปเดตแก้อีกหลายตารางที่มีอยู่ (ใส่ `tenant_id`)
2. **[  ] Middleware Auth:** Implement Axum Middleware ควบคุมสิทธิ์ Tenant ทุกเส้นทาง API
3. **[  ] API Routing:** นำโค้ด Background Job (Crawler, Clustering, Evaluation) มาครอบด้วย REST Route (ส่วนมากเป็น `202 Accepted` response)
4. **[  ] Dashboard Refactoring:** แก้ไขหน้าเว็บครอบ React Context/Auth State และทยอย Implement หน้าจอ UI ตาม Sprints 1 ถึง 5 

*นำร่องโดย Antigravity AI Agent & Human collaborator*
