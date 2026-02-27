# PM-02.11: Sprint 11 Status Report (LLM Fallback & File Improvements)

**Project Name:** Project Mimir
**Sprint:** Sprint 11
**Status:** ✅ Completed
**Date:** 2026-02-27

---

## 1. ขอบเขตของ Sprint 11 (Sprint Scope)
- **Backend:** LLM Fallback Extraction — AI-powered document reading via OpenAI-compatible API with model selection
- **Backend:** CSV Sync Fix — handle `file` source_type in sync route with S3 download (#122)
- **Backend:** Legacy Office formats — `.doc`, `.xls`, `.ppt` extraction via LibreOffice headless (#124)
- **Frontend:** File Upload Wizard — file list with remove buttons (#121)
- **Frontend:** Ingress Console — source-type-aware polling logs with chunk counts, MB size, error messages (#123)
- **Frontend:** LLM Fallback UI — model selector dropdown and AI extraction trigger (#125)

## 2. สรุปผลการทดสอบ (Testing Verification Summary)

### Backend Tests (16/16 Pass)
| ID          | Description                                  | Result |
| ----------- | -------------------------------------------- | ------ |
| TC_SP11_U1  | extract_legacy_office — .doc via LibreOffice | ✅ Pass |
| TC_SP11_U2  | extract router — CSV dispatch                | ✅ Pass |
| TC_SP11_U3  | extract router — HTML dispatch               | ✅ Pass |
| TC_SP11_U4  | extract router — text dispatch               | ✅ Pass |
| TC_SP11_U5  | extract router — unsupported type            | ✅ Pass |
| TC_SP11_U6  | extract router — unsupported extension       | ✅ Pass |
| TC_SP11_U7  | extract_pdf — valid PDF                      | ✅ Pass |
| TC_SP11_U8  | extract_pdf — corrupted PDF → Err            | ✅ Pass |
| TC_SP11_U9  | extract_csv_to_markdown                      | ✅ Pass |
| TC_SP11_U10 | extract_xlsx_to_markdown                     | ✅ Pass |
| TC_SP11_U11 | extract_html_to_markdown                     | ✅ Pass |
| TC_SP11_U12 | extract_text — passthrough                   | ✅ Pass |
| TC_SP11_U13 | extract_mcp_json_to_markdown                 | ✅ Pass |
| TC_SP11_U14 | ingress process_extraction CSV               | ✅ Pass |
| TC_SP11_U15 | ingress process_extraction CSV SQL mode      | ✅ Pass |
| TC_SP11_U16 | ingress process_extraction SQL DDL           | ✅ Pass |

### Feature Tests (5/5 Pass)
| ID         | Description                                | Result |
| ---------- | ------------------------------------------ | ------ |
| TC_SP11_F1 | File upload wizard — remove file from list | ✅ Pass |
| TC_SP11_F2 | CSV sync — "file" source_type supported    | ✅ Pass |
| TC_SP11_F3 | Ingress Console — real polling logs        | ✅ Pass |
| TC_SP11_F4 | LLM Fallback — AI extraction with model    | ✅ Pass |
| TC_SP11_F5 | Legacy Office — .doc/.xls/.ppt accepted    | ✅ Pass |

**Total: 21/21 (100%)**

## 3. GitHub Synchronization & Traceability
### Issues
| Issue # | Title                                                        | Status   |
| ------- | ------------------------------------------------------------ | -------- |
| #121    | Feat: File upload wizard — cannot remove selected files      | ✅ Closed |
| #122    | Bug: CSV upload sync fails — "Unsupported source type: file" | ✅ Closed |
| #123    | Bug: Ingress Console — hardcoded fake logs, wrong messages   | ✅ Closed |
| #124    | Feat: Support legacy Office formats (.doc, .xls, .ppt)       | ✅ Closed |
| #125    | Feat: LLM fallback extraction — AI document reading          | ✅ Closed |

### Pull Requests
| PR # | Title                                                                 | Status   |
| ---- | --------------------------------------------------------------------- | -------- |
| #130 | feat: Sprint 10/11 — Dashboard, Knowledge Base, Search, Pipeline, ISO | ✅ Merged |
| #131 | test: Sprint 10 E2E browser testing — 6/6 PASS                        | ✅ Merged |
| #132 | docs: Sprint 10 final ISO summary                                     | ✅ Merged |

## 4. รายละเอียดการเปลี่ยนแปลง (Changes Detail)

### Backend (Rust)
1. **`routes/sources.rs`** — Added `POST /sources/:id/extract-ai` (LLM fallback), fixed `file` source_type sync with S3 download
2. **`services/extraction.rs`** — Added `extract_legacy_office()`: LibreOffice headless .doc→.docx, .xls→.xlsx, .ppt→.pptx conversion
3. **`routes/sources.rs`** — `resolve_llm_credentials()`, `call_llm_api()` for multi-provider LLM support

### Frontend (Next.js)
1. **`src/app/sources/page.tsx`** — File upload wizard: file list with X remove buttons, LLM fallback extraction panel with model selector
2. **`src/app/sources/page.tsx`** — Ingress Console: replaced hardcoded fake logs with real polling from `fetchSources()`, source-type-aware messages

## 5. ปัญหาที่พบและวิธีแก้ไข (Issues & Resolutions)
1. **CSV sync "Unsupported source type: file" (#122):**
   - *ปัญหา:* sync_source match didn't handle `"file"` source_type
   - *แก้ปัญหา:* Added `"file"` arm that downloads from S3, auto-detects format by extension, and routes to appropriate extractor

2. **Ingress Console hardcoded logs (#123):**
   - *ปัญหา:* Console showed fake sequence: "Connecting..." → "Found 12 pages" regardless of source type
   - *แก้ปัญหา:* Replaced with periodic polling, displaying real status (chunk count, MB size, error messages)

3. **Legacy Office requires LibreOffice (#124):**
   - *ปัญหา:* .doc/.xls/.ppt need external tool for conversion
   - *แก้ปัญหา:* Uses `soffice --headless --convert-to`, with clear error message + install instructions if not available

## 6. Sprint 12 Planning
| Feature        | Description                                              | Priority |
| -------------- | -------------------------------------------------------- | -------- |
| Web Hierarchy  | Site hierarchy discovery via sitemap/BFS, selective load | High     |
| LLM Usage Logs | Track model, tokens, latency, cost per API call          | Medium   |
| LLM Analytics  | Dashboard for LLM usage visualization                    | Medium   |
| Embedding      | Multi-model embedding + Qdrant per-tenant vector store   | High     |

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
