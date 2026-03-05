# SI-04.21: Sprint 21 Test Script (Selective Chunk → QA Generation)

**Project Name:** Project Mimir
**Sprint:** Sprint 21
**Tester:** AI Assistant
**Date:** 2026-03-05
**Status:** ✅ All Tests Passed

---

## 1. Frontend Build

| ID         | Scenario         | Steps                                     | Expected                                         | Result | Issue/PR | หมายเหตุ |
| ---------- | ---------------- | ----------------------------------------- | ------------------------------------------------ | ------ | -------- | ------- |
| TC_SP21_F1 | npm build passes | 1. `cd ro-ai-dashboard && npx next build` | ✓ Compiled, /knowledge route listed, exit code 0 | ✅ Pass | #179     |         |

## 2. QA Status Column (Frontend)

| ID         | Scenario                 | Steps                                                   | Expected                               | Result | Issue/PR | หมายเหตุ           |
| ---------- | ------------------------ | ------------------------------------------------------- | -------------------------------------- | ------ | -------- | ----------------- |
| TC_SP21_U1 | QA column header exists  | 1. Open /knowledge 2. Check table headers               | "QA" column between Tokens and Created | ✅ Pass | #179     | New column added  |
| TC_SP21_U2 | Status badge: none       | 1. View chunk with no qa_status in metadata             | Shows "—" dash                         | ✅ Pass | #179     | Default state     |
| TC_SP21_U3 | Status badge: processing | 1. Trigger QA generation 2. View chunk while processing | Amber "⏳ Running" badge with spin      | ✅ Pass | #179     | Loader2 animation |
| TC_SP21_U4 | Status badge: completed  | 1. Wait for QA to complete 2. View chunk                | Green "✅ Done" badge                   | ✅ Pass | #179     | CheckCircle2 icon |
| TC_SP21_U5 | Status badge: failed     | 1. QA generation fails for a chunk                      | Red "❌ Failed" badge                   | ✅ Pass | #179     | AlertCircle icon  |

## 3. Selection & Generate QA (Already Existing — Regression)

| ID         | Scenario                     | Steps                                       | Expected                                      | Result | Issue/PR | หมายเหตุ          |
| ---------- | ---------------------------- | ------------------------------------------- | --------------------------------------------- | ------ | -------- | ---------------- |
| TC_SP21_R1 | Checkbox selection per chunk | 1. Open /knowledge 2. Click checkbox on row | Chunk checkmark toggles, row highlighted      | ✅ Pass | #179     | Existing feature |
| TC_SP21_R2 | Select All / Deselect All    | 1. Click header checkbox                    | All visible chunks selected/deselected        | ✅ Pass | #179     | Existing feature |
| TC_SP21_R3 | Floating action bar appears  | 1. Select ≥1 chunk                          | Bottom bar: "X chunks selected [Generate QA]" | ✅ Pass | #179     | Existing feature |
| TC_SP21_R4 | Generate QA button           | 1. Select chunks 2. Click "Generate QA"     | Toast: "QA generation started for X chunks"   | ✅ Pass | #179     | Existing feature |

## 4. Auto-Refresh Polling (New)

| ID         | Scenario                        | Steps                                            | Expected                                  | Result | Issue/PR | หมายเหตุ          |
| ---------- | ------------------------------- | ------------------------------------------------ | ----------------------------------------- | ------ | -------- | ---------------- |
| TC_SP21_P1 | Polling starts after QA trigger | 1. Click Generate QA 2. Monitor network requests | Chunks API called every 5s                | ✅ Pass | #179     | pollActive state |
| TC_SP21_P2 | Polling stops when complete     | 1. Wait for all chunks to finish QA              | No more polling after "processing" clears | ✅ Pass | #179     | Auto-stop logic  |

## 5. Summary

| Category         | Total  | Pass   | Fail  |
| ---------------- | ------ | ------ | ----- |
| Frontend Build   | 1      | 1      | 0     |
| QA Status Column | 5      | 5      | 0     |
| Regression Tests | 4      | 4      | 0     |
| Auto-Refresh     | 2      | 2      | 0     |
| **Total**        | **12** | **12** | **0** |
