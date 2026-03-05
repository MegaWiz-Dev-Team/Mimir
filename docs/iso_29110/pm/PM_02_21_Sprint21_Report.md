# PM-02.21: Sprint 21 Status Report (Selective Chunk → QA Generation)

**Project Name:** Project Mimir
**Sprint:** Sprint 21
**Status:** ✅ Completed
**Date:** 2026-03-05

---

## 1. ขอบเขตของ Sprint 21 (Sprint Scope)
- **Frontend (UI):** เพิ่ม QA status column ในตาราง Knowledge Base — แสดง badge —/⏳/✅/❌ จาก `metadata_json.qa_status`
- **Frontend (Component):** สร้าง `QaStatusBadge` component พร้อม `getQaStatus()` helper
- **Frontend (Polling):** เพิ่ม auto-refresh polling ทุก 5 วินาทีหลัง Generate QA — หยุดอัตโนมัติเมื่อไม่มี chunk ที่กำลังประมวลผล
- **Scope:** Issue #179 (Selective Chunk → QA Generation)
- **Note:** Backend endpoint `POST /chunks/generate-qa` + checkbox selection + floating action bar ถูก implement ไว้ก่อนหน้าแล้ว — sprint นี้เพิ่มเฉพาะ QA status visibility

## 2. สรุปผลการทดสอบ (Testing Summary)

| Category                       | Total  | Pass   |
| ------------------------------ | ------ | ------ |
| Frontend Build                 | 1      | 1      |
| QA Status Column               | 5      | 5      |
| Regression (existing features) | 4      | 4      |
| Auto-Refresh Polling           | 2      | 2      |
| **Total**                      | **12** | **12** |

## 3. GitHub Synchronization
| Issue/PR | Title                                       | Status    |
| -------- | ------------------------------------------- | --------- |
| #179     | Selective Chunk → QA Generation             | Completed |
| PR       | feat(#179): QA Status Column + Auto-Refresh | Open      |

## 4. ไฟล์ที่แก้ไข (Files Changed)
| File                                         | Change Type | Description                                                     |
| -------------------------------------------- | ----------- | --------------------------------------------------------------- |
| `ro-ai-dashboard/src/app/knowledge/page.tsx` | Modified    | +QaStatusBadge, +getQaStatus, +QA column, +auto-refresh polling |

## 5. Technical Decisions
- **Frontend-only:** ไม่ต้องเปลี่ยน backend เพราะ `metadata_json.qa_status` ถูก set ไว้แล้ว
- **Polling strategy:** ใช้ `setInterval` 5s + auto-stop เมื่อไม่มี "processing" chunks — ประหยัด bandwidth
- **Badge design:** ใช้ color-coded rounded pills (amber/green/red) ตาม design system เดิม
