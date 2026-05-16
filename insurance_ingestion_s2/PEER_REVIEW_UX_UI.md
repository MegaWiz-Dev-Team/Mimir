# PEER REVIEW: UX/UI + Search Perspective
## Review of S1 Sprint Plan

**Reviewer:** Senior UX/UI Engineer (Search + Mimir interface)  
**Date:** May 16, 2026  
**Documents Reviewed:** S1_DAILY_STANDUP_TEMPLATE.md, EVALUATION_FRAMEWORK_Insurance_Pipeline.md  
**Time:** 15 minutes

---

## ✅ What Looks Good

1. **Hit Rate@3 metric is clear** ✅
   - Measurable (≥75% = success)
   - Achievable with good search
   - Good decision gate for May 22

2. **10 test queries are reasonable** ✅
   - Mix of lookup, reasoning, safety queries
   - Representative of real user needs
   - Easy to run and validate manually

3. **Metrics tracking template is actionable** ✅
   - Daily metrics show progress
   - Slack integration feasible
   - Clear go/no-go signal

4. **Search latency requirement clear** ✅
   - < 500ms p99 is reasonable
   - Achievable with current Qdrant setup

---

## ⚠️ Issues Found

### Issue 1: Search UI Not Ready for May 22

**Problem:**  
Plan requires running 10 test queries on May 22-25:
- Where do QA enter the queries? (CLI? UI?)
- How do we capture results for analysis?
- Mimir UI search is functional but not optimized

**Impact:** HIGH  
- May 22 Hit Rate validation won't have proper UI
- QA forced to use CLI instead of polished search bar
- Looks unprofessional for stakeholder demo

**Recommendation:**
```
Option A: Use CLI test harness (fast, boring)
  └─ Command: scripts/test_queries.py --query "..." --domain insurance
  └─ Pros: Ready today, reliable
  └─ Cons: Not visual, can't show to stakeholders

Option B: Quick UI enhancement (1-2 days work)
  └─ Add domain selector dropdown
  └─ Add test query buttons (pre-populated)
  └─ Add result formatting (relevance score visible)
  └─ Pros: Professional, shareable
  └─ Cons: May delay if done May 20-21

Option C: Hybrid (recommended)
  └─ CLI for actual validation (technical)
  └─ UI demo for stakeholders (visual proof)
  └─ Take 2-3 hours total
```

**Recommendation:** Option C (hybrid approach)  
- Run actual validation via CLI (no UI dependency)
- Take screenshots of UI results for demo
- Both technical rigor + visual polish

**Fix Required:** Yes, clarify approach  
**Owner:** UX/UI + QA  
**Timeline:** Decide by May 17, implement by May 22

---

### Issue 2: Result Display Format Undefined

**Problem:**  
Tests return search results, but format not specified:
- Show just snippet? Or full context?
- Show relevance score? (0.92 = very relevant)
- Show source URL?
- Show confidence level? (from Skuggi validation)

**Impact:** MEDIUM  
- QA won't know what good results look like
- May miss quality issues in early chunks
- Scoring/ranking unclear

**Recommendation:**
```
RESULT DISPLAY FORMAT:

┌────────────────────────────────────────────────────┐
│ "products with critical illness coverage"          │
│                                                    │
│ Result 1: CRITICAL ILLNESS COVERAGE                │
│  ├─ Snippet: "PRU Mao Mao offers critical illness  │
│  │            coverage up to 2,000,000 baht..."   │
│  ├─ Relevance: 0.95 ⭐⭐⭐ (very relevant)        │
│  ├─ Source: prudential.co.th/health/ (official)   │
│  ├─ PII Score: ✅ 0.0 (safe)                       │
│  └─ Confidence: 0.99 (from consolidation)         │
│                                                    │
│ Result 2: ROOM CHARGE LIMITS                       │
│  ├─ Snippet: "Room charges covered up to 6,000    │
│  │            baht per day..."                    │
│  ├─ Relevance: 0.87 ⭐⭐⭐                         │
│  ├─ Source: PRU Mao Mao PDF (official)            │
│  ├─ PII Score: ✅ 0.0 (safe)                       │
│  └─ Confidence: 0.98                              │
│                                                    │
│ [3 more results...]                               │
└────────────────────────────────────────────────────┘

KEY FIELDS:
✅ Title/snippet (what is the result about?)
✅ Relevance score (how good is this match?)
✅ Source (where did we get this info?)
✅ PII clearance (is it safe to show?)
✅ Confidence (how sure are we?)
```

**Fix Required:** Yes, add to EVALUATION_FRAMEWORK  
**Owner:** UX/UI  
**Timeline:** Define by May 18

---

### Issue 3: No Domain/Tenant Selector in Search

**Problem:**  
Currently Mimir searches across default tenant.
- User can't specify "insurance only" vs "medical" (future)
- If medical data added, results get mixed
- Plan doesn't mention tenant selection

**Impact:** MEDIUM  
- Works for S1 (insurance only)
- Will break when S3 (medical) added
- Should design UI to support it now

**Recommendation:**
```
ADD DOMAIN SELECTOR (future-proofing):

┌──────────────────────────────────────────┐
│ [Insurance ▼] Search: ________________   │
│                                           │
│ Results for: Insurance Products           │
│ Tenant: asgard_insurance                 │
│                                           │
│ [Top 10 results shown]                   │
└──────────────────────────────────────────┘

For S1: Show "Insurance" only
For S3+: Show dropdown with [Insurance, Medical, Legal]

Timeline: Not critical for S1, but easy to add
Owner: UX/UI
Effort: 30 minutes
```

**Fix Required:** Not critical for S1, but recommended  
**Owner:** UX/UI (optional for S1)  
**Timeline:** Can be added anytime

---

### Issue 4: No Manual Result Validation UI

**Problem:**  
QA needs to manually validate results are accurate:
- Is relevance score correct?
- Is source attribution correct?
- Is the snippet helpful?
- Should this rank higher/lower?

**Impact:** LOW  
- QA currently has to manually grade 30+ results by eye
- Could be faster with UI checkboxes
- Nice to have, not critical

**Recommendation:**
```
ADD VALIDATION FORM (nice-to-have):

For each search result:
  ☐ Relevant? (YES / NO / SOMEWHAT)
  ☐ Source correct? (YES / NO / UNSURE)
  ☐ Snippet useful? (YES / NO)
  ☐ Rank correct? (Should be higher / Same / Lower)
  ☐ Confidence in result: [1-10]

Saves time over manual grading
Timeline: Not critical for S1
Owner: UX/UI (if done)
Effort: 2-3 hours
```

**Fix Required:** No, skip for S1  
**Owner:** UX/UI (future enhancement)  
**Timeline:** Can be added in S2

---

### Issue 5: No Results "Explainability" Feature

**Problem:**  
Users (stakeholders) can't understand WHY a result ranked high:
- Which keywords matched?
- How was relevance calculated?
- Is this from a trusted source?
- Could this be a false positive?

**Impact:** LOW-MEDIUM  
- Affects stakeholder confidence
- Hard to debug poor results
- Good for future credibility

**Recommendation:**
```
ADD EXPLAINABILITY (future feature):

Result card shows:
  Title: [snippet]
  Relevance: 0.95 
  
  WHY HIGH RANK?
  ├─ Keywords matched: "critical", "illness", "coverage" (3/3)
  ├─ Source authority: Official PDF (high trust)
  ├─ Freshness: Extracted May 16 (recent)
  └─ User rating: 5/5 (validated as accurate)

Timeline: Not critical for S1
Owner: UX/UI + Backend
Effort: 1-2 days
```

**Fix Required:** No, skip for S1  
**Owner:** UX/UI + Backend (future enhancement)  
**Timeline:** S2 nice-to-have

---

### Issue 6: Metrics Dashboard Not Specified

**Problem:**  
EVALUATION_FRAMEWORK mentions metrics (Hit Rate, MRR, NDCG) but no dashboard:
- Who views these metrics daily?
- Where are they displayed?
- How are they updated?

**Impact:** MEDIUM  
- Team doesn't know how to track progress
- No visibility into Hit Rate until May 22
- Daily standup can't show trending

**Recommendation:**
```
METRICS DASHBOARD (required for daily standup):

Option A: Google Sheet
  Pros: Easy, collaborative, no coding
  Cons: Manual updates
  Timeline: 1 hour to set up
  Use: Daily by tech lead

Option B: Grafana dashboard
  Pros: Auto-updating, professional
  Cons: Requires setup
  Timeline: 4-6 hours
  Use: For stakeholder demos

Option C: CLI tool (minimal)
  Pros: Works anywhere, simple
  Cons: Less visual
  Timeline: 1 hour
  Use: Daily by QA

RECOMMENDATION: Option A + Option C
  - Google Sheet for daily tracking (human readable)
  - CLI tool for automation (CI/CD integration)
  - Stakeholder demo uses screenshots
```

**Fix Required:** Yes, define approach  
**Owner:** Tech Lead + Data Engineer  
**Timeline:** Decide by May 17, build by May 18

---

## 🟢 Confidence Assessment

| Component | Status | Risk |
|-----------|--------|------|
| Test query ability | ⚠️ UNCLEAR | MEDIUM |
| Result display format | ❌ UNDEFINED | MEDIUM |
| Hit Rate validation | ⚠️ POSSIBLE | LOW |
| Metrics tracking | ⚠️ UNCLEAR | MEDIUM |
| Domain selector | ✅ OK (future) | LOW |
| Stakeholder demo | ⚠️ UNCLEAR | MEDIUM |

**Overall:** 6/10 - Functional, but UX polish needs work

---

## 📋 Sign-Off

| Criterion | Status |
|-----------|--------|
| Can we run 10 test queries? | ⚠️ VIA CLI (not UI) |
| Can we measure Hit Rate? | ✅ YES |
| Is search UI production-ready? | ⚠️ FUNCTIONAL BUT BASIC |
| Can we demo results to stakeholders? | ⚠️ POSSIBLE (screenshots) |
| Metrics visible daily? | ❌ NOT PLANNED |
| Results properly formatted? | ❌ UNDEFINED |

**OVERALL READINESS:** ⚠️ **NEEDS PREP**

**What needs to happen:**
1. ✅ Issue 1: Choose UI approach (CLI vs UI vs hybrid) - decide by May 17
2. ✅ Issue 2: Define result display format - by May 18
3. ✅ Issue 3: Design domain selector (optional for S1) - nice-to-have
4. ⏭️ Issue 4: Manual validation form (skip for S1) - S2 feature
5. ⏭️ Issue 5: Explainability feature (skip for S1) - S2 feature
6. ✅ Issue 6: Build metrics dashboard - by May 18

**Recommendation:**
- Use CLI for actual Hit Rate validation (technical, reliable)
- Take UI screenshots for stakeholder demo (polished)
- Define result format clearly (before QA starts testing)
- Build Google Sheet for daily metrics (quick, effective)

---

## Feedback from UX/UI

**Name:** [UX/UI Lead]  
**Status:** ⚠️ **NEEDS PREP**  
**Critical Issues:** Issues 1, 2, 6 (need decisions/build)  
**By When Ready:** May 17 EOD (critical items) + May 18 (refinement)  
**Confidence:** 6/10 (will be 8/10 with fixes)

**What to Prepare by May 17 EOD:**
- [ ] Decide: CLI test harness vs UI vs hybrid (Issue 1)
- [ ] Define: Result display format (Issue 2)
- [ ] Build: Metrics dashboard (Issue 6)

**Can start May 18?** 
- ✅ YES, if decisions made
- ⚠️ May be basic/CLI-based instead of polished

---

## Feedback Summary

**From Data Engineer:**
- ⚠️ NEEDS PREP (Issues 1-6: PyThaiNLP, rate limiting, dedup, tokens, thresholds, fallback)
- Ready by: May 17 EOD (Issues 1-2), May 21 (Issues 3-6)
- Confidence: 7/10

**From UX/UI:**
- ⚠️ NEEDS PREP (Issues 1-6: UI ready, format, dashboard)
- Ready by: May 17 EOD (critical decisions), May 18 (build)
- Confidence: 6/10

---

**Overall S1 Readiness After Both Reviews: 6.5/10**

**Action Items to Fix Issues:**
1. Data Engineer: Issues 1-2 by May 17 EOD
2. UX/UI: Issues 1, 2, 6 by May 17-18
3. Tech Lead: Approve fixes, add contingencies

**Can we launch May 18?** ⚠️ **CONDITIONAL YES**
- Proceed if Issues 1-2 (Data Eng) + Issues 1,2,6 (UX/UI) are addressed
- May 17 EOD deadline for critical items
- Less polished than ideal, but functionally ready

---

**Signature:** _________________ **Date:** May 16, 2026

