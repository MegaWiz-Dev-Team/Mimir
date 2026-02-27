# SI-04-9: Sprint 9 Test Script (Content Pipeline & Navigation)
**Project Name:** Project Mimir
**Sprint:** 9
**Feature:** Content Pipeline Enhancement (Chunking, Link Discovery, Cross-source Dedup) & Frontend Navigation Restructure
**ทดสอบเมื่อ:** 2026-02-27

## แนวทางการทดสอบตามมาตรฐาน ISO 29110 (Test Instructions & TDD Approach)
กระบวนการนี้อ้างอิงหลักการ **Test-Driven Development (TDD)** โดยต้องดำเนินการทดสอบ Unit Test ให้ผ่านก่อนการทดสอบระบบจริง และให้ทดสอบทีละข้อตามลำดับ (Step-by-Step) เพื่อให้เป็นไปตามมาตรฐานการควบคุมคุณภาพ

1. **เขียนและรัน Unit Test**: รัน Unit Test ของระบบ (ทั้ง Frontend และ Backend) ให้ผ่าน `✅ Pass` ทุกข้อก่อนเริ่มทดสอบ UI (อ้างอิงตามแนวทาง TDD)
2. **รันระบบ Environment**: รัน Database (`docker-compose up -d`), Backend (`cargo run --bin ro-ai-bridge`), และ Frontend (`npm run dev`)
3. **ทดสอบทีละข้อ (Step-by-step)**: ดำเนินการทดสอบตาม Test Scenarios ด้านล่าง **ทีละข้อ** อย่างเคร่งครัด ห้ามข้ามขั้นตอน
4. **บันทึกผลตามมาตรฐาน ISO**: 
   - บันทึกผลในช่อง **"ผลการประเมิน"** (`✅ Pass` หรือ `❌ Fail`)
   - **ต้อง** ระบุหมายเลข **Issue** และ **Pull Request (PR)** ของ GitHub ที่เกี่ยวข้องในแต่ละข้อ เพื่อให้สามารถอ้างอิงย้อนกลับได้ (Traceability) ตามมาตรฐาน ISO 29110

---

## ตารางการทดสอบตามสถานการณ์ (Test Scenarios)

### ส่วนที่ 1: การตรวจสอบระดับ Unit Test (TDD Approach)
ต้องดำเนินการในส่วนนี้ให้ผ่านทั้งหมดก่อนเริ่มทดสอบระบบผ่านหน้าจอ

#### 1.1 Backend Unit Tests (Rust)

| ID            | Test Scenario                                  | Action / Steps (ขั้นตอนการทดสอบ)                        | Expected Result (ผลที่คาดหวัง)                                                                                       | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ                                            |
| :------------ | :--------------------------------------------- | :---------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------- | :---------------------- | :------------- | :------------------------------------------------- |
| **TC_SP9_U1** | DB Migration (Sprint 9 Schema)                 | 1. รัน `cargo sqlx migrate run`                        | สร้าง 3 tables (chunks, crawled_pages, content_fingerprints) + ALTER data_sources สำเร็จ                             | ✅ Pass                  | #100 / PR #101 | SHOW TABLES confirmed all 3 tables                 |
| **TC_SP9_U2** | Pipeline Wiring — Extraction (UT-001a~e)       | 1. รัน `cargo test -p mimir-core-ai -- ingress`        | `process_source()` และ `process_source_with_data()` ผ่านทุก test cases                                              | ✅ Pass                  | #94 / PR #103  | 10/10 tests passed (5.07s)                         |
| **TC_SP9_U3** | Chunking Service (UT-002a~g)                   | 1. รัน `cargo test -p mimir-core-ai -- chunking`       | `chunk_fixed()`, `chunk_recursive()`, `auto_recommend()` ผ่านทั้ง 7 test cases                                       | ✅ Pass                  | #95 / PR #104  | 14/14 tests passed                                 |
| **TC_SP9_U4** | Link Discovery — OG Metadata (UT-003a~d)       | 1. รัน `cargo test -p mimir-core-ai -- link_discovery` | `parse_og_metadata()` OG title/desc/image ถูกต้อง, fallback to `<title>`, favicon extraction ถูกต้อง                  | ✅ Pass                  | #96 / PR #105  | 9/9 tests passed (shared with U5, U6)              |
| **TC_SP9_U5** | Link Discovery — Same-domain Links (UT-003e~g) | 1. รัน `cargo test -p mimir-core-ai -- link_discovery` | `discover_links()` filter same-domain, dedup, max limit, skip assets (.jpg, .css) ถูกต้อง                           | ✅ Pass                  | #96 / PR #105  | (รวมใน U4)                                         |
| **TC_SP9_U6** | Link Discovery — Content Hash (UT-003h)        | 1. รัน `cargo test -p mimir-core-ai -- link_discovery` | `compute_content_hash()` SHA-256 consistent, different content → different hash                                   | ✅ Pass                  | #96 / PR #105  | (รวมใน U4)                                         |
| **TC_SP9_U7** | Dedup — Fingerprinting (UT-004a~e)             | 1. รัน `cargo test -p mimir-core-ai -- dedup`          | `normalize_text()` lowercase+collapse whitespace, `fingerprint()` SHA-256 consistent, case/whitespace insensitive | ✅ Pass                  | #97 / PR #106  | 13/13 tests passed (includes dedup+link_discovery) |
| **TC_SP9_U8** | Dedup — SimHash Fuzzy Match (UT-004f~i)        | 1. รัน `cargo test -p mimir-core-ai -- dedup`          | `simhash()` similar texts → low hamming distance, different texts → high distance, identical → 0                  | ✅ Pass                  | #97 / PR #106  | (รวมใน U7)                                         |
| **TC_SP9_U9** | Dedup — Tracker Report (UT-004j~k)             | 1. รัน `cargo test -p mimir-core-ai -- dedup`          | `DedupTracker` counts unique/duplicate correctly, `is_seen()` tracks hashes within run                            | ✅ Pass                  | #97 / PR #106  | (รวมใน U7)                                         |

#### 1.2 Frontend Build Verification

| ID             | Test Scenario               | Action / Steps (ขั้นตอนการทดสอบ)               | Expected Result (ผลที่คาดหวัง)                                      | ผลการประเมิน (Pass/Fail) | Issue # / PR #         | หมายเหตุ                             |
| :------------- | :-------------------------- | :------------------------------------------- | :--------------------------------------------------------------- | :---------------------- | :--------------------- | :---------------------------------- |
| **TC_SP9_U10** | Frontend Build — All Routes | 1. รัน `cd ro-ai-dashboard && npx next build` | Build ผ่าน, 15 routes rendered (รวม `/knowledge` และ `/coverage`) | ✅ Pass                  | #98,#99 / PR #107,#108 | 15/15 routes (13 static, 2 dynamic) |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario                               | Action / Steps (ขั้นตอนการทดสอบ)                                                                               | Expected Result (ผลที่คาดหวัง)                                                                              | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ                                         |
| :------------ | :------------------------------------------ | :----------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------- | :---------------------- | :------------- | :------------------------------------------------------ |
| **TC_SP9_01** | Nav — 7-item Layout                         | 1. เปิดหน้า Dashboard<br>2. ตรวจสอบ Navigation bar                                                             | Nav bar แสดง 7 items: Overview · Sources · Knowledge · Quality · Playground · Coverage · Admin           | ✅ Pass                  | #98 / PR #107  | Screenshot confirmed 7 items with correct icons         |
| **TC_SP9_02** | Nav — Knowledge Placeholder                 | 1. คลิก "Knowledge" บน Nav bar                                                                                | แสดงหน้า placeholder "Knowledge Base" พร้อมข้อความ "Coming in Sprint 10"                                    | ✅ Pass                  | #98 / PR #107  | Screenshot shows BookOpen icon + badge                  |
| **TC_SP9_03** | Nav — Coverage Placeholder                  | 1. คลิก "Coverage" บน Nav bar                                                                                 | แสดงหน้า placeholder "Coverage Analytics" พร้อมข้อความ "Coming in Sprint 12"                                | ✅ Pass                  | #98 / PR #107  | Browser subagent confirmed correct page + badge         |
| **TC_SP9_04** | Nav — Existing Pages Accessible             | 1. คลิก Sources, Quality, Playground, Admin ทีละอัน                                                             | แต่ละปุ่มนำไปหน้าที่ถูกต้อง — Sources, Quality Control, Playground, Settings                                      | ✅ Pass                  | #98 / PR #107  | Browser subagent navigated all 4 pages successfully     |
| **TC_SP9_05** | Settings — Tabbed Layout                    | 1. ไปหน้า Admin (Settings)<br>2. ตรวจสอบ Tab sidebar ด้านซ้าย                                                   | แสดง 6 tabs: General, AI Models, Pipeline, Knowledge Graph, Search, Security                             | ⚠️ Deferred              | #99 / PR #108  | Blocked by auth — build verified, code review confirmed |
| **TC_SP9_06** | Settings — General Tab                      | 1. คลิก Tab "General"<br>2. ตรวจสอบ Tenant Name + Tenant ID fields                                            | แสดง Tenant Configuration card พร้อม Name (editable) + ID (readonly)                                      | ⚠️ Deferred              | #99 / PR #108  | Blocked by auth — code review confirmed                 |
| **TC_SP9_07** | Settings — AI Models Tab                    | 1. คลิก Tab "AI Models"<br>2. ตรวจสอบ Provider, Model, System Prompt, Max Tokens                              | แสดง AI Model Configuration card พร้อม fields ทั้งหมด + Save button                                         | ⚠️ Deferred              | #99 / PR #108  | Blocked by auth — code review confirmed                 |
| **TC_SP9_08** | Settings — Pipeline Tab                     | 1. คลิก Tab "Pipeline"<br>2. ตรวจสอบ Chunking Strategy, Chunk Size, Overlap, Dedup Threshold                  | แสดง Pipeline Settings card พร้อม dropdowns (Auto/Fixed/Recursive, dedup levels) + inputs (size, overlap) | ⚠️ Deferred              | #99 / PR #108  | Blocked by auth — code review confirmed                 |
| **TC_SP9_09** | Settings — Coming Soon Tabs                 | 1. คลิก Tab "Knowledge Graph", "Search", "Security" ทีละอัน                                                     | แต่ละ Tab แสดง Coming Soon placeholder พร้อมระบุ Sprint (11, 10, 14 ตามลำดับ)                                 | ⚠️ Deferred              | #99 / PR #108  | Blocked by auth — code review confirmed                 |
| **TC_SP9_10** | URL Preview Endpoint                        | 1. `curl "http://localhost:3100/api/v1/sources/preview?url=https://example.com"`                             | ได้ JSON response พร้อม url, domain, title, description, image, favicon                                    | ⚠️ Deferred              | #96 / PR #105  | Backend port 8080, needs auth header — deferred         |
| **TC_SP9_11** | Sync — Chunking Integration                 | 1. สร้าง data source แบบ web<br>2. กด Sync<br>3. ตรวจสอบ DB                                                   | `chunks` table มี records ที่ chunk_index เรียงต่อกัน, `total_chunks` ใน `data_sources` ตรงกับจำนวน chunks       | ⚠️ Deferred              | #95 / PR #104  | Requires end-to-end data — deferred to integration test |
| **TC_SP9_12** | Sync — Link Discovery (Web Source)          | 1. สร้าง data source แบบ web พร้อม URL<br>2. กด Sync<br>3. ตรวจสอบ `crawled_pages` table                       | มี record status='crawled' สำหรับ main page + records status='pending' สำหรับ discovered links                | ⚠️ Deferred              | #96 / PR #105  | Requires end-to-end data — deferred to integration test |
| **TC_SP9_13** | Sync — Cross-source Dedup                   | 1. อัปโหลดไฟล์ที่มีเนื้อหาเดียวกัน 2 ครั้ง (ชื่อต่างกัน)<br>2. Sync ทั้ง 2 sources<br>3. ตรวจสอบ `content_fingerprints` table | Source ที่ 2 มี duplicate chunks ถูก skip, `content_fingerprints` มี records เฉพาะ unique chunks              | ⚠️ Deferred              | #97 / PR #106  | Requires end-to-end data — deferred to integration test |
| **TC_SP9_14** | Pipeline Wiring — Auto-extraction on Upload | 1. Upload ไฟล์ PDF<br>2. รอ background extraction task<br>3. ตรวจสอบ `raw_markdown`                           | `raw_markdown` มีเนื้อหาที่ extract จาก PDF, สถานะเป็น `COMPLETED`, `total_chunks` > 0                         | ⚠️ Deferred              | #94 / PR #103  | Requires end-to-end data — deferred to integration test |

---

**สรุปผลการทดสอบ Sprint 9 (Sign-off):**
- [x] Unit Tests ผ่านทั้งหมด (10/10)
- [x] Frontend Nav Restructure ผ่าน (4/4 UI tests: TC_SP9_01~04)
- [ ] Settings Tabs (TC_SP9_05~09): Deferred — auth credentials issue ในสภาพแวดล้อมทดสอบ (code review ยืนยันว่า implementation ถูกต้อง)
- [ ] Integration Tests (TC_SP9_10~14): Deferred — ต้องการ end-to-end data pipeline setup

**ผลการทดสอบ 2026-02-27:**
- **Unit Tests**: 10/10 ผ่าน (U1~U10) — total 116 backend tests + 15-route frontend build
- **UI Tests**: 4/4 ผ่าน (TC_SP9_01~04), 10 deferred (TC_SP9_05~14)
- **Total**: 14/24 test cases ผ่าน (58.3%), 10 deferred

**อ้างอิง (GitHub References):**
- **Issues:** #94, #95, #96, #97, #98, #99, #100
- **Pull Requests:** PR #101, PR #103, PR #104, PR #105, PR #106, PR #107, PR #108
