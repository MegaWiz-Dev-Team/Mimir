# SI-04-10-E2E — Sprint 10 E2E Browser Test Script

| Field       | Value                               |
| ----------- | ----------------------------------- |
| Sprint      | 10                                  |
| Tester      | AI (Browser Agent)                  |
| Date        | 2026-02-27                          |
| Environment | localhost:3000 (Next.js dev server) |
| Backend     | localhost:8080 (ro-ai-bridge)       |

---

## TC_SP10_E2E_01 — Add Source Wizard: Web Scraper Flow

| Step | Action                          | Expected                                   | Result |
| ---- | ------------------------------- | ------------------------------------------ | ------ |
| 1    | Navigate to `/sources`          | Sources page loads with source list        | ✅ Pass |
| 2    | Click "+ Add Source"            | Wizard panel opens at Step 1 (Source Type) | ✅ Pass |
| 3    | Click "Web Scraper"             | Auto-advances to Step 2 (Configure Source) | ✅ Pass |
| 4    | Type URL: `https://example.com` | URL field accepts input                    | ✅ Pass |
| 5    | Type Name: `E2E Test Web`       | Source name field accepts input            | ✅ Pass |
| 6    | Click "Back"                    | Returns to Step 1 (Source Type)            | ✅ Pass |
| 7    | Click "Web Scraper" again       | Returns to Step 2 with data preserved      | ✅ Pass |
| 8    | Close wizard                    | Wizard closes, Sources page visible        | ✅ Pass |

**Status: PASS ✅**

---

## TC_SP10_E2E_02 — Add Source Wizard: File Upload Flow

| Step | Action                     | Expected                                                   | Result |
| ---- | -------------------------- | ---------------------------------------------------------- | ------ |
| 1    | Click "+ Add Source"       | Wizard opens at Step 1                                     | ✅ Pass |
| 2    | Click "File Upload"        | Advances to Step 2 with dropzone                           | ✅ Pass |
| 3    | Verify UI elements         | Source Name, Upload Files, Upload Folder, Dropzone visible | ✅ Pass |
| 4    | Verify accepted formats    | PDF, DOCX, DOC, XLSX, XLS, PPTX, PPT, CSV, TXT, JSON, MD   | ✅ Pass |
| 5    | Type Name: `E2E File Test` | Name field accepts input                                   | ✅ Pass |
| 6    | Click "Back"               | Returns to Step 1                                          | ✅ Pass |

**Status: PASS ✅**

---

## TC_SP10_E2E_03 — Add Source Wizard: MCP Connection Flow

| Step | Action                                | Expected                                  | Result |
| ---- | ------------------------------------- | ----------------------------------------- | ------ |
| 1    | Click "MCP Connection"                | Advances to Step 2 with Connection String | ✅ Pass |
| 2    | Type Name: `E2E MCP Test`             | Name field accepts input                  | ✅ Pass |
| 3    | Type URL: `http://localhost:9000/mcp` | Connection String field accepts input     | ✅ Pass |
| 4    | Click "Back"                          | Returns to Step 1                         | ✅ Pass |
| 5    | Close wizard                          | Wizard closes cleanly                     | ✅ Pass |

**Status: PASS ✅**

---

## TC_SP10_E2E_04 — Pipeline Status Bar

| Step | Action                         | Expected                                | Result |
| ---- | ------------------------------ | --------------------------------------- | ------ |
| 1    | View Pipeline bar on all pages | Bar visible at top with 5 stages        | ✅ Pass |
| 2    | Verify stage names             | Sources → Chunks → Dedup → QA → Vector  | ✅ Pass |
| 3    | Verify real data               | Sources: 10, Chunks: 424, Dedup: 3 done | ✅ Pass |
| 4    | Verify future stages           | QA and Vector show "—" (greyed out)     | ✅ Pass |

**Status: PASS ✅**

---

## TC_SP10_E2E_05 — Knowledge Base Page

| Step | Action                   | Expected                                    | Result |
| ---- | ------------------------ | ------------------------------------------- | ------ |
| 1    | Navigate to `/knowledge` | Knowledge Base page loads (not placeholder) | ✅ Pass |
| 2    | Verify search bar        | "Search chunks by content..." input visible | ✅ Pass |
| 3    | Verify source filter     | "All Sources" dropdown visible              | ✅ Pass |
| 4    | Verify empty state       | "No chunks found" with "Go to Sources" CTA  | ✅ Pass |
| 5    | Verify stats badges      | Shows "0 chunks" and "0 tokens"             | ✅ Pass |

**Status: PASS ✅**

---

## TC_SP10_E2E_06 — Search Settings Tab

| Step | Action                               | Expected                               | Result |
| ---- | ------------------------------------ | -------------------------------------- | ------ |
| 1    | Navigate to `/settings` → Search tab | Search & Retrieval Settings form loads | ✅ Pass |
| 2    | Verify Embedding Model               | Dropdown with 5 model options          | ✅ Pass |
| 3    | Verify Top-K Results                 | Number input, default 5                | ✅ Pass |
| 4    | Verify Similarity Threshold          | Slider 0.00-1.00, default 0.70         | ✅ Pass |
| 5    | Verify Search Mode                   | 3 toggles: Semantic, Hybrid, Keyword   | ✅ Pass |
| 6    | Click "Keyword" mode                 | Activates with blue highlight          | ✅ Pass |
| 7    | Verify Save button                   | Disabled with "Sprint 12" note         | ✅ Pass |

**Status: PASS ✅**

---

## Summary

| Test Case      | Description                | Result |
| -------------- | -------------------------- | ------ |
| TC_SP10_E2E_01 | Web Scraper Wizard Flow    | ✅ Pass |
| TC_SP10_E2E_02 | File Upload Wizard Flow    | ✅ Pass |
| TC_SP10_E2E_03 | MCP Connection Wizard Flow | ✅ Pass |
| TC_SP10_E2E_04 | Pipeline Status Bar        | ✅ Pass |
| TC_SP10_E2E_05 | Knowledge Base Page        | ✅ Pass |
| TC_SP10_E2E_06 | Search Settings Tab        | ✅ Pass |

**Overall: 6/6 PASS ✅**
