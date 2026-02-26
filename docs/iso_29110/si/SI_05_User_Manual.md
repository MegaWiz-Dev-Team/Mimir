# SI-05: User Manual (คู่มือการใช้งาน)
**Project Name:** Project Mimir
**Sprint:** 7 (Final - Documenting Complete Flow)

## 1. System Overview (ภาพรวมระบบ)
Project Mimir เป็นระบบ AI แพลตฟอร์มแบบ Multi-Tenant ที่ให้ผู้บริหารระบบ (SuperAdmin) สามารถแบ่งแยกพื้นที่ทำงาน (Workspace) ให้กับแต่ละ Tenant (หรือโปรเจกต์) ได้อย่างเป็นอิสระ โดยแต่ละ Tenant จะสามารถจัดการโมเดล AI (LLM), Vector Database (RAG) และทดสอบระบบผ่าน AI Playground ของตนเองได้โดยไม่ข้องแวะกับข้อมูลของ Tenant อื่น

ในคู่มือฉบับปรับปรุงนี้ จะนำเสนอรูปแบบการใช้งานตั้งแต่การล็อกอิน ตลอดจนถึงการประเมินคุณภาพของ AI ในขั้นตอนสุดท้าย เพื่อให้ผู้ใช้งานสามารถทำตามได้อย่างกะทัดรัดและเข้าใจง่าย

---

## 2. Getting Started (การเริ่มต้นใช้งาน)
### 2.1 การเข้าสู่ระบบ (Login)
หน้าจอล็อกอินถูกออกแบบมาให้เรียบง่ายและปลอดภัย รองรับการเข้าถึงสำหรับทั้ง SuperAdmin และ Tenant User
1. เปิดเบราว์เซอร์และเข้าไปที่ `http://localhost:3000` (หรือ URL ของเซิร์ฟเวอร์ที่ติดตั้ง)
2. กรอก **Email** และ **Password** ที่ได้รับมอบหมาย
   - *สิทธิ์ SuperAdmin สำหรับจัดการทุก Tenant (เช่น `admin@superadmin.com`)*
   - *สิทธิ์ Admin ของ Tenant เฉพาะเจาะจง (เช่น `admin@mimir.local`)*
3. กด **Sign In** เพื่อเข้าสู่ระบบ หากรหัสผ่านถูกต้อง ระบบจะพาคุณเข้าสู่หน้าจอ Dashboard อัตโนมัติ

![หน้าจอล็อกอิน (Login Screen)](./images/mockups/mock_login.png)

---

## 3. Core Features & Usage (ฟีเจอร์หลักและการใช้งาน)

### 3.1 การเข้าถึงและตั้งค่าระบบ (Tenants, Settings & Users)

#### การจัดการพื้นที่ทำงาน (Tenant Management) - เฉพาะ SuperAdmin
ฟีเจอร์นี้สงวนไว้สำหรับผู้ดูแลระบบระดับสูงสุด ใช้ในการสร้างพื้นที่ทำงานใหม่เพื่อแยกข้อมูลให้อิสระต่อกัน (Data Isolation)
1. ล็อกอินด้วยบัญชีระดับ SuperAdmin และคลิก **Tenants**
2. กดปุ่ม **+ Add Tenant** มุมขวาบน ใส่ชื่อและอีเมลดั้งต้น การกระทำนี้จะสร้างฐานข้อมูลเฉพาะให้ทันที

![หน้ารายการจัดการพื้นที่ทำงาน (Tenant Management)](./images/mockups/mock_tenant.png)

#### การตั้งค่า Tenant Configuration (Settings)
ในแต่ละ Workspace ผู้ดูแลระบบของพื้นที่สามารถปรับแต่ง AI ของตัวเองได้:
1. ที่แถบเมนูด้านซ้ายคลิก **Settings**
2. เลือกระบบ AI (Provider) และระบุชื่อ Model (เช่น `gemini-2.5-flash`)
3. กรอก API Key และกด **Save Configuration** 

![หน้าต่างการตั้งค่า (Settings)](./images/mockups/mock_settings.png)

#### การจัดการผู้ใช้งาน (Users)
1. คลิกเมนู **Users**
2. ระบบจะแสดงรายชื่อบัญชีผู้ใช้งานทั้งหมดที่มีสิทธิ์เข้าถึง Tenant ปัจจุบัน
3. คุณสามารถกด **+ Invite User** เพื่อมอบสิทธิ์ (เช่น Admin หรือ Reviewer) และสามารถเตะออกได้ด้วยรูปถังขยะ

![หน้ารายชื่อผู้ใช้งาน (Users)](./images/mockups/mock_users.png)

---

### 3.2 การจัดการคลังข้อมูลและการตรวจสอบ (Sources, QC, Vector DB)

#### ควบคุมข้อมูลเข้า (Sources & Pipeline)
ระบบ Mimir สามารถประมวลผลคำสั่งอัตโนมัติ 4 ขั้นตอน: (Sources -> Generating -> Pending QC -> Vectorized)
1. ไปที่เมนู **Sources** ระบบจะแสดงรายการแพ็กเกจข้อมูล 
2. กดปุ่ม **Run Pipeline** ให้ระบบดึงข้อมูลไปวิ่งบน Background Job

![หน้าจอควบคุมข้อมูลและดูสถิติ (Data Pipeline Dashboard)](./images/mockups/mock_dashboard.png)

#### การควบคุมคุณภาพเนื้อหา (Quality Control)
หาก AI สกัดเจอข้อมูลที่ขัดแย้งหรือซ้ำซ้อนกัน ข้อมูลจะมาพักไว้ที่หน้านี้เพื่อรอมนุษย์ยืนยัน
1. ไปที่เมนู **Quality Control**
2. ในบอร์ด Kanban ฝั่ง "Pending Review" จะเห็นการ์ดแยกตามประเภท (`CONFLICT` หรือ `DUPLICATE`)
3. กดปุ่ม **Resolve** เพื่อควบรวมคำตอบก่อนส่งต่อให้ Vector DB ในท้ายที่สุด

![หน้าจอ Quality Control Kanban](./images/mockups/mock_qc.png)

#### ฐานข้อมูลเวกเตอร์ (Vector Explorer)
หลังจากการทำ QC เรียบร้อย ข้อมูลจะถูกแพ็คลงฐานข้อมูล:
1. ไปที่เมนู **Vector DB** เพื่อตรวจสอบ Chunk ย่อยของข้อมูลเอกสาร
2. สามารถตรวจสอบดูว่าได้ป้ายกำกับ (Badge) `Auto-Verified` หรือ `QC Approved` หรือไม่ ซึ่งจะช่วยเรื่อง Traceability

![หน้าต่างตรวจสอบเวกเตอร์ข้อมูล (Vector Explorer)](./images/mockups/mock_vector.png)

---

### 3.3 การทดสอบและการวัดผล AI (Playground & Evaluations)

#### การจำลองการสนทนา (Playground)
ใช้สำหรับทดสอบ Agent ว่าเข้าใจข้อมูลที่เราเพิ่งฝีดเข้าไปหรือไม่ โดยไม่ต้องไปเขียนโค้ดเรียกใช้งาน
1. คลิกเมนู **Playground** จากแถบซ้าย
2. เลือก Role-play จากแบนเนอร์ด้านบน 
3. พิมพ์โต้ตอบกับ AI เมื่อ AI ใช้แหล่งข้อมูลใด จะมีกล่องอ้างอิงและ Action Code แสดงขึ้นมาให้วิเคราะห์เบื้องหลัง

![หน้าต่างทดสอบแชทบอท (Playground)](./images/mockups/mock_playground.png)

#### การวัดผลด้วยชุดข้อสอบ (Agent Evaluations)
การทดสอบว่า AI ที่ผู้ใช้ปรับแต่ง/สอนเนื้อหาไป สามารถตอบคำถามได้ถูกต้องเที่ยงตรง (ไม่หลอน) มากแค่ไหน:
1. ไปที่เมนู **Evaluations** จากแถบเมนูด้านซ้าย
2. กดปุ่ม **✨ New Evaluation Wizard** เพื่อสั่งให้ระบบตั้งคำถามเข้าหา AI 
3. หน้าจอจะประมวลผลคะแนน 3 ด้าน: Accuracy, Completeness, และ Relevance ออกมาเป็นเปอร์เซ็นต์
4. ตารางเปรียบเทียบ (Heatmap Grid) จะเป็นเครื่องช่วยตัดสินใจ (มีตัว 🌟 บอก Winner)

![หน้าจอแสดงผลการประเมินเอเจนต์ (Agent Evaluations Heatmap)](./images/mockups/mock_eval.png)

---
*บันทึกโดย: AI Assistant (คู่มือฉบับสมบูรณ์สำหรับ Project Mimir - Phase 1 ล่าสุด)*
