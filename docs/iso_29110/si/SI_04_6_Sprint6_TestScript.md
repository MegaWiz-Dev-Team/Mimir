# SI-04-6: Sprint 6 Test Script (Background Evaluation Trigger)
**Project Name:** Project Mimir
**Sprint:** 6
**Feature:** Background Evaluation Trigger (ระบบการสั่งประเมินผลเบื้องหลัง และแถบแสดงสถานะ WebSocket)

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

| ID            | Test Scenario                  | Action / Steps (ขั้นตอนการทดสอบ)                                  | Expected Result (ผลที่คาดหวัง)                                               | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ |
| ------------- | ------------------------------ | --------------------------------------------------------------- | ------------------------------------------------------------------------- | ----------------------- | -------------- | ------- |
| **TC_SP6_U1** | Backend Unit Tests Executions  | 1. รันสคริปต์ `cargo test -p mimir-core-ai evaluations` ใน Backend | Unit tests สำหรับระบบ Queue การทำงานและ API การประเมินผลผ่านทั้งหมด (All Passed) | ✅ Pass                  |                |         |
| **TC_SP6_U2** | Frontend Unit Tests Executions | 1. รันสคริปต์ `npm run test` ใน Dashboard                          | Unit tests สำหรับ Progress Bar และ Event Handler หน้าจอ Evaluations ผ่านทั้งหมด | ✅ Pass                  |                |         |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario             | Action / Steps (ขั้นตอนการทดสอบ)                                     | Expected Result (ผลที่คาดหวัง)                                                                 | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ |
| ------------- | ------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- | ----------------------- | -------------- | --------------- |
| **TC_SP6_01** | Trigger Evaluation Job    | 1. ไปที่เมนู Evaluations<br>2. กดปุ่มเพื่อเริ่มต้นคำสั่งการประเมินผล (Start)     | ระบบส่งคำสั่ง Request ไปยัง Backend สร้างคิวงาน และคืนค่า HTTP 202 Accepted เริ่มงานบนพื้นหลัง            | ✅ Pass                  |                |                 |
| **TC_SP6_02** | Real-time Progress Bar    | 1. รอดูสถานะการทำงานหลังกดเริ่มประเมินผล<br>2. สังเกต UI แถบ Progress Bar | แถบ Progress Bar ตรวจสอบการทำกิจกรรมและเพิ่มระดับให้สอดคล้องกับคิวงานจริงผ่าน WebSocket หรือ Polling    | ✅ Pass                  |                |                 |
| **TC_SP6_03** | Evaluation Logs Execution | 1. ตรวจสอบบริเวณ Logs (ถ้ามี) การประเมินทีละขั้นตอน                       | ระบบแสดงสถานะการประเมินชุดคำถามที่ละข้อจนสำเร็จสถานะเป็น COMPLETED                                   | ✅ Pass                  |                |                 |
| **TC_SP6_04** | Finished State UI Update  | 1. เมื่อคิวงานประเมินผลเสร็จสิ้น 100%<br>2. ตรวจสอบ UI                    | Progress Bar เต็ม 100% และแสดงข้อความสำเสร็จ พร้อมมีปุ่มให้สามารถเริ่มทำงานใหม่ (Restart / Run Again) ได้ | ✅ Pass                  |                |                 |

**สรุปผลการทดสอบ Sprint 6 (Sign-off):** 
- [x] ผ่านเกณฑ์ทั้งหมด (All Passed) นำผลไปกรอกที่ SI_04 Test Plan
- [ ] ไม่ผ่านบางส่วน (Partial Fail) - ระบุข้อที่ต้องแก้โค้ดและ Issue Tracking: _________________________________________

**อ้างอิง (GitHub References):**
- **Issues:** #
- **Pull Requests:** #
