# SI-04-4: Sprint 4 Test Script (Quality Control & Hallucination Prevention)
**Project Name:** Project Mimir
**Sprint:** 4
**Feature:** Quality Control & Hallucination Prevention Dashboard

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

| ID            | Test Scenario                  | Action / Steps (ขั้นตอนการทดสอบ)                                       | Expected Result (ผลที่คาดหวัง)                                                                    | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ |
| ------------- | ------------------------------ | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ----------------------- | -------------- | ------- |
| **TC_SP4_U1** | Backend Unit Tests Executions  | 1. รันสคริปต์ `cargo test -p mimir-core-ai` ในโฟลเดอร์รหัสต้นฉบับฝั่ง Backend | Unit tests สำหรับฟังก์ชันดึงค่า JSON ของ Gemini ฝั่ง Backend และ Clustering Logic ผ่านทั้งหมด (All Passed) | ✅ Pass                  | #37 / #38      | -       |
| **TC_SP4_U2** | Frontend Unit Tests Executions | 1. รันสคริปต์ `npm run test` ในโฟลเดอร์ Dashboard                        | Unit tests ของฟังก์ชันเรียก API ด้าน Frontend (fetch, resolve, generate) ผ่านทั้งหมด (All Passed)      | ✅ Pass                  | #37 / #38      | -       |
| **TC_SP4_U3** | UI State Unit Tests Executions | 1. รันสคริปต์ `npm run test` เพื่อทดสอบ Component Frontend                | Unit tests สำหรับการแสดงผลสถานะ Loading และ Disabling ของปุ่ม "Auto-scan QC issues" ผ่านทั้งหมด       | ✅ Pass                  | #39            | -       |

---

### ส่วนที่ 2: การตรวจสอบแบบอัตโนมัติ (Backend API & Background Worker)

| ID            | Test Scenario                     | Action / Steps (ขั้นตอนการทดสอบ)                                     | Expected Result (ผลที่คาดหวัง)                                                                                                       | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ |
| ------------- | --------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | -------------- | --------------- |
| **TC_SP4_01** | Seed Mock Data API                | 1. ส่ง Request `POST /api/v1/qc/seed` โดยแนบข้อมูล QA Pairs จำลองเข้าไป | ระบบตอบกลับด้วย HTTP 200 OK และระบุจำนวน QA ที่ถูกเพิ่ม (`{"inserted": N}`). ข้อมูลจะถูกบันทึกลงในตาราง `qa_results` สำเร็จ                       | ✅ Pass                  | #37 / #38      |                 |
| **TC_SP4_02** | Trigger Background Clustering API | 1. ส่ง Request `POST /api/v1/qc/generate`                           | ระบบแจ้ง HTTP 200 OK และมีข้อความแจ้งเตือนว่าเข้าสู่กระบวนการ Background Job. ระบบจะเรียกใช้ Gemini LLM เพื่อจัดกลุ่มข้อมูล (Cluster) แบบเบื้องหลัง    | ✅ Pass                  | #37 / #38      |                 |
| **TC_SP4_03** | List Clusters API                 | 1. ส่ง Request `GET /api/v1/qc/clusters?status=PENDING`             | ระบบคืนค่า HTTP 200 OK พร้อมข้อมูล JSON Array ที่มีโครงสร้าง `ClusterDTO` ประกอบด้วย topic, reason และ properties ภายใน `items` ของ Cluster | ✅ Pass                  | #37 / #38      |                 |

---

### ส่วนที่ 3: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario                   | Action / Steps (ขั้นตอนการทดสอบ)                                                                                                                | Expected Result (ผลที่คาดหวัง)                                                                                                                 | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ                                                                                                                                         |
| ------------- | ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **TC_SP4_04** | Quality Control Page Access     | 1. ล็อกอินเข้าสู่แดชบอร์ดด้วยสิทธิ์แอดมิน<br>2. ไปที่เมนู "Quality Control" จาก Sidebar ด้านซ้าย                                                              | แดชบอร์ดแสดงหน้าต่าง Kanban Board ซึ่งประกอบด้วยคอลัมน์ "Pending Review", "Resolved", และ "Ignored" อย่างถูกต้อง                                       | ✅ Pass                  | #37 / #38      | ![Kanban Load](/Users/paripolt/.gemini/antigravity/brain/0e601dc7-a32e-48ea-a343-65b9cdb8eca8/initial_load_qc_empty_1771956730618.png)                  |
| **TC_SP4_05** | Fetch Pending Clusters          | 1. ดูที่คอลัมน์ "Pending Review" บน Kanban Board                                                                                                   | ระบบแสดงการ์ดรายการ Cluster (ข้อมูลที่ทับซ้อนหรือขัดแย้งกัน) ที่ดึงมาจาก API ตรวจสอบว่ามีการแสดงหัวข้อ (Topic), ประเภท (CONFLICT/DUPLICATE) และเนื้อหา Q/A คู่กรณี | ✅ Pass                  | #37 / #38      | -                                                                                                                                                       |
| **TC_SP4_06** | Resolve Cluster: Accept A or B  | 1. ลากการ์ดที่มีป้าย `CONFLICT` จากคอลัมน์ "Pending Review" ไปวางในคอลัมน์ "Resolved"<br>2. ในหน้าต่าง Modal ที่ปรากฏขึ้น ให้เลือกกด 'Accept A' หรือ 'Accept B' | ระบบเรียกใช้ API `/api/v1/qc/resolve/{id}` พร้อมกับพารามิเตอร์ `ACCEPT_A` หรือ `ACCEPT_B` และการ์ดถูกย้ายไปที่คอลัมน์ "Resolved" สำเร็จ                     | ✅ Pass                  | #37 / #38      | ![Conflict](/Users/paripolt/.gemini/antigravity/brain/0e601dc7-a32e-48ea-a343-65b9cdb8eca8/conflict_resolution_modal_1771957218944.png)                 |
| **TC_SP4_07** | Resolve Cluster: Merge          | 1. ลากการ์ดที่มีป้าย `DUPLICATE` จากคอลัมน์ "Pending Review" ไปวางในคอลัมน์ "Resolved"<br>2. พิมพ์คำตอบใหม่ที่เป็นการรวมข้อมูล (Golden Answer) และกดยืนยัน        | ระบบเรียกใช้ API พร้อบกับพารามิเตอร์ `MERGE` และบันทึกคำตอบ `golden_answer` ลงฐานข้อมูล การ์ดเปลี่ยนสถานะและแสดงผลใน "Resolved" สำเร็จ                      | ✅ Pass                  | #37 / #38      | ![Duplicate](/Users/paripolt/.gemini/antigravity/brain/0e601dc7-a32e-48ea-a343-65b9cdb8eca8/duplicate_resolution_modal_1771957291575.png)               |
| **TC_SP4_08** | Auto-scan QC UI Feedback Status | 1. กดปุ่ม "Auto-scan QC issues"<br>2. สังเกตการเปลี่ยนแปลงบนหน้าจอ<br>3. กด Refresh (F5) ขณะที่ระบบยังทำงานอยู่                                           | ปุ่มเปลียนเป็นสถานะกำลังโหลด (Disabled) ทันที มีข้อความว่า 'Scanning...' และเมื่อกด Refresh ปุ่มยังคงทับซ้อนและกดซ้ำไม่ได้จนกว่าจะจบ Job เบื้องหลัง                   | ✅ Pass                  | #39            | ![Final Verification](/Users/paripolt/.gemini/antigravity/brain/0e601dc7-a32e-48ea-a343-65b9cdb8eca8/qc_autoscan_final_verification_1771988212110.webp) |
| **TC_SP4_09** | Vector Stats API Resolution     | 1. เข้าหน้าจอ `/vector` หรือยิง API `GET /api/v1/vector/stats`                                                                                    | ระบบตอบกลับด้วย HTTP 200 OK และหน้าจอแสดงข้อมูลสถิติจาก Qdrant ได้สำเร็จโดยไม่ติดปัญหา 404                                                               | ✅ Pass                  | #41 / #42      | -                                                                                                                                                       |

**สรุปผลการทดสอบ Sprint 4 (Sign-off):** 
- [x] ผ่านเกณฑ์ทั้งหมด (All Passed) นำผลไปกรอกที่ SI_04 Test Plan
- [ ] ไม่ผ่านบางส่วน (Partial Fail) - ระบุข้อที่ต้องแก้โค้ดและ Issue Tracking: _________________________________________

**อ้างอิง (GitHub References):**
- **Issue:** #37, #39, #41 (Sprint 4 - Quality Control & Hallucination Prevention)
- **Pull Request:** #38, #42
