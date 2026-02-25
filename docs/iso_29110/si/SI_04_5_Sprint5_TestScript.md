# SI-04-5: Sprint 5 Test Script (Data Ingress Monitoring)
**Project Name:** Project Mimir
**Sprint:** 5
**Feature:** Data Ingress Monitoring (ระบบดูดข้อมูลและแจ้งเตือน)

## แนวทางการทดสอบตามมาตรฐาน ISO 29110 (Test Instructions & TDD Approach)
กระบวนการนี้อ้างอิงหลักการ **Test-Driven Development (TDD)** โดยต้องดำเนินการทดสอบ Unit Test ให้ผ่านก่อนการทดสอบระบบจริง และให้ทดสอบทีละข้อตามลำดับ (Step-by-Step) เพื่อให้เป็นไปตามมาตรฐานการควบคุมคุณภาพ

1. **เขียนและรัน Unit Test**: รัน Unit Test ของระบบ (ทั้ง Frontend และ Backend) ให้ผ่าน `✅ Pass` ทุกข้อก่อนเริ่มทดสอบ UI (อ้างอิงตามแนวทาง TDD)
2. **รันระบบ Environment**: รัน Database (`docker-compose up -d`), Backend (`cargo run --bin ro-ai-bridge`), และ Frontend (`npm run dev`)
3. **ทดสอบทีละข้อ (Step-by-step)**: ดำเนินการทดสอบตาม Test Scenarios ด้านล่าง **ทีละข้อ** อย่างเคร่งครัด ห้ามข้ามขั้นตอน
4. **บันทึกผลตามมาตรฐาน ISO**: 
   - บันทึกผลในช่อง **"ผลการประเมิน"** (`✅ Pass` หรือ `❌ Fail`)
   - **ต้อง** ระบุหมายเลข **Issue** และ **Pull Request (PR)** ของ GitHub ที่เกี่ยวข้องในแต่ละข้อ เพื่อให้สามารถอ้างอิงย้อนกลับได้ (Traceability) ตามมาตรฐาน ISO 29110

---

## ตารางการทำสอบตามสถานการณ์ (Test Scenarios)

### ส่วนที่ 1: การตรวจสอบระดับ Unit Test (TDD Approach)
ต้องดำเนินการในส่วนนี้ให้ผ่านทั้งหมดก่อนเริ่มทดสอบระบบผ่านหน้าจอ

| ID            | Test Scenario                  | Action / Steps (ขั้นตอนการทดสอบ)                                               | Expected Result (ผลที่คาดหวัง)                                                  | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ |
| ------------- | ------------------------------ | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ----------------------- | -------------- | ------- |
| **TC_SP5_U1** | Backend Unit Tests Executions  | 1. รันสคริปต์ `cargo test -p mimir-core-ai ingress` ในโฟลเดอร์รหัสต้นฉบับฝั่ง Backend | Unit tests สำหรับฟังก์ชัน CRUD ของ Data Sources และ Parsers ผ่านทั้งหมด (All Passed) | ✅ Pass                  | #49 / #50      | -       |
| **TC_SP5_U2** | Frontend Unit Tests Executions | 1. รันสคริปต์ `npm run test` ในโฟลเดอร์ Dashboard                                | Unit tests ของฟังก์ชันเรียก API ด้าน Frontend สำหรับ Sources ผ่านทั้งหมด (All Passed)  | ✅ Pass                  | #49 / #50      | -       |

---

### ส่วนที่ 2: การตรวจสอบแบบอัตโนมัติ (Backend API)

| ID            | Test Scenario           | Action / Steps (ขั้นตอนการทดสอบ)                                           | Expected Result (ผลที่คาดหวัง)                                             | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ |
| ------------- | ----------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------- | ----------------------- | -------------- | --------------- |
| **TC_SP5_01** | Create Data Source API  | 1. ส่ง Request `POST /api/sources` พร้อม payload ข้อมูล source (เช่น Web URL) | ระบบบันทึกข้อมูลลงฐานข้อมูลและตอบกลับด้วยรายละเอียด Source ใหม่ที่สร้างขึ้น (HTTP 201) | ✅ Pass                  | #49 / #50      |                 |
| **TC_SP5_02** | Fetch Data Sources API  | 1. ส่ง Request `GET /api/sources`                                         | ระบบคืนค่ารายการ Sources ที่เกี่ยวข้องกับ Tenant ของผู้ใช้ (HTTP 200 OK)           | ✅ Pass                  | #49 / #50      |                 |
| **TC_SP5_03** | Trigger Source Sync API | 1. ส่ง Request `POST /api/sources/:id/sync`                               | ระบบส่งสัญญาณให้ Background worker เริ่มดึงข้อมูล และตอบกลับ HTTP 202 Accepted   | ✅ Pass                  | #49 / #50      |                 |

---

### ส่วนที่ 3: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario                  | Action / Steps (ขั้นตอนการทดสอบ)                                  | Expected Result (ผลที่คาดหวัง)                                                                         | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ             |
| ------------- | ------------------------------ | --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------- | -------------- | --------------------------- |
| **TC_SP5_04** | Sources Page Access            | 1. ล็อกอินเข้าสู่แดชบอร์ด<br>2. ไปที่เมนู "Sources"                      | ระบบแสดงรายการ Data Sources จริงจาก Backend ไม่ใช่ Mock Data                                           | ✅ Pass                  | #49 / #50      |                             |
| **TC_SP5_05** | Add New Web Source             | 1. คลิกปุ่ม "Add Source"<br>2. กรอก URL ของเว็บไซต์เป้าหมาย แล้วกดบันทึก | Dialog ปิดลงและรายการใหม่ปรากฏขึ้นในตาราง Data Sources                                                  | ✅ Pass                  | #49 / #50      |                             |
| **TC_SP5_06** | Trigger Sync & Console Monitor | 1. คลิกปุ่ม "Sync" ที่รายการ Source<br>2. สังเกตที่หน้าต่าง Console Log   | ระบบจะเริ่มทำงานดึงข้อมูล พร้อมพ่น Log ออกมาบนจอแบบ Real-time (WebSockets/SSE) ให้เห็นความคืบหน้าถึงไหนและดึงกี่เพจ | ✅ Pass                  | #49 / #50      | รูปภาพวีดีโอ ui_test_sprint5   |
| **TC_SP5_07** | Delete Source                  | 1. คลิกปุ่ม "Delete" และยืนยันการลบ                                  | รายการ Source หายไปจากตารางและฐานข้อมูลสำเร็จ                                                           | ✅ Pass                  | #49 / #50      | สร้าง React Dialog แทน alert |

**สรุปผลการทดสอบ Sprint 5 (Sign-off):** 
- [x] ผ่านเกณฑ์ทั้งหมด (All Passed) นำผลไปกรอกที่ SI_04 Test Plan
- [ ] ไม่ผ่านบางส่วน (Partial Fail) - ระบุข้อที่ต้องแก้โค้ดและ Issue Tracking: _________________________________________

**อ้างอิง (GitHub References):**
- **Issue:** #49
- **Pull Request:** #50
