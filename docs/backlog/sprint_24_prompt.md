# Sprint 24 Prompt: UX Flow & Navigation

**Theme:** 🟡 ปรับ user journey ให้ตรงกับ mental model ของ Agentic RAG
**Sprint Goal:** ผู้ใช้ใหม่สามารถ setup ระบบ RAG ได้ภายใน 10 นาที

---

## B-04: Reorganize Nav Groups [P1, Size: S]

### Problem
Graph อยู่ใน "AI" group แต่ Knowledge Graph เป็น data processing step (build graph จาก chunks) ไม่ใช่ AI feature

### Current Nav
```
Overview | Data (Sources, Knowledge, Vector, Quality) | AI (Playground, Agents, Graph) | Analytics (Coverage, LLM, Evaluations, Logs) | Admin
```

### Proposed Nav
```
Overview | Data (Sources, Knowledge, Quality) | Processing (Vector, Graph) | AI (Agents, Playground) | Analytics (Evaluations, Coverage, LLM, Logs) | Admin
```

### Files to Change
- `ro-ai-dashboard/src/components/navbar.tsx` — update `navGroups` array

### Acceptance Criteria
- [ ] Graph moved out of AI group
- [ ] Nav order follows pipeline flow: Data → Processing → AI → Analytics
- [ ] All links still work
- [ ] Active state highlighting correct

---

## B-05: Getting Started Onboarding Wizard [P2, Size: L]

### Concept
First-time user checklist overlay ที่ track progress:

```
Getting Started with Project Mimir
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
☑ 1. Create Tenant          → /tenants
☑ 2. Configure LLM Provider → /settings (AI Models tab)
☐ 3. Add Data Source         → /sources
☐ 4. Sync & Process          → /sources (Sync button)
☐ 5. Test in Playground      → /playground
```

### Files to Create
- `ro-ai-dashboard/src/components/onboarding-wizard.tsx`
- Backend: track completion in `tenant_configs` or localStorage

### Acceptance Criteria
- [ ] Checklist appears for new tenants
- [ ] Each step links to the correct page
- [ ] Steps auto-complete when user finishes them
- [ ] Can dismiss/hide the wizard
- [ ] Persists across page reloads

---

## B-06: Pipeline Status Breadcrumb [P2, Size: M]

### Concept
Visual indicator on dashboard showing RAG pipeline readiness:

```
Sources ✅ → Chunks ✅ → QA ⏳ → Vector ❌ → Agent ❌
```

### Data Source
Combine existing APIs: `fetchStats()`, `fetchSources()`, `fetchVectorStats()`

### Files to Create/Modify
- `ro-ai-dashboard/src/components/pipeline-breadcrumb.tsx` — new component
- `ro-ai-dashboard/src/app/page.tsx` — add to dashboard

### Acceptance Criteria
- [ ] Shows current state of each pipeline stage
- [ ] Clickable — each step links to its page
- [ ] Updates in real-time (uses existing polling)
- [ ] Responsive design

---

## B-07: Simplify Source Wizard [P3, Size: M]

### Problem
3-step wizard (Type → Config → Advanced) — most users don't need step 3

### Proposed Change
- Merge "Advanced Settings" toggle into step 2 (collapsed by default)
- Reduce to 2 steps: Type → Config (with optional advanced expand)

### Files to Modify
- `ro-ai-dashboard/src/app/sources/page.tsx` — wizard step logic

### Acceptance Criteria
- [ ] 2-step wizard by default
- [ ] Advanced settings accessible via expandable section
- [ ] All source types still work (file, URL, sitemap, DB)

---

## Parallel: Skill Enhancement (B-08 to B-11)

เนื่องจากเป็น documentation-only สามารถทำ parallel ระหว่าง sprint ได้:

| ID   | Item                  | File to Create                                                      |
| ---- | --------------------- | ------------------------------------------------------------------- |
| B-08 | SSE Streaming pattern | `.agent/skills/nextjs-frontend-patterns/examples/sse_streaming.md`  |
| B-09 | Wizard pattern        | `.agent/skills/nextjs-frontend-patterns/examples/wizard_pattern.md` |
| B-10 | Eval Matrix pattern   | `.agent/skills/nextjs-frontend-patterns/examples/eval_matrix.md`    |
| B-11 | Data Pipeline skill   | `.agent/skills/data-pipeline/SKILL.md`                              |

---

## ISO Documentation
- [ ] PM-02.24 Sprint Report
- [ ] SI-04.24 Test Script
- [ ] SI-03 Traceability Matrix update
