# RefGraph → Mimir Integration Options

**Question:** How do we integrate RefGraph into the Mimir Pipeline?

**Three Options:**

---

## Option A: Sequential Pipeline (Recommended) ⭐

**Architecture:**
```
Raw Data Files
    ↓
RefGraph CLI (standalone)
    • Extract entities
    • Deduplicate
    • Build graph
    ↓
MimirOutput (JSON/JSONL)
    ↓
Mimir Ingestion API
    • Embed entities
    • Index in Qdrant
    • Store in Neo4j
    ↓
Ready for Search
```

**What you build:**
- **refgraph-rs/** — Standalone Rust service (DONE - 27 tests)
- **New pipeline script** — Orchestrates RefGraph → Mimir
  ```bash
  # s1_pipeline.sh (or rust program)
  refgraph --input raw_data.jsonl --output consolidated.json
  curl -X POST http://localhost:8000/api/ingest \
    -H "Content-Type: application/json" \
    --data @consolidated.json
  ```

**Pros:**
- ✅ Clean separation of concerns
- ✅ RefGraph can be reused (medical, legal, finance domains)
- ✅ Each component independently testable
- ✅ Follows Rust-first principle (two Rust services)
- ✅ Easy to debug (each step produces files)
- ✅ Can run RefGraph on different machines
- ✅ Aligns with Asgard microservice pattern

**Cons:**
- ⚠️ Two separate deployments
- ⚠️ File I/O overhead (intermediate JSON)
- ⚠️ Manual orchestration (script or job scheduler)

**Timeline Impact:**
- May 19-28: Build RefGraph (SAME)
- May 29-June 1: Build orchestration script (2-3 hours)
- June 2: Ready for S1 execution

---

## Option B: RefGraph as Mimir Module

**Architecture:**
```
Raw Data Files
    ↓
Mimir Ingestion API
    ├─ Call RefGraph.consolidate() internally
    ├─ Embed consolidated entities
    ├─ Index in Qdrant
    └─ Store in Neo4j
    ↓
Ready for Search
```

**What you build:**
- **refgraph-rs/crates/refgraph-core** — Rust library
- **Mimir source** — Modified to use RefGraph
  ```rust
  // In Mimir ingestion handler
  let consolidated = RefGraph::new(config)?.consolidate(raw_chunks).await?;
  let output = MimirOutput::from_consolidated(&consolidated)?;
  ingest_to_vectors(&output).await?;
  ```

**Pros:**
- ✅ Single API endpoint
- ✅ No intermediate files
- ✅ Atomic operation (consolidate + ingest together)
- ✅ Simpler deployment (one service)
- ✅ Better performance (no file I/O)

**Cons:**
- ⚠️ Mimir repo becomes Rust-dependent
- ⚠️ Harder to reuse RefGraph elsewhere
- ⚠️ Tightly coupled (harder to change either)
- ⚠️ More complex Mimir codebase

**Timeline Impact:**
- May 19-28: Build RefGraph (SAME)
- May 29: Integrate RefGraph into Mimir (1-2 hours)
- June 1: Ready for S1 execution

---

## Option C: Separate RefGraph Service with API

**Architecture:**
```
Raw Data Files
    ↓
Mimir Pipeline Orchestrator (NEW SERVICE)
    ├─ POST /consolidate → RefGraph Service (8000 or separate port)
    ├─ GET /consolidated-output
    └─ POST /ingest → Mimir Ingestion API
    ↓
Ready for Search
```

**What you build:**
- **refgraph-rs/crates/refgraph-cli** — HTTP API instead of CLI
  ```rust
  // New API endpoint: POST /api/consolidate
  app.post("/api/consolidate", |chunks| {
    RefGraph::new(config)?.consolidate(chunks).await?
  })
  ```
- **New orchestrator service** — Coordinates RefGraph + Mimir
  ```rust
  // New service in Mimir repo
  pub async fn run_s1_pipeline(raw_data: Vec<RawChunk>) {
    let consolidated = call_refgraph_api(raw_data).await?;
    call_mimir_ingest_api(consolidated).await?;
  }
  ```

**Pros:**
- ✅ Clean microservice architecture
- ✅ RefGraph independently deployable
- ✅ Can scale RefGraph separately
- ✅ Easy to add other pipelines (medical, legal)
- ✅ No file I/O overhead

**Cons:**
- ⚠️ Most complex to build
- ⚠️ Requires HTTP/network coordination
- ⚠️ More moving parts to debug
- ⚠️ Adds latency (network calls)

**Timeline Impact:**
- May 19-28: Build RefGraph (SAME)
- May 29-June 1: Build RefGraph API + Orchestrator (4-5 hours)
- June 2: Ready for S1 execution (risk: integration testing needed)

---

## Recommendation: Option A (Sequential Pipeline)

**Why Option A is best for S1:**

1. **Aligns with Asgard principles**
   - Two independent Rust services (RefGraph + Mimir)
   - Each has clear responsibility
   - Follows microservice pattern

2. **Simplest to build & debug**
   - RefGraph produces JSON file (easy to inspect)
   - Can manually test each step
   - No hidden state between services

3. **Fastest timeline**
   - RefGraph already done (May 28)
   - Pipeline script = 2-3 hours (May 29)
   - Ready for June 2

4. **Future-proof**
   - RefGraph can be used for medical domain (same pipeline)
   - Can add new consolidation stages (compression, encryption)
   - Easy to parallelize or distribute

5. **Operational simplicity**
   - Deploy RefGraph independently
   - Deploy Mimir independently
   - Monitor each separately
   - Easy to roll back

---

## Implementation Plan (Option A)

### Step 1: RefGraph Output Format (Already Done ✅)
```rust
// refgraph-rs/src/output.rs
pub struct MimirOutput {
    pub entities: Vec<MimirEntity>,
    pub relationships: Vec<MimirRelationship>,
    pub metadata: ConsolidationMetadata,
}

// CLI produces:
refgraph --input data.jsonl --output consolidated.json
refgraph --input data.jsonl --jsonl consolidated.jsonl
```

### Step 2: Orchestration Script (May 29, ~2 hours)
```bash
#!/bin/bash
# s1_consolidate_and_ingest.sh

INPUT_FILE=$1  # raw insurance data
OUTPUT_FILE=$2 # consolidated entities

# Step 1: Consolidate with RefGraph
echo "Running RefGraph consolidation..."
/path/to/refgraph \
  --domain insurance \
  --input "$INPUT_FILE" \
  --jsonl "$OUTPUT_FILE"

if [ $? -ne 0 ]; then
  echo "RefGraph consolidation failed"
  exit 1
fi

# Step 2: Ingest into Mimir
echo "Ingesting into Mimir..."
curl -X POST http://localhost:8000/api/ingest \
  -H "Content-Type: application/jsonl" \
  --data-binary @"$OUTPUT_FILE"

echo "✅ Consolidation + Ingestion complete"
```

### Step 3: Monitoring & Validation (May 30-June 1)
```bash
# Check RefGraph output quality
cat consolidated.json | jq '.metadata'
# Should show: entity_count, relationship_count, average_confidence

# Verify Mimir ingestion
curl http://localhost:8000/api/stats | jq '.entity_count'
# Should match consolidated.metadata.entity_count

# Run test queries
curl -X POST http://localhost:8000/api/search \
  -d '{"query": "Critical Illness coverage", "top_k": 3}'
```

---

## Decision Matrix

| Factor | Option A | Option B | Option C |
|--------|----------|----------|----------|
| **Complexity** | Simple | Medium | Complex |
| **Timeline** | 2-3 hrs | 1-2 hrs | 4-5 hrs |
| **Reusability** | ✅ High | ⚠️ Medium | ✅ High |
| **Debuggability** | ✅ Easy | ⚠️ Medium | ⚠️ Hard |
| **Performance** | ⚠️ File I/O | ✅ Best | ⚠️ Network |
| **Deployment** | ✅ Simple | ⚠️ Complex | ⚠️ Complex |
| **Asgard-aligned** | ✅ Yes | ⚠️ Partial | ✅ Yes |

---

## What Happens in Each Option on June 2

### Option A: Sequential
```
9:00 AM: Start S1 execution
  $ s1_consolidate_and_ingest.sh prudential_raw_data.jsonl
  ↓ (2 min)
  RefGraph produces: consolidated.json (500+ entities)
  ↓ (1 min)
  Mimir ingest API processes and indexes
  ↓ (30 sec)
11:00 AM: Ready for search validation
  $ test_hit_rate.sh
```

### Option B: Integrated
```
9:00 AM: Start S1 execution
  $ curl -X POST http://localhost:8000/api/consolidate-and-ingest \
    -d @prudential_raw_data.jsonl
  ↓ (3 min - consolidation + ingestion atomic)
11:00 AM: Ready for search validation
  $ test_hit_rate.sh
```

### Option C: Microservices
```
9:00 AM: Start S1 execution
  $ orchestrate-s1-pipeline prudential_raw_data.jsonl
  ↓ Network call to RefGraph service
  ↓ Consolidation
  ↓ Network call to Mimir ingest
  ↓ (4-5 min total)
11:00 AM: Ready for search validation (after debugging network issues)
  $ test_hit_rate.sh
```

---

## My Recommendation

**Go with Option A (Sequential Pipeline):**

1. **You control the whole flow**
   - See intermediate outputs
   - Easy to debug
   - Can pause/resume at any step

2. **Fastest for S1**
   - RefGraph ready May 28
   - Pipeline script ready May 29
   - No integration risk

3. **Best for future**
   - RefGraph becomes standard tool
   - Can use for medical (S2), legal (S3), finance (S4)
   - Team can reuse without changes

4. **Asgard-aligned**
   - Two independent Rust services
   - Each deployable separately
   - Follows microservice pattern

---

## Questions to Answer

**Before I update SOLO_EXECUTION_PLAN.md with integration steps, which option do you prefer?**

A. Sequential Pipeline ← Recommended
B. RefGraph as Mimir Module
C. Separate RefGraph Service
D. Something different?

**Also:**
1. Do you already have Mimir ingestion API endpoints? (what's the POST format?)
2. Should RefGraph output go to files or memory?
3. Is performance critical or is correctness more important?

Once you decide, I'll update the plan with concrete integration code for May 29-30.
