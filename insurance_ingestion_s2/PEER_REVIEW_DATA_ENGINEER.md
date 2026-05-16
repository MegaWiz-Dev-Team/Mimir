# PEER REVIEW: Data Engineer Perspective
## Review of S1 Sprint Plan

**Reviewer:** Senior Data Engineer (Extraction + Entity NER specialty)  
**Date:** May 16, 2026  
**Documents Reviewed:** S1_FIRST_DAY_RUNBOOK.md, S1_PHASE_BY_PHASE_CHECKLIST.md  
**Time:** 15 minutes  

---

## ✅ What Looks Good

1. **5-URL extraction is realistic** ✅
   - Each URL should take ~15-20 min (200 lines text)
   - 2-day timeline (May 18-19) is achievable
   - Confidence: HIGH

2. **Chunking strategy is sound** ✅
   - 500 tokens per chunk is standard
   - 100-token overlap prevents context loss
   - 950 chunks from 200 raw records = realistic ratio
   - Can do this in 1 day (May 20)

3. **Skuggi validation flow makes sense** ✅
   - PII detection before ingestion is correct
   - 0 PII target is achievable for Prudential (no personal data)

4. **Entity extraction timeline realistic** ✅
   - 3-4 entities per chunk = ~300-400 expected
   - May 22-24 for extraction + Neo4j = doable

5. **Neo4j ingestion approach solid** ✅
   - Relationship types clear (has_coverage, excludes, etc.)
   - Product-entity links straightforward

---

## ⚠️ Issues Found

### Issue 1: Missing PyThaiNLP Configuration

**Problem:**  
Phase S1.3 mentions "PyThaiNLP + custom Prudential terms" but doesn't specify:
- Which NER model? (named_entity vs pos_tag?)
- Custom entity dictionary needed?
- Thai vs English NER?
- Threshold confidence for entity acceptance?

**Impact:** MEDIUM  
- May delay S1.3 start if not ready
- Could miss 10-20% of entities if misconfigured

**Recommendation:**
```python
# Add to S1_FIRST_DAY_RUNBOOK.md (Monday prep):

☐ Prepare PyThaiNLP:
  - Model: pythainlp.NER (DEFAULT: thai_ner)
  - Dictionary: Add Prudential product names
  - Confidence threshold: 0.8 (conservative)
  - Language split: 
    * English chunks → spacy-en-core
    * Thai chunks → pythainlp.ner
```

**Fix Required:** Yes, before S1.3 starts  
**Owner:** Data Engineer  
**Timeline:** Add to Monday prep checklist

---

### Issue 2: Rate Limiting Not Addressed

**Problem:**  
Prudential website might rate-limit scraping.
- No delay between requests specified
- No user-agent rotation
- No robots.txt check

**Impact:** MEDIUM  
- Could get 429 errors after 50-100 requests
- Would need to re-scrape later
- Wasting time with retries

**Recommendation:**
```python
# Add to extraction script config:

EXTRACTION_CONFIG = {
    "delays_between_requests": 2.0,  # seconds
    "user_agent_rotation": True,
    "respect_robots_txt": True,
    "max_retries": 3,
    "timeout": 30,
    "headers": {
        "Accept-Language": "en-US,en;q=0.9,th;q=0.8",
        "User-Agent": "Mozilla/5.0 (Insurance Data Extractor v1.0)"
    }
}
```

**Fix Required:** Yes, before May 18  
**Owner:** Data Engineer  
**Timeline:** Update extraction script by Monday

---

### Issue 3: Duplicate Content Detection Missing

**Problem:**  
Prudential website probably has repeated content:
- Same coverage info on multiple product pages
- Same exclusions repeated
- Navigation boilerplate

**Impact:** MEDIUM  
- 950 chunks might include 10-15% duplicates
- Inflates chunk count artificially
- Wastes Mimir embedding capacity
- Skews Hit Rate (duplicate matches aren't novel)

**Recommendation:**
```python
# Add deduplication step in S1.2:

DEDUP_CONFIG = {
    "method": "jaccard_similarity",
    "threshold": 0.95,  # 95% similar = duplicate
    "action": "MERGE",  # merge duplicates into 1 chunk
    "keep_sources": True  # preserve all source URLs
}

# Result: 950 → ~850-900 unique chunks
# Quality improvement: Higher Hit Rate (no noise)
```

**Fix Required:** Yes, add to S1.2 phase  
**Owner:** Data Engineer  
**Timeline:** Implement before May 21

---

### Issue 4: Token Counting Not Specified

**Problem:**  
Plan says "500 tokens per chunk" but doesn't specify:
- Which tokenizer? (tiktoken? transformers.tokenize? simple word count?)
- Does 500 include overlap in the count?
- How to validate chunk size?

**Impact:** LOW  
- Chunks might be 300-700 tokens (varies by tokenizer)
- Could affect embedding quality
- No way to verify "500 tokens"

**Recommendation:**
```python
# Add to EVALUATION_FRAMEWORK:

CHUNK_VALIDATION = {
    "tokenizer": "tiktoken",  # Use GPT-3 tokenizer (standard)
    "model": "cl100k_base",
    "chunk_size_target": 500,
    "chunk_size_acceptable_range": (400, 600),  # Allow variance
    "overlap_tokens": 100,
    "validation_check": "Count tokens before ingestion"
}
```

**Fix Required:** Yes, clarify in evaluation doc  
**Owner:** Data Engineer + QA  
**Timeline:** Document by Monday

---

### Issue 5: Entity Extraction Confidence Threshold Not Set

**Problem:**  
NER produces confidence scores but threshold not defined:
- Include entities with confidence > 0.5? 0.7? 0.9?
- Too low = noisy (wrong entities)
- Too high = missing real entities
- No guidance in plan

**Impact:** MEDIUM  
- Could drastically change entity count (300 vs 500 vs 700)
- Affects Neo4j graph quality
- May cause Hit Rate issues

**Recommendation:**
```python
# Add to S1.3 phase checklist:

ENTITY_EXTRACTION_CONFIG = {
    "models": {
        "english_chunks": "spacy-en-core-web-sm",
        "thai_chunks": "pythainlp.ner"
    },
    "confidence_thresholds": {
        "product": 0.85,      # Product names must be clear
        "coverage": 0.80,     # Coverage terms slightly looser
        "exclusion": 0.75,    # Exclusions important but harder to detect
        "condition": 0.70     # Age/condition requirements looser
    },
    "expected_entities": "400-500",
    "manual_review_sample": 50  # QA reviews sample
}
```

**Fix Required:** Yes, add to S1.3 plan  
**Owner:** Data Engineer (with QA validation)  
**Timeline:** Define by May 22

---

### Issue 6: No Rollback Plan if S1.3 Fails

**Problem:**  
If entity extraction quality is bad (< 400 entities), no recovery path:
- Continue with low-quality graph?
- Redo extraction with different parameters?
- Manual entity tagging (time-consuming)?

**Impact:** MEDIUM  
- If S1.3 produces bad entities, whole graph is compromised
- No decision on what to do

**Recommendation:**
```python
# Add to S1.3 phase:

ENTITY_EXTRACTION_FALLBACK = {
    "trigger": "Entities < 350 OR confidence avg < 0.72",
    "option_A": "Manual review + adjust confidence thresholds (4-6 hours)",
    "option_B": "Use simpler regex-based extraction (2 hours, lower quality)",
    "option_C": "Skip S1.3, proceed with chunks only (skip Neo4j)",
    "recommendation": "Option A (best quality)"
}
```

**Fix Required:** Yes, add contingency  
**Owner:** Tech Lead  
**Timeline:** Document by May 21

---

## 🟢 Confidence Assessment

| Task | Feasibility | Timeline | Risk |
|------|-------------|----------|------|
| S1.1 (Extract) | HIGH ✅ | 2 days = GOOD | LOW |
| S1.2 (Chunk) | HIGH ✅ | 1 day = GOOD | LOW |
| S1.3 (Entity) | MEDIUM ⚠️ | 3 days = OK | MEDIUM |
| S1.4 (Ingest) | HIGH ✅ | 2 days = GOOD | LOW |

**Overall:** 7/10 - Plan is solid, needs entity extraction clarification

---

## 📋 Sign-Off

| Criterion | Status |
|-----------|--------|
| Extraction feasible? | ✅ YES |
| Timeline realistic? | ✅ YES (with fixes) |
| Entity extraction unclear? | ⚠️ NEEDS CLARIFICATION |
| Rate limiting handled? | ❌ MISSING |
| Deduplication strategy? | ❌ MISSING |
| Fallback plans? | ❌ MISSING |

**OVERALL READINESS:** ⚠️ **NEEDS PREP**

**What needs to happen:**
1. ✅ Issue 1: Add PyThaiNLP config (Monday)
2. ✅ Issue 2: Add rate limiting (Monday)
3. ✅ Issue 3: Add dedup step (by May 21)
4. ✅ Issue 4: Add token validation (by Monday)
5. ✅ Issue 5: Add entity thresholds (by May 22)
6. ✅ Issue 6: Add fallback plan (by May 21)

**Can we start May 18?** 
- ✅ YES, if Issues 1-2 are fixed Monday morning
- ⚠️ Issues 3-6 can be done during sprint

**Confidence (1-10):** 7/10  
- Could be 9/10 if all 6 issues fixed

---

## Feedback from Data Engineer

**Name:** [Data Engineer Lead]  
**Status:** ⚠️ **NEEDS PREP** (can start May 18 with fixes)  
**By When Ready:** May 17 EOD (address Issues 1-2)  
**Confidence:** 7/10 (will be 9/10 with fixes)

**Signature:** _________________ **Date:** May 16, 2026

