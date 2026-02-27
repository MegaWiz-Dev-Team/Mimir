# PM-02.8: Sprint 8 Status Report (Unified Data Ingress & File Upload)

**Project Name:** Project Mimir
**Sprint:** Sprint 8
**Status:** Completed
**Date:** 2026-02-27

---

## 1. ขอบเขตของ Sprint 8 (Sprint Scope)
- **Backend:** File Upload API พร้อม validation (extension, size, SHA-256 duplicate detection)
- **Backend:** Extraction Worker สำหรับ PDF, CSV, XLSX, HTML, MCP → Markdown
- **Backend:** SQL Import Module สำหรับ auto-detect column types และ generate DDL/INSERT
- **Backend:** Domain Connector Architecture แยก business logic ตาม tenant domain (medical/game/general)
- **Backend:** Feature Flags API (`GET /tenants/{id}/features`) สำหรับ domain-aware configuration
- **Backend:** สร้าง `ro-ai-domain-medical` crate ใหม่
- **Frontend:** Data Connection Wizard พร้อม Drag & Drop Upload, Folder Upload, Upload Progress
- **Frontend:** IngressTypeSelector, StorageModeSelector, AdvancedSettings components
- **Database:** Migration เพิ่ม `domain` column ใน `tenants` table

## 2. สรุปผลการทดสอบ (Testing Verification Summary)
อ้างอิงจากแผนการทดสอบ `SI_04_8_Sprint8_TestScript.md`:

### Unit Tests (7/7 Pass)
| ID        | Description                         | Result |
| --------- | ----------------------------------- | ------ |
| TC_SP8_U1 | File Validation (8 tests)           | ✅ Pass |
| TC_SP8_U2 | S3 Key & Hashing (8 tests)          | ✅ Pass |
| TC_SP8_U3 | Extraction Logic (15 tests)         | ✅ Pass |
| TC_SP8_U4 | SQL Table Generation (26 tests)     | ✅ Pass |
| TC_SP8_U5 | Domain Connector Routing (10 tests) | ✅ Pass |
| TC_SP8_U6 | Upload Components (5 tests)         | ✅ Pass |
| TC_SP8_U7 | Wizard UI Components (6 tests)      | ✅ Pass |

### UI/API Tests (8/9 Pass, 1 Partial)
| ID        | Description                         | Result    |
| --------- | ----------------------------------- | --------- |
| TC_SP8_01 | File Upload — PDF                   | ✅ Pass    |
| TC_SP8_02 | File Upload — Excel (Markdown Mode) | ✅ Pass    |
| TC_SP8_03 | File Upload — CSV (SQL Table Mode)  | ✅ Pass    |
| TC_SP8_04 | Folder Upload                       | ✅ Pass    |
| TC_SP8_05 | File Size Limit Rejection           | ✅ Pass    |
| TC_SP8_06 | Invalid File Extension Rejection    | ✅ Pass    |
| TC_SP8_07 | Duplicate File Detection            | ✅ Pass    |
| TC_SP8_08 | Worker Error Handling               | ⚠️ Partial |
| TC_SP8_09 | Web URL Fetch via Unified Pipeline  | ✅ Pass    |

**Total: 15/16 (93.75%)**

## 3. GitHub Synchronization & Traceability
### Issues
| Issue # | Title                                                           | Status            |
| ------- | --------------------------------------------------------------- | ----------------- |
| #73     | File/Folder Upload API                                          | ✅ Closed          |
| #74     | Extraction Worker                                               | ✅ Closed          |
| #75     | SQL Import Module                                               | ✅ Closed          |
| #76     | Domain Connector Architecture                                   | ✅ Closed          |
| #77     | Frontend Upload Components                                      | ✅ Closed          |
| #84     | Bug: avatar_url missing in ro-ai-domain-game                    | ✅ Closed          |
| #86     | Enhancement: Connect IngressManager to real extraction pipeline | 🔓 Open (Sprint 9) |

### Pull Requests
| PR #   | Title                               | Status   |
| ------ | ----------------------------------- | -------- |
| PR #78 | Frontend Upload & Wizard Components | ✅ Merged |
| PR #80 | File/Folder Upload API              | ✅ Merged |
| PR #81 | Extraction Worker                   | ✅ Merged |
| PR #82 | SQL Import Module                   | ✅ Merged |
| PR #83 | Domain Connector Architecture       | ✅ Merged |
| PR #85 | Bug fix: avatar_url                 | ✅ Merged |

## 4. ปัญหาที่พบและวิธีแก้ไข (Issues & Resolutions)
1. **Migration Duplicate Column:**
   - *ปัญหา:* `ALTER TABLE ADD COLUMN domain` fail เมื่อ migration ถูก apply manual แล้ว
   - *แก้ปัญหา:* ทำ migration ให้ idempotent ด้วย `IF NOT EXISTS` pattern

2. **avatar_url Missing in Test Files:**
   - *ปัญหา:* `Persona` struct ถูก update เพิ่ม `avatar_url` แต่ test helper ไม่ได้อัปเดต ทำให้ `ro-ai-domain-game` compile ไม่ผ่าน
   - *แก้ปัญหา:* เพิ่ม `avatar_url: None` ใน `create_test_persona()` (Issue #84, PR #85)

3. **Extraction Stub Limitation (TC_SP8_08):**
   - *ปัญหา:* `IngressManager::process_source()` ยังเป็น stub ไม่ได้เรียก extraction จริง ทำให้ corrupted PDF ไม่ถูกตรวจจับ
   - *แก้ปัญหา:* บันทึกเป็น known limitation, สร้าง Issue #86 สำหรับ Sprint ถัดไป

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
