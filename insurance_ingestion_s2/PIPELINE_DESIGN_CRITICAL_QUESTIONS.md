# Pipeline Design: 10 Critical Questions
## Answer These to Unlock Strategy (Q1 is Critical Path)

**Status:** Required for S2 planning  
**Timeline:** Q1 answer needed by May 27 (end of S1)  
**Effort:** 10 min for Q1 only; full session ~90 min  
**Owner:** Tech lead + product manager

---

## 🚨 CRITICAL PATH: Q1 Must Answer FIRST

### Q1: Domain Scope — How Many Domains?

**Options:**
- **A) Insurance Only** → Stay on current pipeline
- **B) Insurance + Medical** → Use multi-domain architecture (S3)
- **C) 3+ Domains** → Full plugin system (Finance, Legal, etc.)

**Decision Impact:**
| Option | Implication | Effort | ROI |
|--------|------------|--------|-----|
| A | Keep insurance pipeline as-is | None | None |
| B | Refactor after S1 → add medical in S3 | 2 weeks infra | High (1 week per new domain) |
| C | Build full plugin system now | 3 weeks infra | Very high (scales to N domains) |

**How to Decide:**
- Do you have PrimeKG + PubMed ready for medical? → suggests B or C
- Will legal/finance be needed in future? → suggests C
- Focus on insurance perfection only? → suggests A

**What's at Stake:**
- **Option A:** Insurance perfect, medical = 2+ month redesign in S3
- **Option B:** Insurance works, medical added cleanly in 1 week (S3)
- **Option C:** Insurance works, any domain added in 1 week (scales forever)

**Recommendation Based on What I Know:**
- You have PrimeKG ready ✅
- You have PubMed access ✅
- Medical team exists ✅
- → **Leans toward B or C**

---

## Questions 2-10 (Answer After Q1 Decision)

### Q2: Consolidation Strategy

**Context:** Currently using RefGraph (semantic graph + compressed refs)

**Question:** Should consolidation be:
- **A) Domain-specific** (InsuranceConsolidator, MedicalConsolidator, etc.)
- **B) Generic** (one consolidator, config-driven)
- **C) Hybrid** (generic base + domain plugins)

**Impacts:**
- A: Simpler, less abstraction, more code duplication
- B: More abstract, reusable, harder to debug
- C: Best of both, moderate complexity

**Recommendation:** C (Hybrid) — Base Consolidator class + domain implementations

---

### Q3: Entity Types — How Many, How Defined?

**Question:** Should entity types be:
- **A) Hardcoded** (only "product" for insurance; add "drug" later)
- **B) Configured** (entity_types.json registry)
- **C) Pluggable** (SQL table, admin UI to create new entity types)

**Current:** Hardcoded as "product"  
**Medical needs:** "drug", "condition", "guideline"  
**Legal needs:** "case", "statute", "precedent"  

**Implications:**
- A: Fast, simple, high refactor cost for new domains
- B: Medium abstraction, JSON-based, good for known domains
- C: Fully extensible, requires admin UI, more infrastructure

**Recommendation:** B (Configured) — Good balance of abstraction + simplicity

---

### Q4: Relationship Types — Registry or Hardcoded?

**Question:** Should relationship types be:
- **A) Hardcoded** (has_coverage, excludes defined in code)
- **B) Registry** (relationship_types.json)
- **C) Admin UI** (create relationships in UI)

**Current:** Hardcoded in consolidation script  
**Insurance:** has_coverage, excludes, requires_age  
**Medical:** treats, contraindicated_with, causes, interacts_with  
**Legal:** cites_statute, follows_precedent, overruled_by  

**Recommendation:** B (Registry) — Same as Q3, JSON-based

---

### Q5: Confidence/Evidence Scoring — Model Per Domain?

**Question:** How should confidence be modeled?

- **A) Single scale** (0.0-1.0 for all domains)
- **B) Domain-specific models** (simple for insurance, evidence_hierarchy for medical)
- **C) Per-relationship configuration** (each relationship defines its own scoring)

**Current Insurance:** Simple 0.7-1.0  
**Medical needs:** Evidence hierarchy (A/B/C/D) + severity (CRITICAL/MODERATE/MILD)  
**Legal needs:** Authority hierarchy (official/published/commentary/media/speculation)  

**Implications:**
- A: Simplest, loses domain-specific semantics
- B: Better UX, requires domain registry
- C: Most flexible, complex to implement

**Recommendation:** B (Domain-specific) — Evidence Model Registry

---

### Q6: Neo4j Schema — Generic or Domain-Specific?

**Question:** Should Neo4j have:
- **A) One schema** (generic Entity + Relationship nodes, all domains use same)
- **B) Domain schemas** (Insurance nodes, Medical nodes, etc.)
- **C) Multiple schemas** (separate databases per domain)

**Implications:**
- A: Single query language, cross-domain queries possible, requires entity_type filtering
- B: Optimized per domain, more indexes, harder to query across domains
- C: Simplest per domain, impossible to query across domains, scaling nightmare

**Recommendation:** A (Generic) — All domains use Entity + Relationship, filtered by domain

---

### Q7: Performance SLOs — Per Domain?

**Question:** What performance targets?

**Insurance S1 baseline:**
- Search latency: < 500ms (p99)
- Hit Rate@3: ≥ 75%
- Throughput: 100 QPS

**Medical expectations (hypothetical):**
- Search latency: < 1000ms (more complex queries)
- Hit Rate@3: ≥ 70% (safety-critical, different metrics)
- Throughput: 50 QPS (fewer users initially)

**Legal expectations (hypothetical):**
- Search latency: < 2000ms (complex reasoning)
- Relevance: HIGH (accuracy > speed)
- Throughput: 10 QPS (expert users)

**Question:** Define per-domain SLOs now or wait?

**Recommendation:** Define for S1 insurance now; others in S3 planning

---

### Q8: Backwards Compatibility — Must Preserve?

**Question:** If we refactor pipeline for multi-domain:

- **A) Break S1 insurance** (redesign after May 27)
- **B) Preserve S1 insurance** (new pipeline parallel with old, migrate gradually)
- **C) Wrap S1 insurance** (keep working, expose via new pattern)

**Risk:**
- A: Risky, but saves refactoring work
- B: Safe, but requires both pipelines
- C: Safe, requires wrapper, moderate complexity

**Recommendation:** C (Wrap) — Insurance pipeline keeps working, exposed via new abstraction

---

### Q9: UI/UX — Multi-Domain Aware or Separate?

**Question:** Should UI be:
- **A) Single tenant** (insurance only in UI)
- **B) Multi-tenant view** (switch between insurance/medical/legal in same UI)
- **C) Separate UIs** (insurance.mimir.local, medical.mimir.local, etc.)

**Implications:**
- A: Simpler, no multi-domain complexity
- B: Unified experience, more UX work
- C: Separate codebases, scaling overhead

**Recommendation:** B (Multi-tenant view) — Single unified UI, domain selector

---

### Q10: Multi-Tenancy Model — Does It Map to Domains?

**Current:**
- asgard_insurance tenant → insurance data
- asgard_medical tenant → medical data (future)

**Question:** Should:
- **A) One tenant = one domain** (asgard_insurance for all insurance, asgard_medical for all medical)
- **B) One domain = many insurers** (asgard_insurance with insurer_id field)
- **C) Custom mapping** (some domains multi-tenant, some single-tenant)

**Current Decision:** A + B (one insurance tenant, but multi-insurer within it)

**Medical scenario:**
- One medical tenant or per-hospital? → Probably one (shared knowledge base)

**Recommendation:** A + B (one tenant per domain, but multi-entity within tenant if needed)

---

## 🎯 Decision Framework: How to Answer

### If Answering Q1 Only (Recommended)
```
1. Read Q1 options (A/B/C)
2. Ask yourself: "Will we need medical + legal in future?"
3. Answer in 5 sentences
4. Done

Time: 10 minutes
Impact: HIGH (unlocks Q2-10)
```

### If Answering All 10 Questions
```
1. Read each question
2. Discuss with tech lead + product
3. Document decisions (1-2 sentences per)
4. Note assumptions
5. Flag any blockers

Time: 90 minutes (1.5 hour meeting)
Impact: VERY HIGH (full strategy locked)
```

### Recommended Approach
```
THIS WEEK (before S1 kickoff):
  ✅ Answer Q1 only (10 min)
  ✅ Share with tech lead
  ✅ Proceed with S1 as planned

AFTER S1 SUCCESS (May 27-30):
  ✅ Schedule 90-min decision session
  ✅ Answer Q2-Q10
  ✅ Lock S3 architecture
  ✅ Begin S3 planning
```

---

## 📋 Quick Reference: Q1 Decision Tree

```
START: Do you want medical domain support?

├─ NO → Answer: A (Insurance Only)
│   └─ S2/S3 focused on insurance perfection
│   └─ New domains require full redesign
│   └─ Effort: Minimal now, high later
│
├─ YES, Medical only → Answer: B (Insurance + Medical)
│   └─ Refactor after S1 → add medical in S3
│   └─ 2-week infrastructure investment
│   └─ 1-week medical consolidator
│   └─ Medical added in 1 week vs 2+ months
│   └─ Effort: Moderate now, high ROI
│
└─ YES, Medical + Future → Answer: C (Multi-Domain System)
    └─ Build full plugin system now
    └─ 3-week infrastructure investment
    └─ Any domain added in 1 week
    └─ Scales to N domains
    └─ Effort: Higher now, massive ROI for 3+
```

---

## 🎯 Next Steps Based on Your Answer

### If You Answer A (Insurance Only)
- [ ] Archive multi-domain docs (for future reference)
- [ ] Proceed with S1 insurance sprint
- [ ] When new domain needed → revisit architecture

### If You Answer B (Insurance + Medical)
- [ ] Move 4 design docs to docs/future/
- [ ] Keep S1 insurance on track
- [ ] After S1 success → begin S3 planning with those docs
- [ ] 2-week infrastructure → 1-week medical consolidator

### If You Answer C (Multi-Domain System)
- [ ] Move 4 design docs to docs/future/ (now docs/strategy/)
- [ ] Keep S1 insurance on track
- [ ] Add 2-3 weeks infrastructure to S2 or S3 timeline
- [ ] Then any domain in 1 week (scales)

---

## 📞 How to Communicate Your Answer

**Format:**
```
Q1 Answer: [A/B/C]

Reasoning: [1-2 sentences]
- Do we have medical domain ready? [Yes/No]
- Will we need legal domain later? [Yes/No]
- Timeline preference? [Minimize now vs minimize future]

Next step: [Proceed with S1 / Schedule Q2-10 meeting]
```

**Examples:**

**Example 1 (Insurance only):**
```
Q1 Answer: A

Reasoning: Focus on insurance perfection first. No immediate medical need. 
Can design multi-domain later if required.

Next step: Proceed with S1 insurance sprint unchanged.
```

**Example 2 (Insurance + Medical):**
```
Q1 Answer: B

Reasoning: Have PrimeKG + PubMed ready. Medical team exists. Want clean 
support in S3, not 2-month redesign.

Next step: Proceed S1 insurance. Schedule 90-min Q2-10 meeting for May 30.
```

**Example 3 (Full multi-domain):**
```
Q1 Answer: C

Reasoning: Will need legal + finance eventually. Better to invest 3 weeks 
now than 2 weeks per domain later.

Next step: Proceed S1 insurance. Add 2 weeks infrastructure to S2 timeline. 
Then medical/legal/finance each in 1 week.
```

---

## ✅ Deliverables

Once you answer:
- [ ] I provide Q2-10 analysis (if requested)
- [ ] Architecture docs organized per your choice (A/B/C)
- [ ] S2/S3 timeline adjusted to your decision
- [ ] Team assignments clear for next sprint

---

**Status:** Ready for Q1 answer  
**Your move:** What's your choice? A, B, or C?
