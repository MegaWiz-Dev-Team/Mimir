# Pre-Launch Review: For UX/UI + Data Engineer Teams
## Quick Checklist (15 min per team)

---

## 📋 What Was Just Designed

**5 Core Documents:**
1. ✅ `SPRINT_1_EXECUTION_DETAILED.md` — Full 10-day plan
2. ✅ `EVALUATION_FRAMEWORK_Insurance_Pipeline.md` — How we measure success
3. ✅ `S1_FIRST_DAY_RUNBOOK.md` — Monday setup (copy-paste ready)
4. ✅ `S1_DAILY_STANDUP_TEMPLATE.md` — Daily tracking + metrics
5. ✅ `S1_PHASE_BY_PHASE_CHECKLIST.md` — What happens each day

**Optional Documents:**
- `PIPELINE_DESIGN_CRITICAL_QUESTIONS.md` — Future domain strategy (S3+)
- `ARCHITECTURE_GAPS_AND_DESIGN.md` — Multi-domain roadmap (S3+)

**Status:** Ready for team review + feedback before May 18 kickoff

---

## 👨‍💻 FOR DATA ENGINEERS: 15-Min Review

**Read:** `S1_FIRST_DAY_RUNBOOK.md` + `S1_PHASE_BY_PHASE_CHECKLIST.md`

### Questions to Answer

1. **Extraction (S1.1, May 18-19)**
   - [ ] Can you extract all 5 Prudential URLs within 2 days?
   - [ ] Will rate limiting be an issue? (Yes/No/Maybe)
   - [ ] Do you need any custom headers or delays?
   - [ ] What format do you prefer output? (TXT / JSON / Other)

2. **Chunking (S1.2, May 20-21)**
   - [ ] 500 tokens/chunk with 100-token overlap: Feasible? (Yes/No)
   - [ ] Can you create 950 chunks from ~200 raw records? (realistic ratio?)
   - [ ] What quality checks do you recommend for chunks?
   - [ ] Any concerns about paragraph boundary detection?

3. **Entity Extraction (S1.3, May 22-24)**
   - [ ] What NER model will you use? (PyThaiNLP? Custom?)
   - [ ] Expected entity types: Product, Coverage, Condition, Exclusion OK? (Any missing?)
   - [ ] Confidence threshold for entities: What's reasonable? (0.7? 0.8? 0.9?)
   - [ ] How long will S1.3 take? (Honest estimate?)

4. **Blockers/Concerns**
   - [ ] Any script dependencies we don't have?
   - [ ] Any environment issues you foresee?
   - [ ] Anything we should test on May 17?
   - [ ] Do you feel ready to start May 18? (Yes/No/Concerns?)

### Feedback Template

```
DATA ENGINEER REVIEW — [Name]

Extraction (S1.1): [READY / NEEDS PREP / NOT READY]
  └─ Can start May 18? [YES / NO / MAYBE]
  └─ Concerns: [list any]

Chunking (S1.2): [READY / NEEDS PREP / NOT READY]
  └─ 950 chunks realistic? [YES / NO / UNSURE]
  └─ Timeline: [2 days is OK / Need more time]
  └─ Concerns: [list any]

Entity Extraction (S1.3): [READY / NEEDS PREP / NOT READY]
  └─ NER model ready? [YES / NO]
  └─ Entity types complete? [YES / MISSING: ...]
  └─ Concerns: [list any]

OVERALL READINESS: [READY / NEEDS PREP / BLOCKERS]
  └─ If not ready, what's blocking? [...]
  └─ When will you be ready? [date/time]

CONFIDENCE (1-10): [_]
  └─ Can we hit all S1.1-S1.3 targets by May 24? [YES / NO / MAYBE]
```

---

## 🎨 FOR UX/UI TEAM: 15-Min Review

**Read:** `EVALUATION_FRAMEWORK_Insurance_Pipeline.md` + `S1_DAILY_STANDUP_TEMPLATE.md`

### Questions to Answer

1. **Search UI (For S1.4 validation, May 25)**
   - [ ] Is search bar ready in Mimir UI?
   - [ ] Can it take 10 test queries? (Yes/No)
   - [ ] What fields should results show?
     - [ ] Product name
     - [ ] Coverage/Exclusion info
     - [ ] Source URL
     - [ ] Relevance score
     - [ ] Other: ___________

2. **Results Display (May 25-26)**
   - [ ] How should results be formatted?
     - [ ] Card view (product card with details)
     - [ ] List view (simple list)
     - [ ] Snippet view (text + highlight)
     - [ ] Other: ___________
   - [ ] Should we show confidence/relevance score? (Yes/No)
   - [ ] Should we show source trail (URL + metadata)? (Yes/No)
   - [ ] Max results to show per query: 3 / 5 / 10?

3. **Test Query Workflow (For Hit Rate check, May 22)**
   - [ ] Can QA enter 10 test queries into search?
   - [ ] Can we capture results for analysis?
   - [ ] Do you need metrics dashboard? (Yes/No)
   - [ ] What metrics should dashboard show?

4. **Blockers/Concerns**
   - [ ] Any UI dependencies on backend?
   - [ ] Is Mimir search API stable?
   - [ ] Can you customize result display?
   - [ ] Do you feel ready for May 22 test queries? (Yes/No)

### Feedback Template

```
UX/UI REVIEW — [Name]

Search UI: [READY / NEEDS WORK / NOT READY]
  └─ Can accept test queries? [YES / NO]
  └─ By May 22? [YES / NO / UNSURE]

Results Display: [READY / NEEDS WORK / NOT READY]
  └─ Preferred format: [CARD / LIST / SNIPPET / OTHER]
  └─ Should show confidence/relevance? [YES / NO]
  └─ Should show source trail? [YES / NO]

Metrics Dashboard: [NOT NEEDED / NICE-TO-HAVE / REQUIRED]
  └─ If needed, priority: [HIGH / MEDIUM / LOW]
  └─ Timeline: [Ready now / Need 1 week]

Test Query Support: [READY / NEEDS PREP / NOT READY]
  └─ Can run by May 22? [YES / NO / UNSURE]
  └─ How many queries? [10 / other: ___]
  └─ Result capture method: [Excel / JSON / API]

BLOCKERS: [NONE / List: ...]

CONFIDENCE (1-10): [_]
  └─ Ready for Hit Rate validation May 22? [YES / NO / UNSURE]
```

---

## 📊 COMBINED REVIEW: Both Teams

**Meeting:** 30 min (not 15 min each — do together)

**Agenda:**
```
1. Data Engineer review (5 min)
   - Extraction/chunking/entities feasible?
   - Timeline: May 18-24 realistic?

2. UX/UI review (5 min)
   - Search UI ready?
   - Results display ready?
   - Hit Rate testing support?

3. Combined discussion (10 min)
   - Any dependencies between teams?
   - Any conflicts? (e.g., UI needs data format X, but data eng wants Y)
   - Risk assessment: What's most likely to slip?

4. Final decision (5 min)
   - READY to kick off May 18? (YES / NO / CONDITIONAL)
   - If conditional, what needs to be fixed before kickoff?
   - Timeline: By May 17 EOD?
```

**Output:**
- [ ] Data Engineer sign-off: ✅ READY
- [ ] UX/UI sign-off: ✅ READY
- [ ] Tech Lead approval: ✅ READY
- [ ] GO DATE: Tuesday May 18, 9:00 AM ✅

---

## 🚨 If Issues Found

**Scenario 1: Data Engineer says "Not ready for May 18"**
```
Timeline slips to: [date]
Reason: [what's blocking]
What's needed to unblock: [action items]
Who owns fixing it: [name]
New kickoff date: [date]
```

**Scenario 2: UX/UI says "Search UI not ready"**
```
Option A: Delay Hit Rate validation from May 22 to [date]
Option B: Use CLI test harness instead of UI for May 22
Option C: Proceed with S1.1-S1.3, delay S1.4-S1.5
Choose: [A / B / C]
```

**Scenario 3: Teams identify dependency conflict**
```
Conflict: [describe]
Solution: [proposed fix]
Owner: [who implements]
Timeline impact: [days slipped]
New plan: [update schedule]
```

---

## ✅ Approval Sign-Off

**Once both teams review, get explicit approval:**

```
DATA ENGINEER SIGN-OFF:
  Name: ___________________
  Status: ✅ READY / ⚠️ NEEDS PREP / ❌ NOT READY
  Signature: ________________ Date: _______

UX/UI SIGN-OFF:
  Name: ___________________
  Status: ✅ READY / ⚠️ NEEDS PREP / ❌ NOT READY
  Signature: ________________ Date: _______

TECH LEAD APPROVAL:
  Name: ___________________
  Status: ✅ APPROVED / ⚠️ CONDITIONAL / ❌ BLOCKED
  Notes: ________________
  Signature: ________________ Date: _______

KICKOFF DECISION:
  [ ] YES — Go May 18, 9:00 AM 🚀
  [ ] NO — Delay until: [date/reason]
  [ ] CONDITIONAL — Fix: [issues] by [date]
```

---

## 📞 Next Steps

**If you find issues:**
1. Document in feedback template above
2. Reply in this thread with your findings
3. We'll convene 30-min team discussion
4. Resolve conflicts + update timeline
5. Get final sign-offs by May 17 EOD

**If everything is ready:**
1. Reply with "✅ Ready" 
2. We proceed with S1 kickoff May 18

---

## 📋 Timeline for This Review

**Today (Friday May 16):**
- [ ] Send this to Data Engineer + UX/UI leads
- [ ] Ask for 15-min independent review
- [ ] Get feedback by EOD

**Tomorrow (Saturday May 17) morning:**
- [ ] 30-min team discussion (if needed)
- [ ] Resolve any conflicts
- [ ] Get final sign-offs

**Sunday May 17 evening:**
- [ ] All approvals in place
- [ ] Confirm GO for May 18

---

## Questions While Reviewing?

**For Data Engineer:** Ask about extraction scripts, data formats, performance  
**For UX/UI:** Ask about search UI, results display, test query workflow  
**For Tech Lead:** Ask about schedule, priorities, blockers  

**Slack channel:** #insurance-s1-sprint (create now)

---

**Status:** Awaiting team review  
**Next:** Your feedback in 1 hour / tomorrow morning  
**Kickoff:** May 18, 9:00 AM (if all clear)

