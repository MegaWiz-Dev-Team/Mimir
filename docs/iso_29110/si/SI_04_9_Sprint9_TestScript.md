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

| ID            | Test Scenario                                  | Action / Steps (ขั้นตอนการทดสอบ)                        | Expected Result (ผลที่คาดหวัง)                                                                                       | ผลการประเมิน | Issue # / PR # | หมายเหตุ                               |
| :------------ | :--------------------------------------------- | :---------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------- | :---------- | :------------- | :------------------------------------ |
| **TC_SP9_U1** | DB Migration (Sprint 9 Schema)                 | 1. รัน `cargo sqlx migrate run`                        | สร้าง 3 tables (chunks, crawled_pages, content_fingerprints) + ALTER data_sources สำเร็จ                             | ✅ Pass      | #100 / PR #101 | SHOW TABLES confirmed all 3 tables    |
| **TC_SP9_U2** | Pipeline Wiring — Extraction (UT-001a~e)       | 1. รัน `cargo test -p mimir-core-ai -- ingress`        | `process_source()` และ `process_source_with_data()` ผ่านทุก test cases                                              | ✅ Pass      | #94 / PR #103  | 10/10 tests passed (5.07s)            |
| **TC_SP9_U3** | Chunking Service (UT-002a~g)                   | 1. รัน `cargo test -p mimir-core-ai -- chunking`       | `chunk_fixed()`, `chunk_recursive()`, `auto_recommend()` ผ่านทั้ง 7 test cases                                       | ✅ Pass      | #95 / PR #104  | 14/14 tests passed                    |
| **TC_SP9_U4** | Link Discovery — OG Metadata (UT-003a~d)       | 1. รัน `cargo test -p mimir-core-ai -- link_discovery` | `parse_og_metadata()` OG title/desc/image ถูกต้อง, fallback to `<title>`, favicon extraction ถูกต้อง                  | ✅ Pass      | #96 / PR #105  | 9/9 tests passed (shared with U5, U6) |
| **TC_SP9_U5** | Link Discovery — Same-domain Links (UT-003e~g) | 1. รัน `cargo test -p mimir-core-ai -- link_discovery` | `discover_links()` filter same-domain, dedup, max limit, skip assets (.jpg, .css) ถูกต้อง                           | ✅ Pass      | #96 / PR #105  | (รวมใน U4)                            |
| **TC_SP9_U6** | Link Discovery — Content Hash (UT-003h)        | 1. รัน `cargo test -p mimir-core-ai -- link_discovery` | `compute_content_hash()` SHA-256 consistent, different content → different hash                                   | ✅ Pass      | #96 / PR #105  | (รวมใน U4)                            |
| **TC_SP9_U7** | Dedup — Fingerprinting (UT-004a~e)             | 1. รัน `cargo test -p mimir-core-ai -- dedup`          | `normalize_text()` lowercase+collapse whitespace, `fingerprint()` SHA-256 consistent, case/whitespace insensitive | ✅ Pass      | #97 / PR #106  | 13/13 tests passed                    |
| **TC_SP9_U8** | Dedup — SimHash Fuzzy Match (UT-004f~i)        | 1. รัน `cargo test -p mimir-core-ai -- dedup`          | `simhash()` similar texts → low hamming distance, different texts → high distance, identical → 0                  | ✅ Pass      | #97 / PR #106  | (รวมใน U7)                            |
| **TC_SP9_U9** | Dedup — Tracker Report (UT-004j~k)             | 1. รัน `cargo test -p mimir-core-ai -- dedup`          | `DedupTracker` counts unique/duplicate correctly, `is_seen()` tracks hashes within run                            | ✅ Pass      | #97 / PR #106  | (รวมใน U7)                            |

#### 1.2 Frontend Build Verification

| ID             | Test Scenario               | Action / Steps (ขั้นตอนการทดสอบ)               | Expected Result (ผลที่คาดหวัง)                                      | ผลการประเมิน | Issue # / PR #         | หมายเหตุ                             |
| :------------- | :-------------------------- | :------------------------------------------- | :--------------------------------------------------------------- | :---------- | :--------------------- | :---------------------------------- |
| **TC_SP9_U10** | Frontend Build — All Routes | 1. รัน `cd ro-ai-dashboard && npx next build` | Build ผ่าน, 15 routes rendered (รวม `/knowledge` และ `/coverage`) | ✅ Pass      | #98,#99 / PR #107,#108 | 15/15 routes (13 static, 2 dynamic) |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario                               | Action / Steps (ขั้นตอนการทดสอบ)                                                                                     | Expected Result (ผลที่คาดหวัง)                                                                    | ผลการประเมิน | Issue # / PR # | หมายเหตุ / รูปภาพ                                         |
| :------------ | :------------------------------------------ | :----------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------- | :---------- | :------------- | :------------------------------------------------------ |
| **TC_SP9_01** | Nav — 7-item Layout                         | 1. เปิดหน้า Dashboard<br>2. ตรวจสอบ Navigation bar                                                                   | Nav bar แสดง 7 items: Overview · Sources · Knowledge · Quality · Playground · Coverage · Admin | ✅ Pass      | #98 / PR #107  | Screenshot confirmed 7 items with correct icons         |
| **TC_SP9_02** | Nav — Knowledge Placeholder                 | 1. คลิก "Knowledge" บน Nav bar                                                                                      | แสดงหน้า placeholder "Knowledge Base" พร้อมข้อความ "Coming in Sprint 10"                          | ✅ Pass      | #98 / PR #107  | Screenshot: BookOpen icon + badge                       |
| **TC_SP9_03** | Nav — Coverage Placeholder                  | 1. คลิก "Coverage" บน Nav bar                                                                                       | แสดงหน้า placeholder "Coverage Analytics" พร้อมข้อความ "Coming in Sprint 12"                      | ✅ Pass      | #98 / PR #107  | Browser subagent confirmed                              |
| **TC_SP9_04** | Nav — Existing Pages Accessible             | 1. คลิก Sources, Quality, Playground, Admin ทีละอัน                                                                   | แต่ละปุ่มนำไปหน้าที่ถูกต้อง — Sources, Quality Control, Playground, Settings                            | ✅ Pass      | #98 / PR #107  | Browser subagent navigated all 4 pages                  |
| **TC_SP9_05** | Settings — Tabbed Layout                    | 1. ไปหน้า Admin (Settings)<br>2. ตรวจสอบ Tab sidebar ด้านซ้าย                                                         | แสดง 6 tabs: General, AI Models, Pipeline, Knowledge Graph, Search, Security                   | ✅ Pass      | #99 / PR #108  | Screenshot: 6 tabs in sidebar confirmed                 |
| **TC_SP9_06** | Settings — General Tab                      | 1. คลิก Tab "General"<br>2. ตรวจสอบ Tenant Name + Tenant ID fields                                                  | แสดง Tenant Configuration card พร้อม Name (editable) + ID (readonly)                            | ✅ Pass      | #99 / PR #108  | Screenshot: "Default Tenant" + readonly ID              |
| **TC_SP9_07** | Settings — AI Models Tab                    | 1. คลิก Tab "AI Models"<br>2. ตรวจสอบ Provider, Model, System Prompt, Max Tokens                                    | แสดง AI Model Configuration card พร้อม fields ทั้งหมด + Save button                               | ✅ Pass      | #99 / PR #108  | Screenshot: All fields visible                          |
| **TC_SP9_08** | Settings — Pipeline Tab                     | 1. คลิก Tab "Pipeline"<br>2. ตรวจสอบ Chunking Strategy, Chunk Size, Overlap, Dedup Threshold                        | แสดง Pipeline Settings card พร้อม dropdowns + inputs                                            | ✅ Pass      | #99 / PR #108  | Screenshot: Auto strategy, 512/50 defaults, SHA-256     |
| **TC_SP9_09** | Settings — Coming Soon Tabs                 | 1. คลิก Tab "Knowledge Graph", "Search", "Security" ทีละอัน                                                           | แต่ละ Tab แสดง Coming Soon placeholder พร้อมระบุ Sprint (11, 10, 14 ตามลำดับ)                       | ✅ Pass      | #99 / PR #108  | Screenshot: Security tab shows "Coming in Sprint 14"    |
| **TC_SP9_10** | URL Preview Endpoint                        | 1. `curl -H "Authorization: Bearer $TOKEN" "http://localhost:8080/api/v1/sources/preview?url=https://example.com"` | ได้ JSON response พร้อม url, domain, title, description, image, favicon                          | ✅ Pass      | #96 / PR #105  | Returns: title="Example Domain", domain="example.com"   |
| **TC_SP9_11** | Sync — Chunking Integration                 | 1. สร้าง web source (example.com)<br>2. Sync<br>3. ตรวจสอบ DB                                                       | `chunks` table มี records, `total_chunks` ใน `data_sources` ตรงกับจำนวน chunks, status=COMPLETED  | ✅ Pass      | #95 / PR #104  | 1 chunk (337 chars), total_chunks=1, COMPLETED          |
| **TC_SP9_12** | Sync — Link Discovery (Web Source)          | 1. สร้าง web source พร้อม URL<br>2. Sync<br>3. ตรวจสอบ `crawled_pages` table                                         | มี record status='crawled' สำหรับ main page                                                       | ✅ Pass      | #96 / PR #105  | crawled_pages: url=example.com, status=crawled          |
| **TC_SP9_13** | Sync — Cross-source Dedup                   | 1. Re-sync source เดิม (URL เดียวกัน)<br>2. ตรวจสอบ chunks + fingerprints count                                       | Duplicate chunks ถูก skip, ไม่มี chunk ใหม่เพิ่ม                                                     | ✅ Pass      | #97 / PR #106  | After re-sync: still 1 chunk, 1 fingerprint — dedup OK  |
| **TC_SP9_14** | Pipeline Wiring — Auto-extraction on Upload | 1. Upload ไฟล์ .md<br>2. ตรวจสอบ extraction + chunking                                                              | `raw_markdown` มีเนื้อหา, status=COMPLETED, chunks + fingerprints created                         | ✅ Pass      | #94 / PR #103  | 3 chunks, 3 fingerprints, 269 chars markdown, COMPLETED |

---

**สรุปผลการทดสอบ Sprint 9 (Sign-off):**
- [x] Unit Tests ผ่านทั้งหมด (10/10)
- [x] Frontend Nav Restructure ผ่าน (4/4 UI tests: TC_SP9_01~04)
- [x] Settings Tabs ผ่าน (5/5 UI tests: TC_SP9_05~09) — หลังจากแก้ไข admin password (#110)
- [x] Integration Tests ผ่าน (5/5 tests: TC_SP9_10~14)

**ผลการทดสอบ 2026-02-27:**
- **Unit Tests**: 10/10 ✅ (116 backend tests + 15-route frontend build)
- **UI Tests**: 9/9 ✅ (TC_SP9_01~09)
- **Integration Tests**: 5/5 ✅ (TC_SP9_10~14)
- **Total**: **24/24 ผ่านทั้งหมด (100%)**

**Bugs ที่พบและแก้ไขระหว่างการทดสอบ:**
- Issue #109: Compile errors (`Deserialize` import + `crate::config` path) — แก้ไขแล้ว ✅
- Issue #110: Admin password hash ไม่ตรง — แก้ไขด้วย migration `20260228100000_fix_admin_password.sql` ✅

**อ้างอิง (GitHub References):**
- **Issues:** #94, #95, #96, #97, #98, #99, #100, #109, #110, #111
- **Pull Requests:** PR #101, PR #103, PR #104, PR #105, PR #106, PR #107, PR #108
