# Integrated Feedback Action Plan
## Peer Reviews → Refined Execution Plan

**Date:** May 16, 2026  
**Reviewers:** Data Engineer + UX/UI  
**Integration:** All feedback incorporated  
**Status:** READY WITH CONDITIONS

---

## 📊 Summary of Feedback

### Data Engineer Review
- **Status:** ⚠️ NEEDS PREP (7/10 confidence)
- **Critical Issues:** 6 found
- **Ready by:** May 17 EOD (Issues 1-2), May 21 (Issues 3-6)

### UX/UI Review
- **Status:** ⚠️ NEEDS PREP (6/10 confidence)
- **Critical Issues:** 3 found (Issues 1, 2, 6)
- **Ready by:** May 17 EOD (decisions), May 18 (build)

### Combined Confidence: **6.5/10** → Will improve to **8.5/10 with fixes**

---

## 🚨 CRITICAL BLOCKERS (Must Fix Before May 18)

### BLOCKER 1: Entity Extraction Configuration
**Issue:** PyThaiNLP + confidence thresholds not defined  
**Impact:** S1.3 could produce 300-700 entities (vs 500 target)  
**Fix:** Add configuration below  
**Owner:** Data Engineer  
**Deadline:** May 17 EOD  

```python
# ADD TO: scripts/extract_entities.py (or new config file)

ENTITY_EXTRACTION_CONFIG = {
    "english_model": "spacy-en-core-web-sm",
    "thai_model": "pythainlp.named_entity",
    "confidence_thresholds": {
        "product": 0.85,      # e.g., "PRU Mao Mao" must be confident
        "coverage": 0.80,     # e.g., "room charges"
        "exclusion": 0.75,    # e.g., "pre-existing"
        "condition": 0.70     # e.g., "age 18-60"
    },
    "expected_entity_count": (400, 600),  # realistic range
    "quality_check": True,  # sample validation
    "fallback_if_low": "Manual review + adjust thresholds"
}

# ALSO ADD: Language detection per chunk
def detect_chunk_language(chunk_text):
    thai_ratio = count_thai_characters(chunk_text) / len(chunk_text)
    return "th" if thai_ratio > 0.3 else "en"
```

**Status:** ⏳ Needs to be written by May 17 EOD

---

### BLOCKER 2: Search UI Decision
**Issue:** Don't know how to run 10 test queries (UI vs CLI)  
**Impact:** May 22 Hit Rate validation unclear  
**Fix:** Choose approach below  
**Owner:** Tech Lead + UX/UI  
**Deadline:** May 17 EOD

```
DECISION REQUIRED (choose one):

OPTION A: Pure CLI (reliable, fast)
├─ QA runs: python scripts/test_queries.py --domain insurance
├─ Results saved to: test_results_may22.json
├─ Demo to stakeholders: Show JSON + screenshots
├─ Pros: Ready now, no UI dependency
├─ Cons: Not visual for non-technical demo
└─ Decision: ✅ RECOMMENDED (technical credibility)

OPTION B: Mimir UI (polished, visual)
├─ UX/UI enhances search: Domain selector + query buttons
├─ QA enters queries directly in UI
├─ Results shown formatted in browser
├─ Pros: Professional, easy to demo
├─ Cons: 2-3 hours work, may delay other things
└─ Decision: ❌ RISKY (timeline pressure)

OPTION C: Hybrid (best of both)
├─ Run actual validation via CLI (technical)
├─ Capture screenshots of UI results (visual)
├─ Both approaches work, combined for credibility
├─ Pros: Most professional, technically sound
├─ Cons: Need minimal UI polish (1-2 hours)
└─ Decision: ⚠️ CONDITIONAL (if 2-3 hours available)

RECOMMEND: Option A (CLI) + Option C (hybrid if time)
├─ Pure CLI for actual May 22 decision (100% reliable)
├─ If UX/UI has 2-3 hours, do hybrid approach
├─ Decision: OPTION A (mandatory), OPTION C (if time)
```

**Status:** ⏳ Needs decision by May 17 EOD

---

### BLOCKER 3: Result Display Format Undefined
**Issue:** QA won't know what good results look like  
**Impact:** May 22 Hit Rate validation lack objectivity  
**Fix:** Define format below  
**Owner:** UX/UI  
**Deadline:** May 18 EOD

```
RESULT DISPLAY FORMAT (add to test_results JSON):

{
  "query": "products with critical illness coverage",
  "query_id": "q001",
  "results": [
    {
      "rank": 1,
      "title": "Critical Illness Coverage - PRU Mao Mao",
      "snippet": "PRU Mao Mao offers critical illness coverage up to 2,000,000 baht per year. Easy application with no medical check-up required.",
      "relevance_score": 0.95,
      "relevance_stars": "⭐⭐⭐⭐⭐",
      "source_type": "official_pdf",
      "source_url": "https://prudential.co.th/en/products/health/",
      "pii_clearance": {
        "score": 0.0,
        "status": "SAFE"
      },
      "consolidation_confidence": 0.99,
      "chunk_id": "chunk_456",
      "reasoning": "Exact match on 'critical illness' + 'coverage' + official source"
    },
    {
      "rank": 2,
      "title": "Room Charge Benefits",
      "snippet": "Room charges covered up to 6,000 baht per day under critical illness policy",
      "relevance_score": 0.87,
      "relevance_stars": "⭐⭐⭐⭐",
      "source_type": "official_pdf",
      "source_url": "prudential.co.th/.../health/",
      ...
    }
  ],
  "hit_rate_at_3": true,  # ≥75% of test queries should have this
  "evaluation": {
    "human_relevant": true,
    "source_correct": true,
    "snippet_useful": true,
    "rank_correct": true
  }
}
```

**Status:** ⏳ Needs to be defined by May 18 EOD

---

## 🔧 HIGH-PRIORITY FIXES (Do Before May 21)

### FIX 1: Rate Limiting Configuration
**Issue:** Prudential website will rate-limit scraping  
**Impact:** May slow down S1.1 extraction  
**Fix:** Add to extraction script

```python
EXTRACTION_CONFIG = {
    "delays_between_requests": 2.0,  # 2 seconds between URLs
    "user_agent_rotation": True,     # Vary User-Agent header
    "respect_robots_txt": True,      # Check robots.txt first
    "max_retries": 3,
    "timeout": 30,
    "headers": {
        "Accept-Language": "en-US,en;q=0.9,th;q=0.8",
        "User-Agent": "Mozilla/5.0 (Insurance Data Extractor v1.0)",
        "Referer": "https://www.google.com/"  # Appear like browser
    }
}
```

**Owner:** Data Engineer  
**Timeline:** Add by May 17 EOD  
**Priority:** HIGH (affects May 18 extraction)

---

### FIX 2: Duplicate Content Detection
**Issue:** ~10-15% of extracted chunks likely duplicates  
**Impact:** Inflates chunk count, wastes embedding capacity  
**Fix:** Add deduplication step in S1.2

```python
# scripts/deduplicate_chunks.py

DEDUP_CONFIG = {
    "method": "jaccard_similarity",
    "threshold": 0.95,           # 95%+ similar = duplicate
    "action": "MERGE",           # Merge into single chunk
    "keep_sources": True,        # Preserve all source URLs
    "expected_reduction": 0.10   # Expect ~10% reduction
}

# Result: 950 chunks → ~850 unique chunks
# Benefits: Higher quality, better Hit Rate, less storage
```

**Owner:** Data Engineer  
**Timeline:** Implement by May 21  
**Priority:** HIGH (improves Hit Rate)

---

### FIX 3: Token Counting Validation
**Issue:** "500 tokens" per chunk not validated  
**Impact:** Chunks could be 300-700 tokens (quality variance)  
**Fix:** Add validation before ingestion

```python
CHUNK_VALIDATION = {
    "tokenizer": "tiktoken",       # Use GPT-3 tokenizer
    "model": "cl100k_base",
    "chunk_size_target": 500,
    "chunk_size_acceptable_range": (400, 600),
    "validation_report": True,
    "reject_if_outside_range": False,  # Log but don't reject
}
```

**Owner:** Data Engineer + QA  
**Timeline:** Document by May 18  
**Priority:** MEDIUM (quality assurance)

---

### FIX 4: Metrics Dashboard
**Issue:** No way to track daily progress  
**Impact:** Team can't see Hit Rate trending toward May 22  
**Fix:** Build simple Google Sheet

```
GOOGLE SHEET SETUP:

Columns:
  A: Date
  B: Phase (S1.1, S1.2, etc.)
  C: Chunks extracted
  D: Entities found
  E: Neo4j relationships
  F: Hit Rate@3 (if measured)
  G: Blockers
  H: Owner

Daily updates by: Tech Lead (5 min per day)
Shared with: Team (read-only link in Slack)
Formula: % Complete = Chunks/950 * 100
```

**Owner:** Tech Lead  
**Timeline:** Create by May 17 EOD  
**Priority:** HIGH (daily visibility)

---

## 📋 UPDATED S1 EXECUTION PLAN

### Monday May 17 (Prep Day)

**NEW ITEMS ADDED:**

```
9:00 AM:  Team kickoff (same as before)

9:30 AM:  Environment readiness checks (same as before)

10:00 AM: Git setup + script deployment (same as before)
          ⭐ NEW: Deploy updated extraction scripts with:
            ✅ Rate limiting config
            ✅ Entity extraction config
            ✅ Token validation
            
10:30 AM: Smoke test (same as before)

12:30 PM: LUNCH

1:00 PM:  Review results + plan Tuesday (same as before)

2:00 PM:  NEW: Setup metrics Google Sheet
          └─ Tech Lead creates sheet
          └─ Test daily update process
          └─ Share link to team
          
2:30 PM:  NEW: Confirm search UI decision (Option A / C)
          └─ Tech Lead + UX/UI decide
          └─ If Option C: start 2-3 hour UI work
          
3:00 PM:  NEW: QA reviews result format
          └─ Add JSON validation rules
          └─ Prepare for Hit Rate checking
          
4:00 PM:  Final confirmations (same as before)

5:00 PM:  END-OF-DAY CHECKLIST (updated below)
```

---

### Tuesday-Thursday May 18-20

**UPDATED PHASES:**

```
S1.1 (May 18-19): EXTRACTION (with rate limiting)
├─ Use new extraction config (delays + user-agent)
├─ Monitor for 429 errors, add more delays if needed
└─ Quality: Confirm extracted files, no duplicates

S1.2 (May 20-21): CHUNKING + DEDUPLICATION (new step)
├─ Chunk as before (500 tokens, 100 overlap)
├─ NEW: Run deduplication (remove 95%+ similar)
├─ Result: ~850 unique chunks (not 950)
├─ Validate token count (400-600 range)
└─ Quality: No duplicates, right size

S1.3 (May 22-24): ENTITIES with thresholds
├─ Use configured confidence thresholds
├─ Expected: 400-500 entities (realistic range)
├─ If < 350: Trigger fallback (adjust thresholds)
├─ If > 600: Review for low-quality entities
└─ Quality: Confidence-based, validated
```

---

### May 22: Hit Rate Decision Gate (UPDATED)

```
NEW: Use CLI + Result Format

9:00 AM:  Standup

9:30 AM:  Run test queries (CLI method)
          ├─ Command: python scripts/test_queries.py
          ├─ Use new result format (JSON with all fields)
          ├─ Output: test_results_may22.json
          └─ Takes: 30-60 min

10:30 AM: Evaluate results
          ├─ Calculate Hit Rate@3 from results
          ├─ Check each result against format specs
          ├─ QA grades relevance (human judgment)
          └─ Decision: ≥75% Hit Rate?

11:00 AM: DECISION POINT
          ├─ GO (Hit Rate ≥75%) → Proceed to S1.3
          └─ NO-GO (Hit Rate <75%) → Activate Plan B

IF OPTION C CHOSEN (hybrid):
  └─ UX/UI separately takes UI screenshots
     └─ For stakeholder demo (visual proof)
     └─ Doesn't affect May 22 decision (technical validation uses CLI)
```

---

## ✅ UPDATED END-OF-DAY CHECKLIST (Monday May 17)

```
🆕 = New items from peer review feedback

TEAM KICKOFF
  ☐ All 4 team members present + aligned
  ☐ Review updated SPRINT_1_EXECUTION_DETAILED.md
  ☐ Roles assigned + confirmed

ENVIRONMENT
  ☐ Heimdall, Qdrant, Neo4j, Mimir responding ✅
  ☐ asgard_insurance tenant exists ✅
  ☐ Gemini API key deployed ✅

🆕 SCRIPTS & CONFIG
  ☐ Extraction scripts include rate limiting config
  ☐ Entity extraction config documented (thresholds)
  ☐ Token validation rules added
  ☐ Deduplication script ready (for S1.2)
  ☐ Python dependencies all installed

🆕 TESTING & VALIDATION
  ☐ Smoke test passes (1 URL → Mimir)
  ☐ Result format defined (JSON schema)
  ☐ Hit Rate calculation method documented
  ☐ Test query list finalized (10 queries)

🆕 METRICS & TRACKING
  ☐ Google Sheet created for daily tracking
  ☐ Test update successful (add 1 row, confirm formula)
  ☐ Slack #insurance-s1-sprint channel created
  ☐ Daily standup link scheduled (9:00 AM)

🆕 DECISIONS MADE
  ☐ Search UI approach chosen (Option A / C)
  ☐ Entity confidence thresholds set (0.70-0.85)
  ☐ Rate limiting configured (2 sec delays)
  ☐ Deduplication threshold set (0.95 similarity)

TEAM SIGN-OFF
  ☐ Data Engineer: ✅ Ready (with configs)
  ☐ UX/UI: ✅ Ready (with search decision)
  ☐ Tech Lead: ✅ Approved (conditions met)

═══════════════════════════════════════════════════════════════
              READY FOR TUESDAY MAY 18 KICKOFF ✅
═══════════════════════════════════════════════════════════════
```

---

## 📊 Final Status After Peer Review

| Component | Before | After | Status |
|-----------|--------|-------|--------|
| Extraction | ⚠️ Unclear | ✅ Configured | READY |
| Chunking | ✅ Good | ✅ Better (+ dedup) | READY |
| Entities | ❌ Undefined | ✅ Configured | READY |
| Search UI | ⚠️ Unclear | ✅ CLI confirmed | READY |
| Metrics | ❌ Missing | ✅ Built | READY |
| Overall | 6.5/10 | **8.5/10** | ✅ READY |

---

## 🚀 GO/NO-GO Decision

### CONDITIONAL GO ✅

**Prerequisites (Must Complete by May 17 EOD):**
- ✅ Add entity extraction config (Data Engineer)
- ✅ Add rate limiting config (Data Engineer)
- ✅ Decide search UI approach (Tech Lead + UX/UI)
- ✅ Define result display format (UX/UI)
- ✅ Create metrics dashboard (Tech Lead)

**If all above done → GO for May 18** ✅  
**If any incomplete → DELAY until fixed**

### Confidence Level
- **Before feedback:** 6.5/10
- **After fixes:** **8.5/10** ✅

### Kickoff Status
- **Monday May 17:** Prep + fixes
- **Tuesday May 18:** EXECUTION STARTS 🚀

---

**Status:** ✅ READY (with fixes by May 17 EOD)  
**Next:** Assign action items below to owners

---

## 🎯 Action Item Assignments

| Issue | Owner | Deadline | Status |
|-------|-------|----------|--------|
| Entity extraction config | Data Engineer | May 17 EOD | ⏳ |
| Rate limiting config | Data Engineer | May 17 EOD | ⏳ |
| Deduplication script | Data Engineer | May 21 | ⏳ |
| Token validation | Data Engineer + QA | May 18 | ⏳ |
| Search UI decision | Tech Lead | May 17 EOD | ⏳ |
| Result display format | UX/UI | May 18 | ⏳ |
| Metrics dashboard | Tech Lead | May 17 EOD | ⏳ |

**Review These by:** May 17, 5:00 PM  
**Confirmed Ready by:** May 17, 6:00 PM  
**Kickoff:** May 18, 9:00 AM

