# S1 Sprint: First Day Runbook (Monday May 17)
## Ready-to-Execute Agenda + Checklist

**Date:** Monday, May 17, 2026  
**Time:** 9:00 AM - 5:00 PM (8 hours)  
**Goal:** Team aligned + environment verified + smoke test passing  
**Success Criteria:** Can extract 10 chunks from 1 URL by EOD

---

## ⏰ Timeline (Copy-Paste Ready)

### 9:00 AM - 9:30 AM: Team Kickoff (Conf Room / Zoom)

**Attendees:** Tech lead, Data engineer, Backend engineer, QA  
**Facilitator:** Tech lead  
**Materials:** 
- SPRINT_1_EXECUTION_DETAILED.md (printed/shared)
- EVALUATION_FRAMEWORK_Insurance_Pipeline.md (printed/shared)

**Agenda:**
```
1. Welcome + sprint goal (2 min)
   "May 18-27: Extract 950 chunks, validate Hit Rate@3 ≥ 75%"

2. Role assignments confirmation (3 min)
   Data Eng: extraction + chunking
   Backend: integration + Mimir API
   QA: validation + test queries
   Tech Lead: daily standup + unblocking

3. Walk through SPRINT_1_EXECUTION_DETAILED.md (10 min)
   - Phase breakdown (1-5)
   - Acceptance criteria per phase
   - Decision gates (May 22: Hit Rate check)

4. Walk through EVALUATION_FRAMEWORK_Insurance_Pipeline.md (10 min)
   - How we measure success
   - What "75% Hit Rate" means
   - Why it matters (S2 gate)

5. Q&A + blockers (5 min)
   "Any questions or blockers BEFORE we start?"
```

**Output:** Everyone aligned, zero confusion

---

### 9:30 AM - 10:00 AM: Environment Readiness Check

**Owner:** Tech lead + Data engineer  
**Location:** Any terminal  
**Time:** 30 min

**Run these commands in order:**

```bash
# 1. Test Heimdall (embedding service)
echo "Testing Heimdall..."
curl -X POST http://heimdall:8001/embed \
  -H "Content-Type: application/json" \
  -d '{"texts": ["insurance product coverage"], "model": "bge-m3"}' \
  2>/dev/null | jq '.embeddings[0] | length' && echo "✅ Heimdall OK" || echo "❌ FAIL"

# 2. Test Qdrant (vector database)
echo "Testing Qdrant..."
curl -s http://qdrant:6333/health | jq '.status' && echo "✅ Qdrant OK" || echo "❌ FAIL"

# 3. Test Neo4j (knowledge graph)
echo "Testing Neo4j..."
export NEO4J_PASSWORD="<your-password>"
neo4j-shell -u neo4j -p $NEO4J_PASSWORD "RETURN 1;" 2>/dev/null \
  && echo "✅ Neo4j OK" || echo "❌ FAIL"

# 4. Test Mimir (RAG retrieval)
echo "Testing Mimir..."
curl -s http://mimir:8000/health | jq '.status' && echo "✅ Mimir OK" || echo "❌ FAIL"

# 5. Verify asgard_insurance tenant exists
echo "Testing Mimir tenant..."
curl -s http://mimir:8000/api/health | jq '.tenant' && echo "✅ Tenant OK" || echo "❌ FAIL"
```

**Expected output:**
```
✅ Heimdall OK
✅ Qdrant OK
✅ Neo4j OK
✅ Mimir OK
✅ Tenant OK
```

**If any FAIL:** Stop here, fix blocker with DevOps

---

### 10:00 AM - 10:30 AM: Git Setup + Script Deployment + Config

**Owner:** Data engineer  
**Location:** Terminal  
**Time:** 30 min

```bash
# 1. Create feature branch
cd /Users/mimir/Developer/Mimir/insurance_ingestion_s2
git checkout -b feature/insurance-s1-data-ingestion
git pull origin main

# 2. Verify extraction scripts exist
ls -la scripts/extract_insurance_sources.py
ls -la scripts/validate_chunks_with_skuggi.py
ls -la scripts/test_queries.py
ls -la scripts/extract_entities.py
ls -la scripts/deduplicate_chunks.py

# 3. Verify data directories
mkdir -p data/raw
mkdir -p data/extracted
mkdir -p data/output
chmod 755 data/*

# 4. Create .env for endpoints
cat > .env << 'EOF'
# Endpoints (K8s services)
HEIMDALL_URL=http://heimdall:8001
QDRANT_URL=http://qdrant:6333
NEO4J_URL=bolt://neo4j:7687
MIMIR_URL=http://mimir:8000

# Credentials
NEO4J_USER=neo4j
NEO4J_PASSWORD=<ask-devops>
GEMINI_API_KEY=<already-deployed>

# Tenant
MIMIR_TENANT=asgard_insurance

# Logging
LOG_LEVEL=DEBUG

# Rate limiting (from peer review)
EXTRACTION_DELAY_SECONDS=2.0
USER_AGENT_ROTATION=true
RESPECT_ROBOTS_TXT=true
REQUEST_TIMEOUT=30

# Entity extraction (from peer review)
ENTITY_CONFIDENCE_PRODUCT=0.85
ENTITY_CONFIDENCE_COVERAGE=0.80
ENTITY_CONFIDENCE_EXCLUSION=0.75
ENTITY_CONFIDENCE_CONDITION=0.70

# Tokenization (from peer review)
TOKENIZER_MODEL=cl100k_base
CHUNK_SIZE_TARGET=500
CHUNK_SIZE_MIN=400
CHUNK_SIZE_MAX=600

# Deduplication (from peer review)
DEDUP_THRESHOLD=0.95
DEDUP_ACTION=MERGE
EOF

# 5. ⭐ NEW: Verify configs are in scripts
echo "Checking entity extraction config..."
grep -q "ENTITY_CONFIDENCE" scripts/extract_entities.py && echo "✅ Entity config found" || echo "⚠️ Add config to extract_entities.py"

echo "Checking rate limiting config..."
grep -q "EXTRACTION_DELAY" scripts/extract_insurance_sources.py && echo "✅ Rate limiting found" || echo "⚠️ Add config to extract_insurance_sources.py"

echo "Checking deduplication config..."
grep -q "DEDUP_THRESHOLD" scripts/deduplicate_chunks.py && echo "✅ Dedup config found" || echo "⚠️ Add config to deduplicate_chunks.py"

# 6. Verify extraction can run
python3 scripts/extract_insurance_sources.py --help && echo "✅ Scripts ready"
python3 scripts/extract_entities.py --help && echo "✅ Entity extraction ready"
python3 scripts/deduplicate_chunks.py --help && echo "✅ Deduplication ready"
```

**Output:** All scripts deployed with NEW configs from peer review, .env configured, ready to test

---

### 10:30 AM - 11:30 AM: Smoke Test (1 URL → 10 Chunks)

**Owner:** Data engineer + Backend engineer  
**Location:** Terminal  
**Time:** 1 hour

**Goal:** Extract 1 URL, validate chunks, ingest to Mimir, confirm retrieval works

```bash
# STEP 0: ⭐ NEW - Validate configs are loaded (2 min)
echo "=== STEP 0: VALIDATE CONFIGS ==="
echo "Rate limiting delay: $EXTRACTION_DELAY_SECONDS sec"
echo "Entity confidence (product): $ENTITY_CONFIDENCE_PRODUCT"
echo "Chunk size target: $CHUNK_SIZE_TARGET tokens"
echo "Dedup threshold: $DEDUP_THRESHOLD"
# Expected: All configs show values

# STEP 1: Extract from 1 URL (10 min)
echo "=== STEP 1: EXTRACTION ==="
python3 scripts/extract_insurance_sources.py \
  --url "https://prudential.co.th/en/products/health/" \
  --output data/raw/health_overview.txt \
  --timeout 30 \
  --delay 2.0 \
  --respect-robots-txt

# Expected: Creates data/raw/health_overview.txt (~5-10 KB)
# Note: 2-second delay between requests prevents rate limiting
ls -lh data/raw/health_overview.txt

# STEP 2: Create chunks (10 min)
echo "=== STEP 2: CHUNKING ==="
python3 scripts/create_chunks.py \
  --input data/raw/health_overview.txt \
  --output data/output/smoke_test_chunks_raw.jsonl \
  --chunk_size 500 \
  --overlap 100

# Expected: Creates data/output/smoke_test_chunks_raw.jsonl with ~10-15 chunks
wc -l data/output/smoke_test_chunks_raw.jsonl
# Should show: ~10-15 chunks before dedup

# STEP 2b: ⭐ NEW - Deduplicate chunks (5 min)
echo "=== STEP 2b: DEDUPLICATION ==="
python3 scripts/deduplicate_chunks.py \
  --input data/output/smoke_test_chunks_raw.jsonl \
  --output data/output/smoke_test_chunks.jsonl \
  --threshold 0.95 \
  --action MERGE

# Expected: ~10-15 → ~8-10 unique chunks (removed 1-2 duplicates)
wc -l data/output/smoke_test_chunks.jsonl
# Should show: ~10 unique chunks (some removed as duplicates)

# STEP 3: ⭐ NEW - Token count validation (5 min)
echo "=== STEP 3: TOKEN VALIDATION ==="
python3 scripts/validate_tokens.py \
  --input data/output/smoke_test_chunks.jsonl \
  --min 400 \
  --max 600 \
  --target 500

# Expected: All chunks 400-600 tokens (target 500)
grep "token_count" data/output/smoke_test_chunks.jsonl | sort | tail -3

# STEP 4: Validate chunks with Skuggi (PII check) (10 min)
echo "=== STEP 4: VALIDATION (Skuggi) ==="
python3 scripts/validate_chunks_with_skuggi.py \
  --input data/output/smoke_test_chunks.jsonl \
  --output data/output/smoke_test_validated.jsonl

# Expected: All chunks pass Skuggi (0 PII detected)
grep "pii_score" data/output/smoke_test_validated.jsonl | head -3

# STEP 4: Ingest into Mimir (10 min)
echo "=== STEP 4: MIMIR INGESTION ==="
python3 scripts/ingest_to_mimir.py \
  --input data/output/smoke_test_validated.jsonl \
  --tenant asgard_insurance \
  --collection insurance_products_001

# Expected: 10 chunks ingested, 0 errors
# Output: "✅ Ingestion complete: 10/10 chunks"

# STEP 5: ⭐ NEW - Test retrieval with CLI (standard for May 22) (5 min)
echo "=== STEP 5: RETRIEVAL TEST (CLI Method) ==="
python3 scripts/test_queries.py \
  --query "health coverage benefits" \
  --tenant asgard_insurance \
  --top_k 3 \
  --domain insurance \
  --format json \
  --output data/output/test_result.json

# Expected output (JSON format per peer review):
# {
#   "query": "health coverage benefits",
#   "results": [
#     {
#       "rank": 1,
#       "title": "Critical Illness Coverage",
#       "relevance_score": 0.95,
#       "source_type": "official_pdf",
#       "pii_clearance": {"score": 0.0, "status": "SAFE"}
#     }
#   ]
# }

cat data/output/test_result.json | jq '.results[0] | {rank, relevance_score, source_type}'

# ⭐ NOTE: This is the OFFICIAL May 22 method (reliable, technical)
# If UX/UI has 2-3 hours: also take screenshots for stakeholder demo
```

**Success Criteria:**
```
✅ Extraction: 1 file created
✅ Chunking: 10 chunks in JSONL
✅ Validation: All chunks pass Skuggi (PII: 0)
✅ Ingestion: 10/10 chunks → Mimir
✅ Retrieval: Query returns results with relevance scores
```

**If ANY step fails:**
- Document error message
- Slack tech lead immediately
- Troubleshoot before 12:00 PM

---

### 11:30 AM - 12:30 PM: Troubleshooting + Fix (If Needed)

**Owner:** Tech lead + Data engineer + Backend engineer  
**Location:** Conference room  
**Time:** 1 hour (if needed)

**If smoke test failed, use this decision tree:**

```
❌ Step 1 (Extraction) failed?
  → URL issue: Check prudential.co.th is accessible
  → Script issue: Verify requests library installed
  → Fix: pip install requests beautifulsoup4

❌ Step 2 (Chunking) failed?
  → Input missing: Verify Step 1 created file
  → Script issue: Check Python 3.10+ installed
  → Fix: Adjust chunk_size parameter

❌ Step 3 (Skuggi) failed?
  → Skuggi not responding: Check K8s pod status
  → PII detected: Files are blocked (expected in some cases)
  → Fix: Ask QA to manually review flagged chunks

❌ Step 4 (Mimir ingest) failed?
  → Mimir not responding: Restart pod
  → Tenant not found: Create asgard_insurance tenant
  → Schema mismatch: Check metadata fields match
  → Fix: Follow CLAUDE.md troubleshooting section

❌ Step 5 (Retrieval) failed?
  → Chunks not indexed: Wait 30 sec, retry
  → Embeddings failed: Check Heimdall response
  → Query syntax: Verify query format matches API
  → Fix: Check Mimir logs for errors
```

**Output:** All blockers documented, resolved by 12:30 PM

---

### 12:30 PM - 1:00 PM: Lunch + Buffer

**Owner:** Everyone  
**Action:** Take break, lunch

---

### 1:00 PM - 2:00 PM: Review Results + Plan Tuesday

**Owner:** Tech lead + all team  
**Location:** Standup area  
**Time:** 1 hour

**Agenda:**
```
1. Review smoke test results (10 min)
   - Show working pipeline end-to-end
   - Celebrate success ✅
   - Document any workarounds

2. Walk through Phase 1 extraction script (20 min)
   - Will run on ALL 5 URLs on Tuesday
   - Show expected output format
   - Confirm team understands chunking strategy

3. Set daily standup rhythm (10 min)
   - Time: 9:00 AM daily (same Zoom link)
   - Duration: 15 min
   - Format: Progress + blockers + metrics

4. Prep for Tuesday kickoff (10 min)
   - Data engineer: Ready for full extraction
   - Backend: Ready for Mimir integration
   - QA: Ready for validation suite
   - Tech lead: Ready for daily tracking

5. Slack channel setup (10 min)
   - Create #insurance-s1-sprint
   - Post daily standup reminder
   - Configure bot for metrics updates
```

**Output:** Team ready for Tuesday launch

---

### 2:00 PM - 5:00 PM: Optional Deep Dives (As Needed)

**Owner:** By role  
**Optional activities:**

**Data Engineer:**
- [ ] Deep dive: data/consolidated/consolidated_products.jsonl structure
- [ ] Practice: Run extraction on multiple URLs
- [ ] Document: Any custom extraction logic needed

**Backend:**
- [ ] Deep dive: Mimir API schema + Neo4j integration
- [ ] Practice: Ingest test data, query it back
- [ ] Document: Any API changes needed

**QA:**
- [ ] Deep dive: Test query evaluation framework
- [ ] Practice: Run validation suite on 10 chunks
- [ ] Document: Any metrics calculations needed

**Tech Lead:**
- [ ] Set up tracking spreadsheet (metrics dashboard)
- [ ] Configure Slack reminders
- [ ] Create escalation decision tree

---

## 📋 End-of-Day Checklist (5:00 PM)

```
✅ TEAM KICKOFF
  ☐ All 4 team members present + aligned
  ☐ SPRINT_1_EXECUTION_DETAILED.md reviewed
  ☐ EVALUATION_FRAMEWORK_Insurance_Pipeline.md reviewed
  ☐ Peer review feedback discussed + understood
  ☐ Roles assigned (data eng, backend, QA, tech lead)

✅ ENVIRONMENT
  ☐ Heimdall responds ✅
  ☐ Qdrant responds ✅
  ☐ Neo4j responds ✅
  ☐ Mimir responds ✅
  ☐ asgard_insurance tenant exists ✅

✅ SCRIPTS (with peer review fixes)
  ☐ Git branch created: feature/insurance-s1-data-ingestion
  ☐ Extraction scripts deployed WITH rate limiting
  ☐ Entity extraction script with confidence thresholds ⭐
  ☐ Deduplication script deployed ⭐
  ☐ Token validation script deployed ⭐
  ☐ Validation scripts deployed
  ☐ .env file configured with all new variables ⭐
  ☐ Python dependencies installed (tiktoken for tokens, etc.)

✅ SMOKE TEST
  ☐ Config validation passed (all env vars loaded)
  ☐ 1 URL extracted successfully (with 2s rate limiting delays)
  ☐ 10-15 chunks created, 10 unique after dedup ⭐
  ☐ Token count validation passed (400-600 range) ⭐
  ☐ Skuggi validation passed (0 PII)
  ☐ Mimir ingestion successful
  ☐ Query retrieval works (CLI JSON format) ⭐
  ☐ Test result saved as JSON for May 22 format ⭐

✅ TEAM READY
  ☐ Daily standup scheduled (9:00 AM daily)
  ☐ Slack channel created (#insurance-s1-sprint)
  ☐ Tracking spreadsheet created (Google Sheet) ⭐
  ☐ Search UI approach decided (CLI + optional UI demo) ⭐
  ☐ Result format documented (JSON schema) ⭐
  ☐ Tomorrow's plan clear
  ☐ All peer review issues addressed
  ☐ No blockers preventing start

═════════════════════════════════════════════════════════════════
                    READY FOR TUESDAY KICKOFF ✅
                  S1.1 EXTRACTION STARTS 9:00 AM
═════════════════════════════════════════════════════════════════
```

---

## 🚨 If Anything Fails

**Escalation Path:**
```
Issue occurs → Tech lead notified immediately
↓
Tech lead diagnoses (15 min)
↓
If fixable by team → Fix it
   If not fixable → Escalate to DevOps/Platform
↓
Document the issue + solution
↓
Continue sprint (don't stop for blockers)
```

**Keep working on** what you CAN do while waiting for fixes.

---

## 📞 Support During the Day

**Tech Lead:** Available 9 AM - 5 PM, Slack #insurance-s1-sprint  
**DevOps:** On-call for infrastructure issues  
**Data Eng Lead:** Mentoring extraction script issues  

**Slack channel:** #insurance-s1-sprint (create today)

---

**Status:** ✅ Ready to execute  
**Next:** Tuesday 9:00 AM, S1.1 Extraction starts  
**Good luck!** 🚀

