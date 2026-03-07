---
name: iso-documentation
description: ISO/IEC 29110 documentation workflow for Project Mimir — creating Sprint Reports (PM-02), Test Scripts (SI-04), updating Traceability Matrix (SI-03), and maintaining compliance with documentation standards. Triggers when creating reports, closing sprints, writing test documentation, or updating project management documents.
---

# ISO/IEC 29110 Documentation Skill

Project Mimir follows ISO/IEC 29110 (Software Life-Cycle Profiles for Very Small Entities). This skill guides the Agent through creating and updating all required documentation artifacts.

## Document Hierarchy

```
docs/iso_29110/
├── pm/                              # Project Management
│   ├── PM_01_Project_Plan.md        # Master project plan
│   ├── PM_02_Status_Reports.md      # Master registry of all sprints
│   ├── PM_02_{N}_Sprint{N}_Report.md # Individual sprint reports
│   └── PM_03_Project_Closure.md     # Project closure report
└── si/                              # Software Implementation
    ├── SI_01_Software_Requirements_Specification.md
    ├── SI_02_Software_Design_Document.md
    ├── SI_03_Traceability_Matrix.md
    └── SI_04_{N}_Sprint{N}_TestScript.md  # Per-sprint test scripts
```

## Sprint Report (PM-02) — When a Sprint Completes

### Required Sections
1. **Header**: Project Name, Sprint number, Status (✅/❌), Date
2. **ขอบเขตของ Sprint (Sprint Scope)**: Bullet list of deliverables, scope Issue numbers
3. **สรุปผลการทดสอบ (Testing Summary)**: Category × Total × Pass table
4. **GitHub Synchronization**: Issue/PR × Title × Status table
5. **ไฟล์ที่แก้ไข (Files Changed)**: File × Change Type × Description table
6. **Technical Decisions**: Key architecture/design decisions made

### Rules
- File naming: `PM_02_{N}_Sprint{N}_Report.md` (e.g., `PM_02_21_Sprint21_Report.md`)
- Title format: `PM-02.{N}: Sprint {N} Status Report ({Feature Name})`
- ALL section headers MUST use **Thai** (with English in parentheses)
- Always update `PM_02_Status_Reports.md` master registry with the new sprint

### Template
See `examples/sprint_report_template.md` for the canonical template.

## Test Script (SI-04) — For Each Sprint

### Required Sections
1. **Header**: Project Name, Sprint, Tester, Date, Status
2. **Test Categories**: Group tests by feature area (Frontend Build, Unit Tests, Regression, etc.)
3. **Test Table Columns**: ID | Scenario | Steps | Expected | Result | Issue/PR | หมายเหตุ
4. **Summary Table**: Category × Total × Pass × Fail

### Test ID Naming Convention
- Format: `TC_SP{sprint}_{category}{number}`
- Categories:
  - `F` = Frontend Build
  - `U` = Unit/Feature test
  - `R` = Regression test
  - `I` = Integration test
  - `P` = Performance/Polling test
  - `S` = Security test
- Example: `TC_SP21_U3` = Sprint 21, Unit test #3

### Rules
- File naming: `SI_04_{N}_Sprint{N}_TestScript.md`
- Each test row MUST link to a GitHub Issue/PR number
- Results use emoji: `✅ Pass` or `❌ Fail`
- หมายเหตุ (Notes) column for additional context

### Template
See `examples/test_script_template.md` for the canonical template.

## Traceability Matrix (SI-03) Update

When completing a sprint, update `SI_03_Traceability_Matrix.md`:
1. Add new requirement rows mapping to Implementation Plan → Test Cases → Issues/PRs
2. Ensure every feature has a traceable chain: Requirement → Design → Code → Test → Report

## Master Status Report (PM_02_Status_Reports.md)

After creating a sprint report, append a new row to the master sprint table:
- Sprint number, date range, status, key deliverables summary, link to individual report

## Issue/Change Log

When bugs or architectural changes occur, ALWAYS append to the Issue/Change Logs table in `PM_02_Status_Reports.md` with:
- Issue #, Description, Sprint discovered, Sprint resolved, Status

## Sprint Closure Checklist

Before marking a Sprint as complete:
- [ ] All test cases pass (SI-04 test script complete)
- [ ] Sprint report created (PM-02)
- [ ] Master status report updated
- [ ] Traceability matrix updated (SI-03)
- [ ] All GitHub Issues in sprint scope are closed or deferred with justification
- [ ] Files changed list is accurate in sprint report
