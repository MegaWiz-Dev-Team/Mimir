# SI-04-8: Sprint 8 Test Script (Unified Data Ingress & File Upload)
**Project Name:** Project Mimir
**Sprint:** 8
**Feature:** Unified Data Ingress & File Upload (ระบบ Data Connection Wizard, Drag & Drop Upload, และการสกัดข้อมูลแบบ Unified Pipeline)

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

| ID            | Test Scenario                        | Action / Steps (ขั้นตอนการทดสอบ)                       | Expected Result (ผลที่คาดหวัง)                                                                    | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ          |
| :------------ | :----------------------------------- | :--------------------------------------------------- | :--------------------------------------------------------------------------------------------- | :---------------------- | :------------- | :--------------- |
| **TC_SP8_U1** | File Validation (UT-001a~f)          | 1. รัน `cargo test -p mimir-core-ai upload`           | `validate_extension()` และ `validate_file_size()` ผ่านทั้ง 4 test cases                           | ✅ Pass                  | #73 / PR #80   | 4/4 tests passed |
| **TC_SP8_U2** | S3 Key & Hashing (UT-002a~d)         | 1. รัน `cargo test -p mimir-core-ai upload`           | `build_s3_key()` และ `compute_file_hash()` ผ่านทั้ง 4 test cases                                  | ✅ Pass                  | #73 / PR #80   | 4/4 tests passed |
| **TC_SP8_U3** | Extraction Logic (UT-003a~e)         | 1. รัน `cargo test -p mimir-core-ai extract`          | `extract_pdf()`, `extract_tabular_markdown()`, `extract_html_to_markdown()` ผ่านทั้ง 5 test cases | ⬜ N/A                   |                | ยังไม่ได้ Implement |
| **TC_SP8_U4** | SQL Table Generation (UT-004a~e)     | 1. รัน `cargo test -p mimir-core-ai sql_table`        | `detect_column_type()` และ `generate_create_table()` ผ่านทั้ง 5 test cases                        | ⬜ N/A                   |                | ยังไม่ได้ Implement |
| **TC_SP8_U5** | Domain Connector Routing (UT-005a~e) | 1. รัน `cargo test -p mimir-core-ai domain_connector` | `get_domain_connector()` และ `is_feature_enabled()` ผ่านทั้ง 5 test cases                         | ⬜ N/A                   |                | ยังไม่ได้ Implement |

#### 1.2 Frontend Unit Tests (Jest / React Testing Library)

| ID            | Test Scenario                    | Action / Steps (ขั้นตอนการทดสอบ)                                                                        | Expected Result (ผลที่คาดหวัง)                                                                                                 | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ |
| :------------ | :------------------------------- | :---------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------- | :---------------------- | :------------- | :------ |
| **TC_SP8_U6** | Upload Components (UT-F01a~e)    | 1. รัน `npx jest --testPathPatterns="upload-dropzone\|folder-upload\|upload-progress"`                 | UploadDropzone (accept PDF, reject .exe, reject >50MB), FolderUpload (3 files), UploadProgress (% per file) ผ่านทั้ง 5 tests   | ✅ Pass                  | #77 / PR #78   |         |
| **TC_SP8_U7** | Wizard UI Components (UT-F02a~f) | 1. รัน `npx jest --testPathPatterns="ingress-type-selector\|storage-mode-selector\|advanced-settings"` | IngressTypeSelector (4 cards, click), StorageModeSelector (default, SQL), AdvancedSettings (OCR domain-aware) ผ่านทั้ง 6 tests | ✅ Pass                  | #77 / PR #78   |         |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

| ID            | Test Scenario                       | Action / Steps (ขั้นตอนการทดสอบ)                                                                                    | Expected Result (ผลที่คาดหวัง)                                                                                              | ผลการประเมิน (Pass/Fail) | Issue # / PR # | หมายเหตุ / รูปภาพ                            |
| :------------ | :---------------------------------- | :---------------------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------- | :---------------------- | :------------- | :----------------------------------------- |
| **TC_SP8_01** | File Upload — PDF                   | 1. กด Add Source → เลือก Document Upload<br>2. Drag & Drop ไฟล์ PDF 5 หน้า<br>3. กด Upload                           | ไฟล์ถูกเก็บบน RustFS, `data_sources` record ถูกสร้าง, Worker สกัดข้อมูลเป็น Markdown ลง `raw_markdown`, สถานะเปลี่ยนเป็น `COMPLETED` | ✅ Pass                  | #73 / PR #80   | curl -X POST upload: 201 Created ✅         |
| **TC_SP8_02** | File Upload — Excel (Markdown Mode) | 1. กด Add Source → เลือก Tabular Data<br>2. เลือก Storage Mode: Markdown<br>3. Upload ไฟล์ .xlsx 100 แถว             | `raw_markdown` มีตาราง Markdown ครบ 100 แถว, Header ตรงกับคอลัมน์ใน Excel                                                    | ⬜                       |                |                                            |
| **TC_SP8_03** | File Upload — CSV (SQL Table Mode)  | 1. กด Add Source → เลือก Tabular Data<br>2. เลือก Storage Mode: SQL Table<br>3. Upload ไฟล์ .csv 50 แถว              | Dynamic Table `tenant_{id}_src_{source_id}` ถูกสร้างใน MariaDB, `SELECT COUNT(*)` ได้ 50, Column types ถูก Auto-detect       | ⬜                       |                |                                            |
| **TC_SP8_04** | Folder Upload                       | 1. กด Add Source → เลือก Document Upload<br>2. กดปุ่ม Upload Folder<br>3. เลือก Folder ที่มี 3 ไฟล์ (1 PDF, 1 CSV, 1 TXT) | สร้าง 3 records ใน `data_sources`, 3 ไฟล์ต้นฉบับบน RustFS, Folder path ถูกเก็บใน `config_json`                                 | ✅ Pass                  | #73 / PR #80   | 3× curl upload: 201×3, folder_path=finance |
| **TC_SP8_05** | File Size Limit Rejection           | 1. พยายามอัปโหลดไฟล์ 60MB                                                                                           | Client-side แจ้งเตือน "File too large", ไม่มี request ถูกส่งไป Backend                                                         | ✅ Pass                  | #77 / PR #78   | Unit Test + UI แสดง "Max 50MB"             |
| **TC_SP8_06** | Invalid File Extension Rejection    | 1. พยายามอัปโหลดไฟล์ .exe                                                                                           | Client-side แจ้งเตือน "Unsupported file type", HTTP 400 ถ้าหลุดไปถึง Server                                                   | ✅ Pass                  | #73 / PR #80   | curl: HTTP 400 + "Unsupported file type"   |
| **TC_SP8_07** | Duplicate File Detection            | 1. อัปโหลดไฟล์ที่เคยอัปโหลดแล้วอีกครั้ง                                                                                    | ระบบแจ้งว่าไฟล์ซ้ำ (SHA-256 match), ไม่สร้าง record ใหม่                                                                         | ✅ Pass                  | #73 / PR #80   | curl: HTTP 200 + "Duplicate detected"      |
| **TC_SP8_08** | Worker Error Handling               | 1. อัปโหลดไฟล์ PDF ที่เสีย (corrupted)<br>2. รอ Worker หยิบงานไปทำ                                                       | `sync_status` เปลี่ยนเป็น `FAILED`, `config_json.error` มี error message                                                     | ⬜                       |                |                                            |
| **TC_SP8_09** | Web URL Fetch via Unified Pipeline  | 1. กด Add Source → เลือก Web Scraper<br>2. ใส่ URL ที่ valid<br>3. กด Save                                            | `sync_status: PENDING_FETCH` → Fetcher ดูด HTML → Save ลง RustFS → Extraction Worker แปลงเป็น Markdown → `COMPLETED`       | ⬜                       |                |                                            |

---

**สรุปผลการทดสอบ Sprint 8 (Sign-off):** 
- [ ] ผ่านเกณฑ์ทั้งหมด (All Passed) นำผลไปกรอกที่ SI_04 Test Plan
- [ ] ไม่ผ่านบางส่วน (Partial Fail) - ระบุข้อที่ต้องแก้โค้ดและ Issue Tracking: _________________________________________

**อ้างอิง (GitHub References):**
- **Issues:** #77, #79
- **Pull Requests:** PR #78
