# SI-04-10: Sprint 10 Test Script (Dashboard Redesign & Knowledge Base)
**Project Name:** Project Mimir
**Sprint:** 10
**Feature:** Dashboard Redesign, Knowledge Base Page, Search Settings, Dashboard UX Fixes
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

#### 1.1 Backend API Endpoints

| ID             | Test Scenario             | Action / Steps (ขั้นตอนการทดสอบ)                                                           | Expected Result (ผลที่คาดหวัง)                                                                    | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                |
| :------------- | :------------------------ | :--------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------- | :---------- | :------------- | :------------------------------------- |
| **TC_SP10_U1** | Stats API endpoint        | 1. `curl -H "Auth: Bearer $TOKEN" http://localhost:8080/api/v1/stats`                    | JSON response: `total_sources`, `total_chunks`, `qa_pairs`, `vector_coverage`, `source_health` | ✅ Pass      | #115           | Returns aggregated dashboard stats     |
| **TC_SP10_U2** | Sync All Sources endpoint | 1. `curl -X POST -H "Auth: Bearer $TOKEN" http://localhost:8080/api/v1/sources/sync-all` | 200 OK, triggers sync for all sources                                                          | ✅ Pass      | #115           | New POST endpoint                      |
| **TC_SP10_U3** | Chunks List API           | 1. `curl -H "Auth: Bearer $TOKEN" http://localhost:8080/api/v1/chunks`                   | JSON response with paginated chunks list, search & filter support                              | ✅ Pass      | #116           | Supports source_id, search, pagination |
| **TC_SP10_U4** | Chunk Detail API          | 1. `curl -H "Auth: Bearer $TOKEN" http://localhost:8080/api/v1/chunks/:id`               | JSON response with full chunk content                                                          | ✅ Pass      | #116           | Returns single chunk by ID             |

#### 1.2 Frontend Build Verification

| ID             | Test Scenario               | Action / Steps (ขั้นตอนการทดสอบ)               | Expected Result (ผลที่คาดหวัง)                                             | ผลการประเมิน | Issue # / PR # | หมายเหตุ            |
| :------------- | :-------------------------- | :------------------------------------------- | :---------------------------------------------------------------------- | :---------- | :------------- | :----------------- |
| **TC_SP10_U5** | Frontend Build — All Routes | 1. รัน `cd ro-ai-dashboard && npx next build` | Build ผ่าน, route `/knowledge` renders functional page (not placeholder) | ✅ Pass      | #116, #117     | Knowledge + Search |

---

### ส่วนที่ 2: การตรวจสอบระบบผ่านหน้าจอ (Frontend UI Verification)

#### 2.1 Dashboard Redesign (#115)

| ID             | Test Scenario                         | Action / Steps (ขั้นตอนการทดสอบ)                             | Expected Result (ผลที่คาดหวัง)                                                                      | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                         |
| :------------- | :------------------------------------ | :--------------------------------------------------------- | :----------------------------------------------------------------------------------------------- | :---------- | :------------- | :---------------------------------------------- |
| **TC_SP10_01** | Dashboard — KPI Cards (with fallback) | 1. เปิดหน้า Dashboard<br>2. ตรวจสอบ KPI Cards 4 ใบ           | แสดง Total Sources (4), Total Chunks (43), QA Pairs (0), Vector Coverage (0%) — ใช้ fallback data | ✅ Pass      | #115, #118     | Fallback from sources data when stats API fails |
| **TC_SP10_02** | Dashboard — Recent Activity           | 1. ตรวจสอบ Recent Activity section                         | แสดง source events เรียงตาม updated_at, ภาษาเป็น English ("Updated Xm ago")                        | ✅ Pass      | #115, #118     | Fixed mixed Thai/English language               |
| **TC_SP10_03** | Dashboard — Source Health Donut       | 1. ตรวจสอบ Source Health section                           | แสดง donut chart: 1 healthy (สีเขียว), 3 failed (สีแดง) — fallback จาก sources data                 | ✅ Pass      | #115, #118     | Computed from source last_sync_status           |
| **TC_SP10_04** | Dashboard — Quick Actions             | 1. ตรวจสอบ Quick Actions section                           | แสดง 3 buttons: Add Source, Sync All Sources, Open Playground                                    | ✅ Pass      | #115           | All 3 actions functional                        |
| **TC_SP10_05** | Dashboard — No redundant pills        | 1. ตรวจสอบว่าไม่มี color pills ซ้ำกับ Global Pipeline Status bar | ไม่มี pills (SOURCES 0, GENERATING 0, ...) อยู่ — เหลือแค่ Global bar ด้านบน                            | ✅ Pass      | #118           | Removed redundant pills                         |

#### 2.2 Pipeline Status Fixes (#119, #120)

| ID             | Test Scenario                           | Action / Steps (ขั้นตอนการทดสอบ)                                  | Expected Result (ผลที่คาดหวัง)                                           | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                                  |
| :------------- | :-------------------------------------- | :-------------------------------------------------------------- | :-------------------------------------------------------------------- | :---------- | :------------- | :------------------------------------------------------- |
| **TC_SP10_06** | Global Bar — Stage names consistent     | 1. เปิด Dashboard<br>2. ตรวจสอบ Global Pipeline Status bar ด้านบน | แสดง: SOURCES 4 → INGESTED 1 → CHUNKED 1 → QA READY 0 → VECTORIZED 0  | ✅ Pass      | #120           | Stage names now match Pipeline table                     |
| **TC_SP10_07** | Global Bar — Data matches KPI           | 1. เปรียบเทียบตัวเลข Global bar vs KPI cards                       | SOURCES ตรงกับ Total Sources (4), Pipeline counts consistent           | ✅ Pass      | #120           | All numbers from fetchSources() — single source of truth |
| **TC_SP10_08** | Pipeline Table — Alignment              | 1. ดู Pipeline Status table<br>2. ตรวจสอบ icon ✅ / ○ ตรง header  | Icons (✅, ○, 🔒) center ตรงหัวคอลัมน์ (Ingest, Chunks, Dedup, QA, Vector) | ✅ Pass      | #119           | flex justify-center applied                              |
| **TC_SP10_09** | Pipeline Table — Empty states           | 1. ตรวจสอบ source ที่ยังไม่ processed (test, test2, timesheet)      | แสดง ○ (gray circle) แทน "—", QA/Vector แสดง 🔒 lock icon              | ✅ Pass      | #119           | Gray circle = not reached, Lock = not available yet      |
| **TC_SP10_10** | Pipeline Table — Source names clickable | 1. ตรวจสอบ Source Name ในตาราง                                  | ชื่อ source เป็นสีน้ำเงิน (link) คลิกไปหน้า /sources ได้                        | ✅ Pass      | #119           | Blue link with hover underline                           |
| **TC_SP10_11** | Pipeline Table — Type badges            | 1. ตรวจสอบ Type column                                          | แสดง badge สีพื้น: File (สีม่วง), Web (สีน้ำเงิน) อ่านง่าย                       | ✅ Pass      | #119           | Colored badges with icon                                 |
| **TC_SP10_12** | Pipeline Table — Summary footer         | 1. ตรวจสอบ header ของ Pipeline table                            | แสดง "1/4 fully processed" สรุปสถานะรวม                                | ✅ Pass      | #119           | Quick overview in table header                           |

#### 2.3 Knowledge Base Page (#116)

| ID             | Test Scenario               | Action / Steps (ขั้นตอนการทดสอบ)                         | Expected Result (ผลที่คาดหวัง)                                            | ผลการประเมิน | Issue # / PR # | หมายเหตุ                              |
| :------------- | :-------------------------- | :----------------------------------------------------- | :--------------------------------------------------------------------- | :---------- | :------------- | :----------------------------------- |
| **TC_SP10_13** | Knowledge — Page renders    | 1. คลิก "Knowledge" บน Nav bar                          | แสดง Knowledge Base page ไม่ใช่ placeholder "Coming in Sprint 10"        | ✅ Pass      | #116           | Functional page replaces placeholder |
| **TC_SP10_14** | Knowledge — Chunk table     | 1. ตรวจสอบ chunk table                                 | แสดงตาราง chunks: Index, Source, Content preview, Tokens, Date         | ✅ Pass      | #116           | Paginated table with real data       |
| **TC_SP10_15** | Knowledge — Search & Filter | 1. ค้นหาใน search box<br>2. เลือก source filter dropdown | Search filters chunks by content, source dropdown filters by source_id | ✅ Pass      | #116           | Both filter mechanisms work          |

#### 2.4 Search Settings Tab (#117)

| ID             | Test Scenario                | Action / Steps (ขั้นตอนการทดสอบ)           | Expected Result (ผลที่คาดหวัง)                                                                                  | ผลการประเมิน | Issue # / PR # | หมายเหตุ              |
| :------------- | :--------------------------- | :--------------------------------------- | :----------------------------------------------------------------------------------------------------------- | :---------- | :------------- | :------------------- |
| **TC_SP10_16** | Search Tab — No placeholder  | 1. ไปหน้า Settings<br>2. คลิก Tab "Search" | ไม่แสดง "Coming in Sprint 10" — แสดง Search settings form แทน                                                 | ✅ Pass      | #117           | Placeholder removed  |
| **TC_SP10_17** | Search Tab — All form fields | 1. ตรวจสอบ fields ใน Search tab          | แสดง: Embedding Model (dropdown), Top-K (number), Similarity Threshold (slider), Search Mode (hybrid/sem/kw) | ✅ Pass      | #117           | All 4 fields present |

#### 2.5 Tenant & Vector Coverage Corrections

| ID             | Test Scenario                         | Action / Steps (ขั้นตอนการทดสอบ) | Expected Result (ผลที่คาดหวัง)                                                | ผลการประเมิน | Issue # / PR # | หมายเหตุ                                      |
| :------------- | :------------------------------------ | :----------------------------- | :------------------------------------------------------------------------- | :---------- | :------------- | :------------------------------------------- |
| **TC_SP10_18** | Tenant dropdown — real data           | 1. ดู Tenant dropdown บน navbar | แสดง "Default Tenant" เท่านั้น (ไม่มี Ragnarok TH, Medical Clinic A ที่ hardcode) | ✅ Pass      | —              | Now uses fetchTenants() API                  |
| **TC_SP10_19** | Vector Coverage — correct calculation | 1. ดู Vector Coverage KPI card  | แสดง 0% (ไม่ใช่ 25%) เพราะ vectorization ยังไม่ implement                      | ✅ Pass      | —              | Was incorrectly counting sources with chunks |

---

**สรุปผลการทดสอบ Sprint 10 (Sign-off):**
- [x] Backend API tests ผ่าน (4/4: TC_SP10_U1~U4)
- [x] Frontend Build ผ่าน (1/1: TC_SP10_U5)
- [x] Dashboard Redesign ผ่าน (5/5: TC_SP10_01~05)
- [x] Pipeline Status Fixes ผ่าน (7/7: TC_SP10_06~12)
- [x] Knowledge Base ผ่าน (3/3: TC_SP10_13~15)
- [x] Search Settings ผ่าน (2/2: TC_SP10_16~17)
- [x] Tenant & Vector ผ่าน (2/2: TC_SP10_18~19)

**ผลการทดสอบ 2026-02-27:**
- **Unit/API Tests**: 5/5 ✅ (TC_SP10_U1~U5)
- **UI Tests**: 14/14 ✅ (TC_SP10_01~12, TC_SP10_16~17)
- **Integration Tests**: 5/5 ✅ (TC_SP10_13~15, TC_SP10_18~19)
- **Total**: **24/24 ผ่านทั้งหมด (100%)**

**อ้างอิง (GitHub References):**
- **Issues:** #114, #115, #116, #117, #118, #119, #120
- **Pull Requests:** (pending — changes on working branch)
