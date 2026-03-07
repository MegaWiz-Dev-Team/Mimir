# SI-04.{N}: Sprint {N} Test Script ({Feature Name})

**Project Name:** Project Mimir
**Sprint:** Sprint {N}
**Tester:** AI Assistant
**Date:** YYYY-MM-DD
**Status:** ✅ All Tests Passed / ❌ Some Tests Failed

---

## 1. Frontend Build

| ID          | Scenario         | Steps                                     | Expected                              | Result | Issue/PR | หมายเหตุ |
| ----------- | ---------------- | ----------------------------------------- | ------------------------------------- | ------ | -------- | ------- |
| TC_SP{N}_F1 | npm build passes | 1. `cd ro-ai-dashboard && npx next build` | ✓ Compiled, route listed, exit code 0 | ✅ Pass | #{X}     |         |

## 2. Feature Tests

| ID          | Scenario        | Steps               | Expected           | Result | Issue/PR | หมายเหตุ |
| ----------- | --------------- | ------------------- | ------------------ | ------ | -------- | ------- |
| TC_SP{N}_U1 | [Scenario name] | 1. [Step] 2. [Step] | [Expected outcome] | ✅ Pass | #{X}     | [Notes] |
| TC_SP{N}_U2 | [Scenario name] | 1. [Step] 2. [Step] | [Expected outcome] | ✅ Pass | #{X}     | [Notes] |

## 3. Regression Tests

| ID          | Scenario                 | Steps               | Expected           | Result | Issue/PR | หมายเหตุ          |
| ----------- | ------------------------ | ------------------- | ------------------ | ------ | -------- | ---------------- |
| TC_SP{N}_R1 | [Existing feature works] | 1. [Step] 2. [Step] | [Expected outcome] | ✅ Pass | #{X}     | Existing feature |

## 4. Summary

| Category         | Total | Pass  | Fail  |
| ---------------- | ----- | ----- | ----- |
| Frontend Build   | 1     | 1     | 0     |
| Feature Tests    | X     | X     | 0     |
| Regression Tests | X     | X     | 0     |
| **Total**        | **X** | **X** | **0** |
