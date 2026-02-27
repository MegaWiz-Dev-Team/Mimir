# SI-04-11: Sprint 11 Test Script (LLM Fallback & File Improvements)
**Project Name:** Project Mimir
**Sprint:** 11
**Feature:** LLM Fallback Extraction, File Upload Fix, CSV Sync Fix, Ingress Console Logs, Legacy Office Formats
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

#### 1.1 Backend Extraction Unit Tests (16 tests)

| ID              | Test Scenario                     | Action / Steps (ขั้นตอนการทดสอบ)                                                 | Expected Result (ผลที่คาดหวัง)                            | ผลการประเมิน | Issue # / PR # | หมายเหตุ                        |
| :-------------- | :-------------------------------- | :----------------------------------------------------------------------------- | :----------------------------------------------------- | :---------- | :------------- | :----------------------------- |
| **TC_SP11_U1**  | extract_pdf — valid PDF           | 1. รัน `cargo test -p mimir-core-ai -- test_extract_pdf_valid`                  | Test passes, non-empty text extracted                  | ✅ Pass      | #124, PR #130  | PDF extraction via pdf-extract |
| **TC_SP11_U2**  | extract_pdf — corrupted PDF       | 1. รัน `cargo test -p mimir-core-ai -- test_extract_pdf_corrupted`              | Test passes, returns Err for corrupted data            | ✅ Pass      | #124, PR #130  | Error handling verified        |
| **TC_SP11_U3**  | extract_csv — Markdown table      | 1. รัน `cargo test -p mimir-core-ai -- test_extract_csv_to_markdown`            | Test passes, CSV → pipe-separated Markdown table       | ✅ Pass      | #124, PR #130  | Correct table formatting       |
| **TC_SP11_U4**  | extract_xlsx — Markdown table     | 1. รัน `cargo test -p mimir-core-ai -- test_extract_xlsx_to_markdown`           | Test passes, XLSX → Markdown table via calamine        | ✅ Pass      | #124, PR #130  | calamine crate for .xls/.xlsx  |
| **TC_SP11_U5**  | extract_html — Markdown           | 1. รัน `cargo test -p mimir-core-ai -- test_extract_html_to_markdown`           | Test passes, HTML → Markdown via html2md               | ✅ Pass      | #124, PR #130  | html2md crate                  |
| **TC_SP11_U6**  | extract_text — passthrough        | 1. รัน `cargo test -p mimir-core-ai -- test_extract_text`                       | Test passes, UTF-8 text returned as-is                 | ✅ Pass      | #124, PR #130  | Plain text passthrough         |
| **TC_SP11_U7**  | extract_mcp_json — formatted MD   | 1. รัน `cargo test -p mimir-core-ai -- test_extract_mcp_json_to_markdown`       | Test passes, JSON → MCP Data heading + code block      | ✅ Pass      | #124, PR #130  | MCP JSON formatting            |
| **TC_SP11_U8**  | extract router — CSV              | 1. รัน `cargo test -p mimir-core-ai -- test_extract_router_csv`                 | Test passes, "tabular" + .csv → correct dispatch       | ✅ Pass      | #122, PR #130  | Router dispatches correctly    |
| **TC_SP11_U9**  | extract router — HTML             | 1. รัน `cargo test -p mimir-core-ai -- test_extract_router_html`                | Test passes, "web" + .html → correct dispatch          | ✅ Pass      | #122, PR #130  | Web extraction dispatch        |
| **TC_SP11_U10** | extract router — Text             | 1. รัน `cargo test -p mimir-core-ai -- test_extract_router_text`                | Test passes, "document" + .txt → correct dispatch      | ✅ Pass      | #122, PR #130  | Document extraction dispatch   |
| **TC_SP11_U11** | extract router — unsupported type | 1. รัน `cargo test -p mimir-core-ai -- test_extract_router_unsupported_type`    | Test passes, returns "Unsupported source_type" error   | ✅ Pass      | #122, PR #130  | Error handling                 |
| **TC_SP11_U12** | extract router — unsupported ext  | 1. รัน `cargo test -p mimir-core-ai -- test_extract_router_unsupported_ext`     | Test passes, returns "Unsupported document extension"  | ✅ Pass      | #122, PR #130  | Error handling                 |
| **TC_SP11_U13** | ingress — CSV extraction          | 1. รัน `cargo test -p mimir-core-ai -- test_process_extraction_csv`             | Test passes, CSV processed through extraction pipeline | ✅ Pass      | #122, PR #130  | Ingress integration            |
| **TC_SP11_U14** | ingress — CSV SQL mode            | 1. รัน `cargo test -p mimir-core-ai -- test_process_extraction_csv_sql_mode`    | Test passes, SQL mode CSV extraction                   | ✅ Pass      | #122, PR #130  | Dual-mode SQL import           |
| **TC_SP11_U15** | ingress — SQL DDL generation      | 1. รัน `cargo test -p mimir-core-ai -- test_process_extraction_sql_returns_ddl` | Test passes, DDL correctly generated                   | ✅ Pass      | #122, PR #130  | Dynamic table generation       |
| **TC_SP11_U16** | link discovery — favicon          | 1. รัน `cargo test -p mimir-core-ai -- test_link_discovery_favicon`             | Test passes, favicon URL extracted                     | ✅ Pass      | #122, PR #130  | Link discovery utility         |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

#### 2.1 File Upload Wizard Fix (#121)

| ID             | Test Scenario             | Action / Steps (ขั้นตอนการทดสอบ)                                              | Expected Result (ผลที่คาดหวัง)       | ผลการประเมิน | Issue # / PR # | หมายเหตุ                         |
| :------------- | :------------------------ | :-------------------------------------------------------------------------- | :-------------------------------- | :---------- | :------------- | :------------------------------ |
| **TC_SP11_01** | File list — remove button | 1. เปิด Add Source → File Upload<br>2. เพิ่มไฟล์หลายไฟล์<br>3. กดปุ่ม X ลบไฟล์ 1 ตัว | ไฟล์ถูกลบออกจากลิสต์, ไฟล์ที่เหลือยังอยู่ครบ | ✅ Pass      | #121, PR #130  | File list with X remove buttons |
| **TC_SP11_02** | Upload buttons            | 1. ตรวจสอบปุ่ม Upload Files และ Upload Folder                                 | ทั้งสองปุ่มแสดงและทำงานได้              | ✅ Pass      | #121, PR #130  | Separate file vs folder upload  |

#### 2.2 CSV Sync Fix (#122)

| ID             | Test Scenario                       | Action / Steps (ขั้นตอนการทดสอบ)                           | Expected Result (ผลที่คาดหวัง)                                      | ผลการประเมิน | Issue # / PR # | หมายเหตุ                               |
| :------------- | :---------------------------------- | :------------------------------------------------------- | :--------------------------------------------------------------- | :---------- | :------------- | :------------------------------------ |
| **TC_SP11_03** | Sync "file" source type             | 1. สร้าง source type "file"<br>2. อัปโหลดไฟล์<br>3. กด Sync | Sync สำเร็จ, ไม่แสดง "Unsupported source type: file"                | ✅ Pass      | #122, PR #130  | Added "file" arm in sync_source       |
| **TC_SP11_04** | File source — S3 download + extract | 1. ตรวจสอบ sync flow สำหรับ source_type="file"             | ดาวน์โหลดจาก S3, auto-detect extension, route ไป extractor ที่ถูกต้อง | ✅ Pass      | #122, PR #130  | S3 download + extension-based routing |

#### 2.3 Ingress Console Real Logs (#123)

| ID             | Test Scenario                   | Action / Steps (ขั้นตอนการทดสอบ)                           | Expected Result (ผลที่คาดหวัง)                                            | ผลการประเมิน | Issue # / PR # | หมายเหตุ                           |
| :------------- | :------------------------------ | :------------------------------------------------------- | :--------------------------------------------------------------------- | :---------- | :------------- | :-------------------------------- |
| **TC_SP11_05** | Console — real polling logs     | 1. Trigger sync บน source<br>2. ตรวจสอบ Ingress Console  | แสดง real-time polling messages แทน hardcoded fake logs                | ✅ Pass      | #123, PR #130  | Replaced fake sequential messages |
| **TC_SP11_06** | Console — source-type-aware msg | 1. Sync Web source vs File source<br>2. ตรวจสอบ messages | ข้อความต่างกันตาม source type (Web: "Crawling...", File: "Processing...") | ✅ Pass      | #123, PR #130  | Type-specific status messages     |
| **TC_SP11_07** | Console — completion data       | 1. รอ sync สำเร็จ<br>2. ตรวจสอบ completion message         | แสดงจำนวน chunks, MB size, completion status จาก API                    | ✅ Pass      | #123, PR #130  | Real data from fetchSources()     |

#### 2.4 LLM Fallback Extraction (#125)

| ID             | Test Scenario                     | Action / Steps (ขั้นตอนการทดสอบ)                      | Expected Result (ผลที่คาดหวัง)                   | ผลการประเมิน | Issue # / PR # | หมายเหตุ                           |
| :------------- | :-------------------------------- | :-------------------------------------------------- | :-------------------------------------------- | :---------- | :------------- | :-------------------------------- |
| **TC_SP11_08** | AI Extract panel — model selector | 1. เปิด source detail<br>2. ตรวจสอบ AI Extract panel | แสดง Model selector dropdown พร้อมรายชื่อ models | ✅ Pass      | #125, PR #130  | Multi-model LLM support           |
| **TC_SP11_09** | AI Extract — trigger extraction   | 1. เลือก model<br>2. กด "Extract with AI"            | แสดง loading state → ผลลัพธ์ extracted markdown | ✅ Pass      | #125, PR #130  | POST /sources/:id/extract-ai      |
| **TC_SP11_10** | AI Extract — token usage          | 1. ตรวจสอบ response จาก extract-ai API              | แสดง tokens_used ในผลลัพธ์                      | ✅ Pass      | #125, PR #130  | Token tracking for cost awareness |

#### 2.5 Legacy Office Formats (#124)

| ID             | Test Scenario                    | Action / Steps (ขั้นตอนการทดสอบ)                     | Expected Result (ผลที่คาดหวัง)                                            | ผลการประเมิน | Issue # / PR # | หมายเหตุ                              |
| :------------- | :------------------------------- | :------------------------------------------------- | :--------------------------------------------------------------------- | :---------- | :------------- | :----------------------------------- |
| **TC_SP11_11** | Dropzone accepts legacy formats  | 1. เปิด File Upload<br>2. ตรวจสอบ accepted formats  | Dropzone แสดง PDF, DOCX, DOC, XLSX, XLS, PPTX, PPT, CSV, TXT, JSON, MD | ✅ Pass      | #124, PR #130  | Frontend already supports extensions |
| **TC_SP11_12** | Backend routes legacy extensions | 1. ตรวจสอบ extraction router สำหรับ .doc, .xls, .ppt | .doc/.ppt → extract_legacy_office, .xls → extract_xlsx_to_markdown     | ✅ Pass      | #124, PR #130  | Routing wired in extract() function  |
| **TC_SP11_13** | LibreOffice conversion           | 1. ตรวจสอบ extract_legacy_office function          | .doc→.docx via soffice, .xls→.xlsx, .ppt→.pptx — then delegate         | ✅ Pass      | #124, PR #130  | LibreOffice headless conversion      |

---

**สรุปผลการทดสอบ Sprint 11 (Sign-off):**
- [x] Backend Unit Tests ผ่าน (16/16: TC_SP11_U1~U16)
- [x] File Upload Fix ผ่าน (2/2: TC_SP11_01~02)
- [x] CSV Sync Fix ผ่าน (2/2: TC_SP11_03~04)
- [x] Ingress Console ผ่าน (3/3: TC_SP11_05~07)
- [x] LLM Fallback ผ่าน (3/3: TC_SP11_08~10)
- [x] Legacy Office ผ่าน (3/3: TC_SP11_11~13)

**ผลการทดสอบ 2026-02-27:**
- **Unit Tests**: 16/16 ✅ (TC_SP11_U1~U16)
- **UI/Feature Tests**: 13/13 ✅ (TC_SP11_01~13)
- **Total**: **29/29 ผ่านทั้งหมด (100%)**

**อ้างอิง (GitHub References):**
- **Issues:** #121, #122, #123, #124, #125
- **Pull Requests:** PR #130
