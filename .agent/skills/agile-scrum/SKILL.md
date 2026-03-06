---
name: agile-scrum
description: Agile Scrum feature management and sprint scope discipline for Project Mimir — Product Backlog management via GitHub Issues, Sprint scope sanctity, mid-sprint adjustment protocols, sprint planning workflows, and session boundaries. Triggers when planning sprints, managing features, handling scope changes, or discussing project management.
---

# Agile Scrum Skill

Project Mimir follows Agile Scrum adapted for a small team (VSE) with ISO 29110 traceability. This skill enforces sprint discipline while maintaining flexibility.

## Core Principles

1. **Sprint Scope Sanctity** — No unplanned work mid-sprint without trade-offs
2. **Product Backlog = GitHub Issues** — Single source of truth
3. **1 Sprint = 1 Conversation** — Context boundaries for optimal AI performance
4. **ISO Traceability** — Every feature links to Issue → PR → Test → Report

## Product Backlog Management

### Creating Backlog Items
When the user requests a new feature or reports a non-critical bug that is NOT in the current sprint:
1. **DO NOT** implement it immediately
2. Create a GitHub Issue using `mcp_github-mcp-server` tools
3. Apply appropriate labels: `feature`, `bug`, `enhancement`, `documentation`
4. Inform the user: "ได้สร้าง Issue #{X} ไว้ใน Backlog แล้วครับ จะ prioritize ในการวางแผน Sprint ถัดไป"

### Backlog Grooming
Between sprints, assist the user in:
1. Reviewing open GitHub Issues
2. Prioritizing by business value and technical dependencies
3. Estimating effort (Small/Medium/Large)
4. Selecting items for the next sprint

## Sprint Scope Rules

### ✅ Allowed Mid-Sprint
- Bug fixes for features IN the current sprint
- Minor adjustments to sprint items (same scope, different approach)
- Documentation updates

### ❌ Not Allowed Mid-Sprint
- New features not in the sprint plan
- Scope expansion of existing sprint items without trade-offs
- "Quick" additions that aren't tracked

### Mid-Sprint Adjustment Protocol
If the user insists on adding work mid-sprint:
1. **Acknowledge**: "เข้าใจครับว่าสำคัญ"
2. **Trade-off**: "ถ้าจะเพิ่ม feature นี้ ต้องตัด feature ไหนออกจาก Sprint นี้ดีครับ?"
3. **Document**: If agreed, update:
   - Implementation Plan document
   - `PM_02_Status_Reports.md` Change Logs
   - Related GitHub Issues (re-label, re-milestone)

## Sprint Lifecycle

### 1. Sprint Planning
```
Review Backlog → Prioritize → Select Items → Write Implementation Plan → Start Sprint
```

- Create `docs/03_*_Implementation_Plan_Sprint{N}.md`
- Define clear deliverables, phases, and acceptance criteria
- Create GitHub Issues for each work item

### 2. Sprint Execution
```
For each item: Issue → Branch → TDD → Test → Commit → PR
```

- Follow `code-review` skill for GitHub workflow
- Follow `tdd` skill for test-driven development
- Follow `testing-workflow` skill for test documentation

### 3. Sprint Closure
```
Run all tests → Create SI-04 Test Script → Create PM-02 Report → Update master docs → Close Issues
```

- Follow `iso-documentation` skill for report creation
- Verify 100% test pass rate before marking sprint complete
- Update `PM_02_Status_Reports.md` master registry

## Session Boundaries

### 1 Sprint = 1 Conversation
- When a sprint completes and a new sprint begins, **inform the user to start a new conversation**
- The AI relies on Knowledge Items (KIs) to retain context between sessions
- Starting fresh prevents context window overload and stale file buffers

### When to Suggest New Conversation
- Sprint is officially closed
- New phase/sprint begins
- Conversation has been running for 20+ major tasks
- Context switch to unrelated project area

### How to Suggest
> "Sprint {N} เสร็จเรียบร้อยแล้วครับ 🎉 แนะนำให้เปิด conversation ใหม่สำหรับ Sprint {N+1} เพื่อให้ AI ทำงานได้แม่นยำที่สุดครับ"
