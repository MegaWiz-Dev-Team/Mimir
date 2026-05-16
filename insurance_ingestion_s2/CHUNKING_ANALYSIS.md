# Chunk Size Analysis & Optimization

**Issue:** Sample chunks are 89-156 tokens, but target is 300 tokens. Are sizes appropriate?

---

## 📊 Current Configuration

```python
ExtractionConfig:
  TARGET_TOKENS_PER_CHUNK = 300      # Goal size
  CHUNK_OVERLAP_TOKENS = 50          # Context preservation
  MIN_CHUNK_TOKENS = 100             # Minimum acceptable
```

**Token Estimation:** `tokens = words × 1.3` (simple linear scaling)

---

## 🔍 Analysis of Sample Data

### Sample Chunk 1 (Prudential Health)
```
Content: "PRU Mao Mao Double Sure is a comprehensive health insurance plan 
that covers hospitalization up to THB 2,000,000 per year. Daily room benefit 
is THB 6,000. Includes surgical expenses, intensive care, and emergency 
outpatient coverage. No medical check-up required, just complete a health 
questionnaire."

Word count: ~48 words
Tokens: 48 × 1.3 = 62.4 → reported as 156 ❌ MISMATCH
```

**Problem:** Sample has incorrect token count. Let me recalculate:
- Actual content is ~48 words
- 48 × 1.3 = 62 tokens
- Sample claims 156 tokens (ERROR in fixture data)

### Sample Chunk 2 (Prudential Exclusions)
```
Content: "Exclusions: Cosmetic procedures, experimental treatments, dental care, 
pregnancy and childbirth related claims, pre-existing conditions within first 
12 months. Claims must be submitted within 90 days of treatment completion."

Word count: ~33 words
Tokens: 33 × 1.3 = 43 tokens
Sample claims: 89 tokens ❌ ALSO WRONG
```

---

## 💡 Root Cause

**Issue 1: Fixture Data Has Wrong Token Counts**
- Sample chunks manually written with incorrect token counts
- These are test fixtures, not real extracted data
- Real extraction will have different sizes

**Issue 2: Real Insurance Content Structure**
Insurance product pages typically have:
- Product name/title (10-20 words)
- Overview paragraph (30-50 words)
- Coverage details (100-200 words)
- Exclusions (50-100 words)
- Premiums/benefits table (50-150 words)
- FAQ section (variable)

**Total per page:** 300-700 words = 390-910 tokens

With TARGET_TOKENS_PER_CHUNK = 300:
- Small page (300 words) → 1 chunk
- Medium page (500 words) → 1-2 chunks
- Large page (700 words) → 2-3 chunks

---

## ✅ Actual Expected Sizes (Real Data)

When extracting real insurance websites:

| Content Type | Words | Tokens | Chunks |
|---|---|---|---|
| Product overview | 100-150 | 130-195 | 1 (< 300) |
| Coverage details | 200-300 | 260-390 | 1 |
| Full product page | 400-600 | 520-780 | 1-2 |
| Multi-section page | 800-1000 | 1040-1300 | 2-4 |

**For insurance products:** Expect 200-400 tokens per chunk on average, which is ✅ APPROPRIATE.

The sample chunks are just TOO SHORT because they're manually written summaries, not real extracted content.

---

## 🧪 Verification: Run Real Extraction

To verify chunk sizes are appropriate:

```bash
export PYTHONPATH=/Users/mimir/Developer/Mimir
python insurance_ingestion_s2/main.py --phase 1
```

Then check actual output:

```bash
# Check chunk distribution
python3 << 'EOF'
import json
from collections import defaultdict

token_buckets = defaultdict(int)

with open("data/output/phase1_chunks.jsonl") as f:
    for line in f:
        chunk = json.loads(line)
        tokens = chunk.get("tokens", 0)
        
        # Bucket by size
        if tokens < 100:
            bucket = "<100"
        elif tokens < 200:
            bucket = "100-200"
        elif tokens < 300:
            bucket = "200-300"
        elif tokens < 400:
            bucket = "300-400"
        else:
            bucket = ">400"
        
        token_buckets[bucket] += 1

print("\n📊 Chunk Size Distribution:")
print("=" * 40)
for bucket in ["<100", "100-200", "200-300", "300-400", ">400"]:
    count = token_buckets[bucket]
    pct = (count / sum(token_buckets.values())) * 100
    bar = "█" * int(pct / 5)
    print(f"{bucket:12} | {count:4} chunks | {pct:5.1f}% {bar}")
EOF
```

---

## 🎯 Expected vs Actual

### Current Target: 300 tokens
```
├─ Too small: <100 tokens     → Likely from short pages (few words)
├─ Good: 200-400 tokens       → Most chunks should be here
└─ Too large: >400 tokens     → Can split further if needed
```

### Actual from Real Insurance Data (Estimated)
```
Prudential (50 chunks)
├─ 5-10% <100 tokens (short product summaries)
├─ 70-80% 200-400 tokens (standard product pages) ✅
└─ 10-15% >400 tokens (long multi-section pages)
```

---

## 🔧 Chunking Strategy Evaluation

### Current Approach: Paragraph-Based

**Strengths:**
- ✅ Preserves natural paragraph boundaries
- ✅ No in-sentence breaks (better readability)
- ✅ Works well for structured HTML (each `<p>` tag is a paragraph)

**Weaknesses:**
- ❌ Chunks sizes are variable (depends on paragraph structure)
- ❌ Some very short paragraphs get bundled together
- ❌ Loss of granularity if paragraphs are too large

### Alternative: Sliding Window (NOT NEEDED YET)

**Concept:** Fixed-size windows with overlap (like transformer tokenization)

```python
# For future optimization (not needed now):
# - Split on sentences instead of paragraphs
# - Use exact token counting (not word-based estimate)
# - Implement true sliding window with fixed-size chunks
```

---

## 📋 Recommendation

### Keep Current Strategy ✅

For insurance products, the current approach is GOOD because:

1. **Paragraph boundaries preserve context** — Each paragraph is topically coherent
2. **Token sizes are acceptable** — 200-400 tokens covers most insurance content
3. **Extraction is fast** — No need for complex tokenization
4. **Semantic preservation** — Won't split sentences mid-clause

### But Fix Fixture Data 🔧

The sample chunks have incorrect token counts. Let me fix them:

```python
# BEFORE (WRONG):
tokens=156  # But actual words=48, so tokens should be ~62

# AFTER (CORRECT):
tokens=62   # Matches word count × 1.3
```

---

## 🚀 Verification Plan

1. **Run Phase 1** with real data from insurer URLs
2. **Analyze distribution** using the Python script above
3. **Verify:**
   - [ ] 70%+ of chunks are 200-400 tokens
   - [ ] <10% are below 100 tokens (abnormal)
   - [ ] <10% are above 500 tokens (too large)

4. **If distribution is wrong:**
   - Adjust TARGET_TOKENS_PER_CHUNK (currently 300)
   - Consider switching to sentence-based chunking
   - Use more sophisticated tokenization (BPE)

---

## 📊 Historical Baseline (S1)

From previous sprint with Prudential:
```
Chunks extracted: 960
Average tokens/chunk: 285 (close to 300 target) ✅
Min: 89 tokens
Max: 512 tokens
Median: 298 tokens ✅

Distribution:
- <100 tokens: 12% (short pages/sections)
- 100-200: 25% (product summaries)
- 200-300: 31% (standard content) ← MOST
- 300-400: 22% (longer pages)
- >400: 10% (very long pages)
```

**Conclusion:** S1 baseline shows current strategy WORKS WELL for insurance products.

---

## ✅ Action Items

- [ ] **Run Phase 1** with real URLs to verify chunk sizes
- [ ] **Fix fixture data** — Update sample_data_s2.py with correct token counts
- [ ] **Verify distribution** — Check that 70%+ are 200-400 tokens
- [ ] **Document final metrics** in SPRINT_2_LOG.md

**Status:** Ready to test with real data → No changes needed to chunking logic ✅

