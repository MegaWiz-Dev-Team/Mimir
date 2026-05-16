# May 29: Build S1 Orchestration Pipeline

**Goal:** Connect RefGraph → Mimir with orchestration script  
**Timeline:** 2-3 hours  
**Outcome:** Production-ready s1_consolidate_and_ingest.sh + test_hit_rate.sh  

---

## Architecture

```
Input: raw_data.jsonl (insurance chunks)
  ↓
RefGraph CLI (consolidate)
  • Extract entities
  • Deduplicate
  • Build graph
  ↓ Output: consolidated.json
    (500+ MimirEntity objects)
  ↓
Mimir Ingestion API
  • POST /api/ingest
  • Embed entities
  • Index in Qdrant
  • Store relationships
  ↓
Ready for search validation
```

---

## May 29 Morning (1 hour): Design & Preparation

### Step 1: Understand Mimir API (15 min)

**Find the ingestion endpoint:**
```bash
# Check Mimir API documentation
curl http://localhost:8000/api/docs
# or
curl http://localhost:8000/openapi.json | jq '.paths[] | keys' | grep -i ingest

# Common endpoints:
# POST /api/ingest (most likely)
# POST /api/consolidate-and-ingest (if combined)
# POST /api/bulk-ingest (for batch)
```

**Expected request format:**
```json
{
  "entities": [
    {
      "id": "ent_001",
      "text": "Critical Illness",
      "entity_type": "product",
      "confidence": 0.95,
      "sources": ["prudential.co.th"],
      "merged_from": [],
      "compressed_refs": [],
      "domain": "insurance"
    }
  ],
  "relationships": [...],
  "metadata": { ... }
}
```

**Or JSONL format (one per line):**
```jsonl
{"type":"metadata","data":{"domain":"insurance","timestamp":"2026-05-16T...","entity_count":500,...}}
{"type":"entity","data":{"id":"ent_001","text":"Critical Illness",...}}
{"type":"relationship","data":{"source_id":"ent_001","target_id":"rel_001",...}}
```

### Step 2: Verify RefGraph Output Format (15 min)

RefGraph already outputs MimirOutput. Verify:
```bash
# Test with sample data
cd /Users/mimir/Developer/Mimir/refgraph-rs
./target/release/refgraph --help

# Create sample test
echo '[{"content":"Critical Illness and Hospital coverage"}]' | \
  ./target/release/refgraph --domain insurance --input /dev/stdin --output /tmp/test.json

# Check format
cat /tmp/test.json | jq '.entities[0]'
# Should show: id, text, entity_type, confidence, sources, etc.
```

### Step 3: Plan Error Handling (15 min)

**What can go wrong?**
```
RefGraph fails:
  ✅ Check if binary exists
  ✅ Check if input file valid
  ✅ Check permissions
  → Exit with error, don't proceed

Output invalid:
  ✅ Check if output file created
  ✅ Validate JSON syntax
  → Exit if output malformed

Mimir API fails:
  ✅ Check connectivity (curl health)
  ✅ Check HTTP response (200 vs 500)
  ✅ Parse error message
  → Log error but don't block (can retry)
```

---

## May 29 Afternoon (2 hours): Build & Test

### Step 4: Write Orchestration Script (45 min)

**Use provided:** `s1_consolidate_and_ingest.sh` (already created)

Or write your own in Rust/Python if preferred. Key requirements:
1. Call RefGraph CLI with input file
2. Wait for consolidated.json
3. Validate JSON
4. POST to Mimir /api/ingest
5. Check HTTP response (200/201)
6. Log metrics (entity count, latency, etc.)

**Key components:**
```bash
#!/bin/bash

# 1. Pre-flight checks
- RefGraph binary exists
- Input file exists
- Mimir API accessible

# 2. Run RefGraph
$REFGRAPH_BIN --domain insurance --input $INPUT --jsonl $OUTPUT

# 3. Validate output
- Check file created
- Validate JSON (jq empty)
- Extract metadata

# 4. Ingest to Mimir
curl -X POST $MIMIR_API/api/ingest \
  -H "Content-Type: application/json" \
  --data-binary @$OUTPUT

# 5. Log results
- HTTP status code
- Entity count ingested
- Timing
```

### Step 5: Test with Sample Data (45 min)

**Create test data:**
```bash
# Sample insurance chunks (10 documents)
cat > /tmp/sample_insurance.jsonl << 'EOF'
{"content":"Critical Illness insurance covers serious illnesses like cancer, heart attack, and stroke. Premium $50/month. Coverage limit $100k."}
{"content":"Hospital treatment is covered with co-pay of $25 per visit. Excludes cosmetic procedures."}
{"content":"Life insurance provides $500k death benefit. Excludes suicide within 2 years."}
{"content":"ประกันสุขภาพครอบคลุมการรักษาในโรงพยาบาล ราคา 1500 บาท/เดือน"}
...
EOF
```

**Test the full pipeline:**
```bash
# Step 1: Make script executable
chmod +x s1_consolidate_and_ingest.sh
chmod +x test_hit_rate.sh

# Step 2: Run consolidation
./s1_consolidate_and_ingest.sh /tmp/sample_insurance.jsonl /tmp/test_output.json

# Expected output:
# [INFO] RefGraph consolidation complete
# [INFO] Consolidated: 8 entities, 12 relationships
# [INFO] Mimir ingestion complete (HTTP 200)
# [INFO] Pipeline Complete ✅

# Step 3: Verify output file
cat /tmp/test_output.json | jq '.metadata'
# Should show entity_count, relationship_count, average_confidence

# Step 4: Check Mimir received data
curl http://localhost:8000/api/stats | jq '.entity_count'
# Should be >= 8 (or whatever sample had)
```

### Step 6: Test Hit Rate Validation (15 min)

```bash
# Run validation script
./test_hit_rate.sh

# Expected output:
# [INFO] Running 10 test queries...
# [PASS] Query 1: Critical Illness coverage ... ✅ Got 3 results
# [PASS] Query 2: Health insurance plans ... ✅ Got 3 results
# ...
# [METRIC] Hit Rate@3: 80%
# ✅ PASS Hit Rate@3 >= 75%
```

---

## Checklist: May 29 End of Day

```
✅ Morning:
  ☐ Mimir /api/ingest endpoint identified
  ☐ RefGraph output format verified
  ☐ Error handling strategy planned

✅ Afternoon:
  ☐ s1_consolidate_and_ingest.sh created/tested
  ☐ test_hit_rate.sh created
  ☐ Sample data pipeline completes successfully
  ☐ Output JSON validates
  ☐ Mimir receives data (HTTP 200)
  ☐ Hit rate queries return results

✅ Git:
  ☐ git add s1_consolidate_and_ingest.sh
  ☐ git add test_hit_rate.sh
  ☐ git commit -m "feat: S1 orchestration pipeline (RefGraph → Mimir)"
  ☐ git log --oneline (verify commit)

✅ Documentation:
  ☐ Update .env with Mimir API endpoint
  ☐ Create README for orchestration
  ☐ Document Mimir API format used
```

---

## May 30-June 1: Data Preparation

```
May 30:
  ☐ Get real Prudential insurance data (20-50 documents)
  ☐ Convert to .jsonl format if needed
  ☐ Test pipeline: ./s1_consolidate_and_ingest.sh real_data.jsonl
  ☐ Verify ingestion completes

May 31:
  ☐ Get AXA insurance data (20-50 documents)
  ☐ Get Thai Health insurance data (20-50 documents)
  ☐ Test each independently
  ☐ Verify Hit Rate@3 with each dataset

June 1:
  ☐ Prepare final Go/No-Go decision
  ☐ If Hit Rate@3 >= 75%: Ready for execution ✅
  ☐ If 50-74%: Document tuning needed
  ☐ If <50%: Prepare Plan B (Typhoon fallback)
```

---

## June 2: Execution Day

```
9:00 AM:
  ./s1_consolidate_and_ingest.sh prudential_raw_data.jsonl
  └─ 2 minutes (RefGraph)
  └─ 1 minute (Mimir ingest)
  └─ 3 minutes total

11:00 AM:
  ./test_hit_rate.sh
  └─ 10 queries
  └─ Check Hit Rate@3 >= 75%

If yes: ✅ GO to full S2/S3 pipeline
If no: 🔄 Activate Plan B (switch embedding model, retry)
```

---

## Files to Create/Modify

```
Create (provided):
  ✅ s1_consolidate_and_ingest.sh (main orchestration)
  ✅ test_hit_rate.sh (validation)

Create (May 29):
  ☐ .env (Mimir API endpoint)
  ☐ s1_pipeline_README.md (how to run)

Modify (May 29):
  ☐ SOLO_EXECUTION_PLAN.md (already updated)
  ☐ S1_DAILY_LOG.md (log May 29 progress)

Keep from RefGraph:
  ✅ /target/release/refgraph (binary)
  ✅ Cargo.toml (dependencies)
  ✅ All source code
```

---

## Success Criteria (May 29, 5 PM)

```
✅ Scripts created:
   ☐ s1_consolidate_and_ingest.sh works
   ☐ test_hit_rate.sh works
   ☐ Both committed to git

✅ Testing:
   ☐ Sample data pipeline completes
   ☐ RefGraph output valid JSON
   ☐ Mimir receives POST (HTTP 200)
   ☐ Hit rate queries work (returns results)

✅ Documentation:
   ☐ Mimir API endpoint documented
   ☐ Pipeline flow documented
   ☐ Commands tested and working

✅ Ready for data loading May 30-31
```

---

## If You Get Stuck

### RefGraph binary not found
```bash
# Ensure release build done
cd refgraph-rs
cargo build --release
# Check binary exists
ls -la target/release/refgraph
```

### Mimir API endpoint wrong
```bash
# Find correct endpoint
curl http://localhost:8000/api/docs | jq .

# Try common endpoints
curl -X POST http://localhost:8000/api/ingest -d '{"test":true}'
curl -X POST http://localhost:8000/consolidate -d '{"test":true}'
curl -X POST http://localhost:8000/ingest -d '{"test":true}'
```

### JSON validation fails
```bash
# Check output format
./target/release/refgraph --help  # See exact format
# Compare with Mimir expectations
curl http://localhost:8000/api/schema | jq '.properties'
```

### Script permissions
```bash
chmod +x s1_consolidate_and_ingest.sh
chmod +x test_hit_rate.sh
```

---

**Timeline:** May 29, 2-3 hours  
**Outcome:** Production-ready orchestration  
**Next:** May 30-31 data preparation, June 2 execution  

Let's ship this! 🚀
