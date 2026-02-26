# SI-04-8: Sprint 8 Test Script — Unified Data Ingress & File Upload
**Project Name:** Project Mimir  
**Version:** 2.4  
**Sprint:** 8  
**วันที่สร้าง:** 2026-02-26  

## Test Cases

### TC-008-01: File Upload — PDF
| รายการ           | รายละเอียด                                                                                                                |
| :--------------- | :----------------------------------------------------------------------------------------------------------------------- |
| **Objective**    | ทดสอบการอัปโหลดไฟล์ PDF และสกัดข้อมูลเป็น Markdown                                                                             |
| **Precondition** | Login สำเร็จ, RustFS ทำงานปกติ                                                                                               |
| **Steps**        | 1. กด Add Source → เลือก Document Upload<br>2. Drag & Drop ไฟล์ PDF 5 หน้า<br>3. กด Upload                                  |
| **Expected**     | ไฟล์ถูกเก็บบน RustFS, `data_sources` record ถูกสร้าง, Worker สกัดข้อมูลเป็น Markdown ลง `raw_markdown`, สถานะเปลี่ยนเป็น `COMPLETED` |
| **Status**       | ⬜ Pending                                                                                                                |

### TC-008-02: File Upload — Excel (Markdown Mode)
| รายการ           | รายละเอียด                                                                                             |
| :--------------- | :---------------------------------------------------------------------------------------------------- |
| **Objective**    | ทดสอบการอัปโหลด .xlsx และแปลงเป็น Markdown Table                                                        |
| **Precondition** | Login สำเร็จ                                                                                            |
| **Steps**        | 1. กด Add Source → เลือก Tabular Data<br>2. เลือก Storage Mode: Markdown<br>3. Upload ไฟล์ .xlsx 100 แถว |
| **Expected**     | `raw_markdown` มีตาราง Markdown ครบ 100 แถว, Header ตรงกับคอลัมน์ใน Excel                                 |
| **Status**       | ⬜ Pending                                                                                             |

### TC-008-03: File Upload — CSV (SQL Table Mode)
| รายการ           | รายละเอียด                                                                                                          |
| :--------------- | :----------------------------------------------------------------------------------------------------------------- |
| **Objective**    | ทดสอบการอัปโหลด .csv แบบ SQL Table Mode                                                                             |
| **Precondition** | Login สำเร็จ                                                                                                         |
| **Steps**        | 1. กด Add Source → เลือก Tabular Data<br>2. เลือก Storage Mode: SQL Table<br>3. Upload ไฟล์ .csv 50 แถว               |
| **Expected**     | Dynamic Table `tenant_{id}_src_{source_id}` ถูกสร้างใน MariaDB, `SELECT COUNT(*)` ได้ 50, Column types ถูก Auto-detect |
| **Status**       | ⬜ Pending                                                                                                          |

### TC-008-04: Folder Upload
| รายการ           | รายละเอียด                                                                                                         |
| :--------------- | :---------------------------------------------------------------------------------------------------------------- |
| **Objective**    | ทดสอบการอัปโหลด Folder ที่มีหลายไฟล์                                                                                   |
| **Precondition** | Login สำเร็จ                                                                                                        |
| **Steps**        | 1. กด Add Source → เลือก Document Upload<br>2. กดปุ่ม Upload Folder<br>3. เลือก Folder ที่มี 3 ไฟล์ (1 PDF, 1 CSV, 1 TXT) |
| **Expected**     | สร้าง 3 records ใน `data_sources`, 3 ไฟล์ต้นฉบับบน RustFS, Folder path ถูกเก็บใน `config_json`                          |
| **Status**       | ⬜ Pending                                                                                                         |

### TC-008-05: File Size Limit Rejection
| รายการ           | รายละเอียด                                                        |
| :--------------- | :--------------------------------------------------------------- |
| **Objective**    | ทดสอบว่าไฟล์เกิน 50MB ถูกปฏิเสธ                                       |
| **Precondition** | Login สำเร็จ                                                       |
| **Steps**        | 1. พยายามอัปโหลดไฟล์ 60MB                                          |
| **Expected**     | Client-side แจ้งเตือน "File too large", ไม่มี request ถูกส่งไป Backend |
| **Status**       | ⬜ Pending                                                        |

### TC-008-06: Invalid File Extension Rejection
| รายการ           | รายละเอียด                                                              |
| :--------------- | :--------------------------------------------------------------------- |
| **Objective**    | ทดสอบว่านามสกุลที่ไม่รองรับถูกปฏิเสธ                                           |
| **Precondition** | Login สำเร็จ                                                             |
| **Steps**        | 1. พยายามอัปโหลดไฟล์ .exe                                                |
| **Expected**     | Client-side แจ้งเตือน "Unsupported file type", HTTP 400 ถ้าหลุดไปถึง Server |
| **Status**       | ⬜ Pending                                                              |

### TC-008-07: Duplicate File Detection
| รายการ           | รายละเอียด                               |
| :--------------- | :-------------------------------------- |
| **Objective**    | ทดสอบว่าไฟล์ซ้ำ (SHA-256 match) ไม่ถูกอัปโหลดซ้ำ |
| **Precondition** | มีไฟล์ถูกอัปโหลดแล้ว 1 ไฟล์                   |
| **Steps**        | 1. อัปโหลดไฟล์เดิมอีกครั้ง                    |
| **Expected**     | ระบบแจ้งว่าไฟล์ซ้ำ, ไม่สร้าง record ใหม่        |
| **Status**       | ⬜ Pending                               |

### TC-008-08: Worker Error Handling
| รายการ           | รายละเอียด                                                            |
| :--------------- | :------------------------------------------------------------------- |
| **Objective**    | ทดสอบว่า Worker จัดการไฟล์เสียได้ถูกต้อง                                    |
| **Precondition** | อัปโหลดไฟล์ PDF ที่เสีย (corrupted)                                       |
| **Steps**        | 1. รอ Worker หยิบงานไปทำ                                               |
| **Expected**     | `sync_status` เปลี่ยนเป็น `FAILED`, `config_json.error` มี error message |
| **Status**       | ⬜ Pending                                                            |

### TC-008-09: Web URL Fetch via Unified Pipeline
| รายการ           | รายละเอียด                                                                                                                   |
| :--------------- | :-------------------------------------------------------------------------------------------------------------------------- |
| **Objective**    | ทดสอบว่า Web URL ผ่าน Unified Pipeline ได้ถูกต้อง                                                                                |
| **Precondition** | Login สำเร็จ                                                                                                                  |
| **Steps**        | 1. กด Add Source → เลือก Web Scraper<br>2. ใส่ URL ที่ valid<br>3. กด Save                                                      |
| **Expected**     | `sync_status: PENDING_FETCH` → Fetcher ดูด HTML → Save Original ลง RustFS → Extraction Worker แปลงเป็น Markdown → `COMPLETED` |
| **Status**       | ⬜ Pending                                                                                                                   |

---

## TDD Unit Tests — Backend (Rust)
> **กระบวนการ TDD:** เขียน Test ก่อน (🔴 Red) → เขียนโค้ดให้ผ่าน (🟢 Green) → ปรับปรุง (🔵 Refactor)

### UT-001: File Validation

| Test ID | Function               | Input                           | Expected               | Status |
| :------ | :--------------------- | :------------------------------ | :--------------------- | :----- |
| UT-001a | `validate_extension()` | `"report.pdf"`                  | `Ok(())`               | ⬜      |
| UT-001b | `validate_extension()` | `"virus.exe"`                   | `Err(UnsupportedType)` | ⬜      |
| UT-001c | `validate_extension()` | `"scan.dcm"` + domain=`medical` | `Ok(())`               | ⬜      |
| UT-001d | `validate_extension()` | `"scan.dcm"` + domain=`game`    | `Err(UnsupportedType)` | ⬜      |
| UT-001e | `validate_file_size()` | 10MB                            | `Ok(())`               | ⬜      |
| UT-001f | `validate_file_size()` | 60MB                            | `Err(PayloadTooLarge)` | ⬜      |

### UT-002: S3 Key & Hashing

| Test ID | Function              | Input                                          | Expected               | Status |
| :------ | :-------------------- | :--------------------------------------------- | :--------------------- | :----- |
| UT-002a | `build_s3_key()`      | tenant=1, src=5, path="finance", file="q1.pdf" | `"1/5/finance/q1.pdf"` | ⬜      |
| UT-002b | `build_s3_key()`      | tenant=1, src=5, path="", file="report.csv"    | `"1/5/report.csv"`     | ⬜      |
| UT-002c | `compute_file_hash()` | ไฟล์เดียวกัน 2 ครั้ง                                | SHA-256 hash ตรงกัน     | ⬜      |
| UT-002d | `compute_file_hash()` | ไฟล์ต่างกัน                                       | SHA-256 hash ไม่ตรงกัน   | ⬜      |

### UT-003: Extraction Logic

| Test ID | Function                     | Input                       | Expected                        | Status |
| :------ | :--------------------------- | :-------------------------- | :------------------------------ | :----- |
| UT-003a | `extract_pdf()`              | PDF 3 หน้า (มีข้อความ)         | String ที่ไม่ว่าง, มีข้อความจากทุกหน้า  | ⬜      |
| UT-003b | `extract_pdf()`              | PDF เสีย (corrupted)         | `Err(ExtractionFailed)`         | ⬜      |
| UT-003c | `extract_tabular_markdown()` | CSV: `name,age\nAlice,30`   | `"                              | name   | age | \n | --- | --- | \n | Alice | 30 | "` | ⬜ |
| UT-003d | `extract_tabular_markdown()` | XLSX 100 แถว                | Markdown Table ที่มี 100 data rows | ⬜      |
| UT-003e | `extract_html_to_markdown()` | `<h1>Title</h1><p>Text</p>` | `"# Title\nText"`               | ⬜      |

### UT-004: SQL Table Generation (Dual-Mode)

| Test ID | Function                  | Input                                     | Expected                                                       | Status |
| :------ | :------------------------ | :---------------------------------------- | :------------------------------------------------------------- | :----- |
| UT-004a | `detect_column_type()`    | `["123", "456", "789"]`                   | `DECIMAL`                                                      | ⬜      |
| UT-004b | `detect_column_type()`    | `["hello", "world"]`                      | `VARCHAR(255)`                                                 | ⬜      |
| UT-004c | `detect_column_type()`    | `["2026-01-01", "2026-02-15"]`            | `DATE`                                                         | ⬜      |
| UT-004d | `detect_column_type()`    | `["123", "hello", "456"]` (ผสม)           | `VARCHAR(255)`                                                 | ⬜      |
| UT-004e | `generate_create_table()` | headers=`["name","age"]`, tenant=1, src=5 | `CREATE TABLE tenant_1_src_5 (name VARCHAR(255), age DECIMAL)` | ⬜      |

### UT-005: Domain Connector Routing

| Test ID | Function                 | Input                                 | Expected                    | Status |
| :------ | :----------------------- | :------------------------------------ | :-------------------------- | :----- |
| UT-005a | `get_domain_connector()` | domain=`"game"`                       | `GameConnector` instance    | ⬜      |
| UT-005b | `get_domain_connector()` | domain=`"medical"`                    | `MedicalConnector` instance | ⬜      |
| UT-005c | `get_domain_connector()` | domain=`"general"`                    | `DefaultConnector` instance | ⬜      |
| UT-005d | `is_feature_enabled()`   | domain=`"game"`, feature=`"dicom"`    | `false`                     | ⬜      |
| UT-005e | `is_feature_enabled()`   | domain=`"medical"`, feature=`"dicom"` | `true`                      | ⬜      |

---

## TDD Unit Tests — Frontend (Jest / React Testing Library)

### UT-F01: Upload Components

| Test ID | Component          | Scenario           | Expected                             | Status |
| :------ | :----------------- | :----------------- | :----------------------------------- | :----- |
| UT-F01a | `<UploadDropzone>` | Drop `.pdf` file   | File accepted, อยู่ใน file list        | ✅ Pass |
| UT-F01b | `<UploadDropzone>` | Drop `.exe` file   | File rejected, แสดง error message    | ✅ Pass |
| UT-F01c | `<UploadDropzone>` | Drop file > 50MB   | File rejected, แสดง "File too large" | ✅ Pass |
| UT-F01d | `<FolderUpload>`   | เลือก Folder 3 ไฟล์  | Recursive scan แสดง 3 ไฟล์ใน list     | ✅ Pass |
| UT-F01e | `<UploadProgress>` | Upload in progress | แสดง progress bar + % ต่อไฟล์          | ✅ Pass |

### UT-F02: Wizard UI Components

| Test ID | Component               | Scenario              | Expected                                   | Status |
| :------ | :---------------------- | :-------------------- | :----------------------------------------- | :----- |
| UT-F02a | `<IngressTypeSelector>` | Render                | แสดง 4 cards (Web, Document, Tabular, MCP) | ✅ Pass |
| UT-F02b | `<IngressTypeSelector>` | คลิก "Document Upload" | เปลี่ยนไป Step 2 พร้อม Dropzone               | ✅ Pass |
| UT-F02c | `<StorageModeSelector>` | Render                | แสดง Radio: Markdown (default) + SQL       | ✅ Pass |
| UT-F02d | `<StorageModeSelector>` | เลือก SQL Table        | State เปลี่ยนเป็น `"sql"`                     | ✅ Pass |
| UT-F02e | `<AdvancedSettings>`    | domain=`"medical"`    | แสดง OCR option                            | ✅ Pass |
| UT-F02f | `<AdvancedSettings>`    | domain=`"game"`       | ซ่อน OCR option                             | ✅ Pass |

---

## Execution Summary

| Test Case                      | Description              | Type | Result | หมายเหตุ |
| :----------------------------- | :----------------------- | :--- | :----- | :------ |
| **Integration / E2E**          |                          |      |        |         |
| TC-008-01                      | PDF Upload               | E2E  | ⬜      |         |
| TC-008-02                      | Excel Markdown Mode      | E2E  | ⬜      |         |
| TC-008-03                      | CSV SQL Table Mode       | E2E  | ⬜      |         |
| TC-008-04                      | Folder Upload            | E2E  | ⬜      |         |
| TC-008-05                      | File Size Limit          | E2E  | ⬜      |         |
| TC-008-06                      | Invalid Extension        | E2E  | ⬜      |         |
| TC-008-07                      | Duplicate Detection      | E2E  | ⬜      |         |
| TC-008-08                      | Worker Error Handling    | E2E  | ⬜      |         |
| TC-008-09                      | Web URL Unified Pipeline | E2E  | ⬜      |         |
| **Backend Unit Tests (Rust)**  |                          |      |        |         |
| UT-001a~f                      | File Validation          | Unit | ⬜      |         |
| UT-002a~d                      | S3 Key & Hashing         | Unit | ⬜      |         |
| UT-003a~e                      | Extraction Logic         | Unit | ⬜      |         |
| UT-004a~e                      | SQL Table Generation     | Unit | ⬜      |         |
| UT-005a~e                      | Domain Connector Routing | Unit | ⬜      |         |
| **Frontend Unit Tests (Jest)** |                          |      |        |         |
| UT-F01a~e                      | Upload Components        | Unit | ✅ Pass | PR #TBD |
| UT-F02a~f                      | Wizard UI Components     | Unit | ✅ Pass | PR #TBD |
