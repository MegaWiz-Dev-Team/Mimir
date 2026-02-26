# SI-04-7: Sprint 7 Test Script (UX/UI Pipeline Refinement & Traceability)
**Project Name:** Project Mimir
**Sprint:** 7
**Feature:** UX/UI Pipeline Refinement & Traceability (การปรับปรุง UI/UX ตลอดสายงานข้อมูลและการติดตาม)

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

| ID            | Test Scenario                  | Action / Steps (ขั้นตอนการทดสอบ)                                    | Expected Result (ผลที่คาดหวัง)                                                   | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ |
| ------------- | ------------------------------ | ----------------------------------------------------------------- | ----------------------------------------------------------------------------- | ----------------------- | -------------- | ------- |
| **TC_SP7_U1** | Frontend Unit Tests Executions | 1. รันสคริปต์ `npm run test` ในโฟลเดอร์ Dashboard (`ro-ai-dashboard`) | Unit tests ของ Components ใหม่ (Navbar, PipelineStatusBar, Sources, QC) ผ่านหมด | ✅ Pass                  | #61 / #56      |         |
| **TC_SP7_U2** | API Error Handling Tests       | 1. รันสคริปต์ `npm run test src/lib/api.test.ts`                     | ระบบทดสอบการ Throw Error เมื่อเรียก QC API ล้มเหลวผ่านทั้งหมด                        | ✅ Pass                  | #57 / #70      |         |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario                 | Action / Steps (ขั้นตอนการทดสอบ)                               | Expected Result (ผลที่คาดหวัง)                                                                | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ           |
| ------------- | ----------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------ | ----------------------- | -------------- | ------------------------- |
| **TC_SP7_01** | Global Pipeline Status Bar    | 1. ล็อกอินเข้าสู่แดชบอร์ดด้วยบัญชี Admin<br>2. สังเกตแถบด้านบน (Navbar) | แถบ Navigation แสดง Pipeline Status (Generating, Pending QC, Vectorized) อัปเดตข้อมูลอัตโนมัติ   | ✅ Pass                  | #68 / #69      | Hotfix Hydration Mismatch |
| **TC_SP7_02** | Sources Markdown Preview      | 1. ไปที่เมนู Sources<br>2. คลิกที่ปุ่มเปิด Preview ของรายการ Source   | ระบบจะแสดงหน้าต่าง Dialog แสดง Markdown / Text Preview ของข้อมูลพร้อมปุ่มบันทึก/ยกเลิก               | ✅ Pass                  | #56 / #56      |                           |
| **TC_SP7_03** | Knowledge Base Coverage       | 1. ไปที่เมนู Knowledge Base<br>2. ตรวจสอบแถบ Progress Bar       | ระบบแสดงแถบสีบอกเปอร์เซ็นต์ ACU Coverage และ Highlighting ของข้อมูลในตาราง                       | ✅ Pass                  | #56 / #56      |                           |
| **TC_SP7_04** | QC Kanban & Conflict Resolve  | 1. ไปที่เมนู Quality Control<br>2. คลิกการ์ดที่มีสถานะ Conflict      | ระบบจะเปิด Dialog ให้เลือกเปรียบเทียบคำตอบ 2 เวอร์ชั่น (Side-by-side) เพื่อหา Golden Answer           | ✅ Pass                  | #56 / #56      |                           |
| **TC_SP7_05** | Vector DB Traceability Badges | 1. ไปที่เมนู Vector Database<br>2. ตรวจสอบข้อมูลในตารางค้นหา       | ทุกรายการข้อมูลเวกเตอร์มี Traceability Badges แสดงแหล่งที่มา (Source) หรือรหัสผู้บันทึก (Approval) ชัดเจน | ✅ Pass                  | #56 / #56      |                           |

**สรุปผลการทดสอบ Sprint 7 (Sign-off):** 
- [x] ผ่านเกณฑ์ทั้งหมด (All Passed) นำผลไปกรอกที่ SI_04 Test Plan
- [ ] ไม่ผ่านบางส่วน (Partial Fail) - ระบุข้อที่ต้องแก้โค้ดและ Issue Tracking: _________________________________________

**อ้างอิง (GitHub References):**
- **Issues:** #57, #61, #66, #68
- **Pull Requests:** #56, #67, #69, #70
