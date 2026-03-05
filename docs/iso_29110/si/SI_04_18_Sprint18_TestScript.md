# SI-04.18: Sprint 18 Test Script (Coverage Analytics Dashboard)

**Project Name:** Project Mimir
**Sprint:** Sprint 18
**Tester:** AI Assistant
**Date:** 2026-03-04
**Status:** ✅ All Tests Passed

---

## 1. Unit Tests — Backend

### 1.1 Build Verification
| ID         | Scenario               | Steps                                                | Expected                   | Result | Issue/PR | หมายเหตุ            |
| ---------- | ---------------------- | ---------------------------------------------------- | -------------------------- | ------ | -------- | ------------------ |
| TC_SP18_U1 | cargo check — 0 errors | 1. รัน `cargo check 2>&1 \| grep -cE "^error"`        | Output: 0                  | ✅ Pass | #188     | Clean build        |
| TC_SP18_U2 | coverage tests (14)    | 1. รัน `cargo test --lib -p ro-ai-bridge -- coverage` | 14 tests pass, exit code 0 | ✅ Pass | #188     | All new tests pass |

### 1.2 Coverage Score Calculation Tests (3 tests)
| ID         | Scenario               | Steps                                   | Expected                        | Result | Issue/PR | หมายเหตุ          |
| ---------- | ---------------------- | --------------------------------------- | ------------------------------- | ------ | -------- | ---------------- |
| TC_SP18_C1 | All stages present     | 1. `calculate_coverage_score(10,5,3,2)` | 100.0                           | ✅ Pass | #188     | 4×25 = 100       |
| TC_SP18_C2 | No stages present      | 1. `calculate_coverage_score(0,0,0,0)`  | 0.0                             | ✅ Pass | #188     |                  |
| TC_SP18_C3 | Partial (1/2/3 stages) | 1. Various combinations                 | 25.0 / 50.0 / 75.0 respectively | ✅ Pass | #188     | Boundary testing |

### 1.3 Blindspot Detection Tests (3 tests)
| ID         | Scenario         | Steps                            | Expected                                                           | Result | Issue/PR | หมายเหตุ       |
| ---------- | ---------------- | -------------------------------- | ------------------------------------------------------------------ | ------ | -------- | ------------- |
| TC_SP18_B1 | All healthy      | 1. All metrics above threshold   | Empty blindspot vector                                             | ✅ Pass | #188     | No flags      |
| TC_SP18_B2 | No chunks at all | 1. chunk=0, qa=0, vec=0, kg=0    | ["no_chunks","no_qa_pairs","low_vector_coverage","no_kg_entities"] | ✅ Pass | #188     | All flags set |
| TC_SP18_B3 | High dedup only  | 1. dedup_ratio=0.5, rest healthy | ["high_dedup_ratio"]                                               | ✅ Pass | #188     | Single flag   |

### 1.4 Overall Score Tests (3 tests)
| ID         | Scenario      | Steps                        | Expected | Result | Issue/PR | หมายเหตุ   |
| ---------- | ------------- | ---------------------------- | -------- | ------ | -------- | --------- |
| TC_SP18_O1 | Full coverage | 1. 4/4 sources in all stages | 100.0    | ✅ Pass | #188     |           |
| TC_SP18_O2 | No sources    | 1. total_sources = 0         | 0.0      | ✅ Pass | #188     | Edge case |
| TC_SP18_O3 | Half coverage | 1. 2/4 sources in each stage | 50.0     | ✅ Pass | #188     |           |

### 1.5 Serialization Tests (3 tests)
| ID         | Scenario                   | Steps                         | Expected                       | Result | Issue/PR | หมายเหตุ        |
| ---------- | -------------------------- | ----------------------------- | ------------------------------ | ------ | -------- | -------------- |
| TC_SP18_S1 | CoverageOverview roundtrip | 1. Serialize → deserialize    | Fields match original          | ✅ Pass | #188     | JSON roundtrip |
| TC_SP18_S2 | SourceCoverage serialize   | 1. Serialize, check JSON keys | Contains source_id, blindspots | ✅ Pass | #188     |                |
| TC_SP18_S3 | CoverageGaps roundtrip     | 1. Serialize → deserialize    | Gap arrays match               | ✅ Pass | #188     |                |

### 1.6 Route Assembly Test (1 test)
| ID         | Scenario        | Steps                  | Expected                     | Result | Issue/PR | หมายเหตุ |
| ---------- | --------------- | ---------------------- | ---------------------------- | ------ | -------- | ------- |
| TC_SP18_R1 | Routes assembly | 1. `coverage_routes()` | Router created without panic | ✅ Pass | #188     |         |

---

## 2. Frontend Tests

### 2.1 Build Verification
| ID         | Scenario             | Steps                                    | Expected                           | Result | Issue/PR | หมายเหตุ      |
| ---------- | -------------------- | ---------------------------------------- | ---------------------------------- | ------ | -------- | ------------ |
| TC_SP18_F1 | npm run build passes | 1. `cd ro-ai-dashboard && npm run build` | ✓ Compiled, /coverage route listed | ✅ Pass | #188     | 19 routes OK |

### 2.2 Feature Verification
| ID         | Scenario                 | Steps                            | Expected                                               | Result | Issue/PR | หมายเหตุ              |
| ---------- | ------------------------ | -------------------------------- | ------------------------------------------------------ | ------ | -------- | -------------------- |
| TC_SP18_F2 | Coverage page renders    | 1. Navigate to /coverage         | Page shows KPI cards, pipeline flow, gap panel, table  | ✅ Pass | #188     | Replaces Coming Soon |
| TC_SP18_F3 | API interfaces type-safe | 1. Check api.ts TypeScript types | CoverageOverview, SourceCoverage, CoverageGaps defined | ✅ Pass | #188     | 3 interfaces + 3 fns |
| TC_SP18_F4 | Sortable table           | 1. Click column headers          | Table sorts by name/score/chunks/qa/kg                 | ✅ Pass | #188     | Asc/desc toggle      |
| TC_SP18_F5 | Gap filter               | 1. Click gap item in panel       | Table filters to matching sources                      | ✅ Pass | #188     | Clear filter button  |

---

## 3. Agent Studio & Infrastructure Tests

### 3.1 Agent Chat (Heimdall)
| ID         | Scenario                     | Steps                                                               | Expected                                   | Result | Issue/PR | หมายเหตุ                   |
| ---------- | ---------------------------- | ------------------------------------------------------------------- | ------------------------------------------ | ------ | -------- | ------------------------- |
| TC_SP18_A1 | Chat via Heimdall→Qwen3.5-9B | 1. Open Agent Studio → Chat with Mimir → Send "hello"               | Agent responds in Thai (HTTP 200)          | ✅ Pass | #194     | Latency ~42s for 9B model |
| TC_SP18_A2 | Agent Studio UX redesign     | 1. Navigate to /agents → Check stats bar, card design, chat sidebar | Stats bar, provider-colored cards rendered | ✅ Pass | —        | Professional UI           |

### 3.2 Vault Persistent Storage
| ID         | Scenario                      | Steps                                                       | Expected                                | Result | Issue/PR | หมายเหตุ                    |
| ---------- | ----------------------------- | ----------------------------------------------------------- | --------------------------------------- | ------ | -------- | -------------------------- |
| TC_SP18_V1 | Secrets persist after restart | 1. Save secret → `docker restart mimir_vault` → Read secret | Secret still accessible after restart   | ✅ Pass | #194     | file backend works         |
| TC_SP18_V2 | Auto-unseal on restart        | 1. `docker restart mimir_vault` → Check logs                | Logs show "Vault unsealed" + Root Token | ✅ Pass | #194     | entrypoint.sh via HTTP API |

### 3.3 Layout Fix
| ID         | Scenario                     | Steps                                         | Expected                      | Result | Issue/PR | หมายเหตุ                  |
| ---------- | ---------------------------- | --------------------------------------------- | ----------------------------- | ------ | -------- | ------------------------ |
| TC_SP18_H1 | Hydration warning suppressed | 1. Open browser console → Navigate to /agents | No hydration mismatch warning | ✅ Pass | —        | suppressHydrationWarning |

**Grand Total: 24/24 (100%)**

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด SI-04)*

