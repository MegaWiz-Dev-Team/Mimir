# S1 Sprint: Phase-by-Phase Execution Checklist
## What Happens Each Day (May 18-27)

**Reference:** SPRINT_1_EXECUTION_DETAILED.md  
**Evaluation:** EVALUATION_FRAMEWORK_Insurance_Pipeline.md  

---

## 📅 WEEK 1: Extraction + Foundation (May 18-24)

### 🔵 PHASE S1.1: Extract Products (May 18-19)
**Owner:** Data Engineer  
**Goal:** 5 URLs → raw text files → 200+ chunks  
**Success Criterion:** phase1_raw.jsonl with 200 records

#### Tuesday May 18 (Day 1)
```
9:00 AM: Team standup
         ├─ Confirm extraction script ready
         ├─ Review 5 URLs to scrape
         └─ Q&A before starting

9:30 AM: START EXTRACTION
         ├─ URL 1: prudential.co.th/en/products/health/ → health_overview.txt
         ├─ Monitor: Page loads? Timeout? Rate limited?
         ├─ Log: Time taken, bytes downloaded, any errors
         └─ Git commit: "Extract health overview"

10:30 AM: URL 2: prudential.co.th/en/products/life/ → life_overview.txt
11:30 AM: URL 3: prudential.co.th/en/products/savings/ → savings_overview.txt
12:30 PM: LUNCH

1:30 PM: URL 4: prudential.co.th/en/products/investment/ → investment_overview.txt
2:30 PM: URL 5: prudential.co.th/about-us/ → about_us.txt

3:30 PM: Quality check
         ├─ Count total lines extracted: ≥5,000
         ├─ Check for duplicate content
         ├─ Look for navigation cruft (remove if found)
         └─ Git commit: "Complete S1.1 extraction"

4:30 PM: Update metrics
         ├─ Chunks extracted: ~200
         ├─ Total size: ? MB
         ├─ Errors: ? (should be 0)
         └─ Update tracking spreadsheet

5:00 PM: Standup summary
         └─ "Extracted 5 URLs (200 raw chunks), ready for chunking"
```

#### Wednesday May 19 (Day 2)
```
9:00 AM: Standup (review yesterday's extraction)

9:30 AM: De-duplicate & clean extracted content
         ├─ Look for repeated sections (boilerplate)
         ├─ Remove navigation menus
         ├─ Remove footer content
         └─ Keep: Product info, features, pricing, benefits

10:30 AM: Convert to phase1_raw.jsonl
          ├─ Format: {"source": "URL", "section": "title", "text": "..."}
          ├─ Keep source URL for audit trail
          └─ Check: ~200 records in JSONL

11:30 AM: Git commit: "Phase S1.1 complete: 200 raw records"

12:00 PM: READY FOR PHASE S1.2
          └─ Data Engineer → Backend/QA for chunking

📊 S1.1 Success Criteria:
  ☐ 5 URLs extracted
  ☐ 200 raw chunks created
  ☐ 0 duplicates
  ☐ phase1_raw.jsonl created
  ☐ All committed to git
```

---

### 🟠 PHASE S1.2: Transform to Chunks (May 20-21)
**Owner:** QA + Backend  
**Goal:** phase1_raw.jsonl → phase1_chunks.jsonl → validated chunks  
**Success Criterion:** 950 chunks validated with Skuggi (0 PII)

#### Thursday May 20 (Day 3)
```
9:00 AM: Standup (S1.1 complete, S1.2 starts)

9:30 AM: Chunking strategy review
         ├─ Chunk size: 500 tokens (~2000 chars)
         ├─ Overlap: 100 tokens (for context)
         ├─ Strategy: Paragraph boundaries respected
         └─ Expected: ~950 chunks from 200 raw

10:00 AM: Run chunking script
          ├─ Input: phase1_raw.jsonl (200 records)
          ├─ Script: scripts/create_chunks.py
          ├─ Output: phase1_chunks.jsonl (950 chunks)
          └─ Monitor: Progress output every 50 chunks

12:00 PM: LUNCH

1:00 PM: Validate chunk quality
         ├─ Sample 10 chunks manually
         ├─ Check: No incomplete sentences?
         ├─ Check: Context preserved in overlap?
         ├─ Check: Chunk size reasonable?
         └─ Document: Any quality issues

2:00 PM: Add metadata to chunks
         ├─ source_id: which URL
         ├─ section_number: which part of document
         ├─ product_type: health/life/savings/investment
         └─ language: "en" (all prudential.co.th)

3:00 PM: Git commit: "Phase S1.2: 950 chunks created"

4:00 PM: Pass to QA for Skuggi validation
         └─ "Ready for PII check"

5:00 PM: Standup summary
         └─ "950 chunks created, ready for PII validation"
```

#### Friday May 21 (Day 4)
```
9:00 AM: Standup (chunking complete, validation starts)

9:30 AM: Run Skuggi validation
         ├─ Input: phase1_chunks.jsonl (950 chunks)
         ├─ Script: scripts/validate_chunks_with_skuggi.py
         ├─ Output: phase1_validated.jsonl (with pii_score)
         └─ Monitor: Any PII detected?

11:00 AM: Review PII detections
          ├─ If pii_score = 0.0: ✅ PASS
          ├─ If pii_score > 0.9: ❌ BLOCK (manually review)
          ├─ If pii_score 0.3-0.9: ⚠️ MANUAL REVIEW (maybe email?)
          └─ Document: Which chunks flagged, why

12:00 PM: LUNCH

1:00 PM: Fix any PII issues
         ├─ Redact sensitive data (if found)
         ├─ Or remove problematic chunks
         ├─ Or mark with confidence score
         └─ Re-validate if changes made

2:00 PM: Add schema metadata to all chunks
         ├─ chunk_id: unique ID
         ├─ chunk_index: sequence number
         ├─ tokens: count (estimate or actual)
         ├─ metadata: {product_type, language, source_id, ...}
         └─ confidence: PII clearance score

3:00 PM: Final quality check
         ├─ Count total chunks: should be 950
         ├─ Check: All chunks have metadata?
         ├─ Check: All chunks pass schema validation?
         └─ Check: No PII detected?

4:00 PM: Git commit: "Phase S1.2 complete: 950 validated chunks"

5:00 PM: Standup summary
         └─ "S1.2 complete: 950 chunks (0 PII issues), ready for Entity extraction"

📊 S1.2 Success Criteria:
  ☐ 950 chunks created from 200 raw
  ☐ Chunk size ~500 tokens
  ☐ Overlap 100 tokens
  ☐ All metadata present
  ☐ Skuggi validation: 0 PII
  ☐ phase1_chunks.jsonl final
```

---

## 📅 WEEK 2: Entities + Integration (May 22-27)

### 🟡 PHASE S1.3: Entity Extraction + Neo4j (May 22-24)
**Owner:** Backend + QA  
**Goal:** Extract entities → Neo4j graph → relationship mapping  
**Success Criterion:** 500+ entities, 1000+ relationships in Neo4j

#### Saturday May 22 (Day 5) — DECISION GATE ⚠️
```
9:00 AM: CRITICAL STANDUP
         └─ Hit Rate validation decision today

9:30 AM: Run test queries (QA)
         ├─ Input: 10 test queries from EVALUATION_FRAMEWORK
         ├─ Script: scripts/test_queries.py
         ├─ Query against: phase1_chunks.jsonl (no Mimir yet)
         ├─ Measure: Hit Rate@3, MRR, NDCG
         └─ Output: test_results.json

10:30 AM: DECISION POINT
          ├─ Hit Rate ≥ 75%?
          │   ├─ YES → ✅ PROCEED (go to S1.3)
          │   └─ NO → 🔄 FALLBACK (Plan B activation)
          │
          └─ Plan B (if Hit Rate < 50%):
              ├─ Switch embedding model: BGE-M3 → Typhoon-Thai
              ├─ Re-embed all 950 chunks
              ├─ Re-run test queries
              ├─ Check Hit Rate again
              └─ Takes ~2 hours

12:00 PM: LUNCH + Decision made

1:00 PM: Tech Lead decision call
         ├─ Review results with team
         ├─ Communicate decision (GO / NO-GO)
         └─ Log decision in docs/decisions/decision_log.md

2:00 PM: If GO → BEGIN S1.3
         └─ Entity extraction starts

If NO-GO → ESCALATE
  └─ Contact product manager
  └─ Decide: Pivot or continue debugging?
  └─ Update timeline

---

ASSUMING GO (Hit Rate ≥ 75%):

2:30 PM: Entity extraction setup
         ├─ Script: scripts/extract_entities.py
         ├─ Input: phase1_chunks.jsonl (950 chunks)
         ├─ NER model: PyThaiNLP + custom Prudential terms
         ├─ Expected entities: Product, Coverage, Condition, Exclusion
         └─ Output: phase1_entities.jsonl

3:30 PM: Extract relationships
         ├─ Script: scripts/extract_relationships.py
         ├─ Relationship types: has_coverage, excludes, requires_age
         ├─ Graph edges: Entity → Entity
         └─ Output: phase1_edges.jsonl

4:30 PM: Upload to Neo4j
         ├─ Script: scripts/ingest_to_neo4j.py
         ├─ Create nodes from entities
         ├─ Create relationships from edges
         ├─ Index by entity_type, domain
         └─ Verify: cypher-shell MATCH (n) RETURN COUNT(n)

5:00 PM: Standup summary
         └─ "Hit Rate ✅, S1.3 started: Entity extraction in progress"

📊 May 22 Success Criteria:
  ☐ Test queries run
  ☐ Hit Rate measured
  ☐ Decision made (GO / NO-GO)
  ☐ If GO: S1.3 started
```

#### Sunday May 23 (Day 6)
```
[Continue S1.3 if started]

9:00 AM: Entity extraction progress
         ├─ Monitor: How many entities found?
         ├─ Check: Quality of extraction
         ├─ Sample: 20 chunks, manually verify entities
         └─ Adjust: NER thresholds if needed

1:00 PM: Relationship mapping
         ├─ Connect entities in chunks
         ├─ Build knowledge graph
         ├─ Check: Relationships make sense?
         └─ Example: "Product Mao Mao" → has_coverage → "Room charges"

3:00 PM: Neo4j ingestion
         ├─ Load entities as nodes
         ├─ Load relationships as edges
         ├─ Index for query performance
         └─ Verify: Query test works?

5:00 PM: Standup summary
         └─ "~400 entities found, relationships mapped, Neo4j loaded"
```

#### Monday May 24 (Day 7)
```
9:00 AM: Complete S1.3
         ├─ Finish any remaining entity extraction
         ├─ Validate Neo4j completeness
         └─ Q&A on relationships

1:00 PM: Git commit: "Phase S1.3 complete: 500+ entities, 1000+ relationships"

📊 S1.3 Success Criteria:
  ☐ ≥500 entities extracted
  ☐ ≥1000 relationships mapped
  ☐ All in Neo4j
  ☐ Queryable from Neo4j
  ☐ Query tests pass
```

---

### 🟢 PHASE S1.4: Mimir Ingestion (May 25-26)
**Owner:** Backend + QA  
**Goal:** Load chunks into Mimir → vectors → Qdrant → ready for search  
**Success Criterion:** 950 chunks searchable, Hit Rate validated

#### Tuesday May 25 (Day 8)
```
9:00 AM: Standup (S1.3 complete, S1.4 starts)

9:30 AM: Prepare chunks for Mimir
         ├─ Input: phase1_chunks.jsonl (950 chunks)
         ├─ Format validation (schema check)
         ├─ Metadata enrichment (add Neo4j context)
         └─ Output: phase1_mimir_ready.jsonl

10:30 AM: Generate embeddings
          ├─ Script: scripts/generate_embeddings.py
          ├─ Model: BGE-M3 (or Typhoon-Thai if Plan B)
          ├─ Batch size: 50 chunks at a time
          ├─ Output: embeddings.pkl
          └─ Monitor: Speed (should be ~10 chunks/sec)

12:00 PM: LUNCH

1:00 PM: Ingest to Qdrant
         ├─ Script: scripts/ingest_to_qdrant.py
         ├─ Collection: insurance_products_001
         ├─ Vectors: 950 (BGE-M3 = 1024 dimensions)
         ├─ Metadata: chunk_id, product_type, source_url
         └─ Verify: Qdrant collection health

2:30 PM: Ingest to Mimir
         ├─ Script: scripts/ingest_to_mimir.py
         ├─ Endpoint: POST /api/ingest
         ├─ Tenant: asgard_insurance
         ├─ Batch: 50 chunks at a time
         └─ Output: Mimir indexes chunks

3:30 PM: Verify ingestion
         ├─ Mimir UI: Search bar shows results?
         ├─ Query test: Simple query works?
         ├─ Latency: < 500ms p99?
         └─ Git commit: "Phase S1.4 started: chunks in Mimir"

5:00 PM: Standup summary
         └─ "Embeddings generated, Qdrant loaded, Mimir ingestion in progress"
```

#### Wednesday May 26 (Day 9)
```
9:00 AM: Standup (S1.4 continuation)

9:30 AM: Finish Mimir ingestion
         ├─ Ensure all 950 chunks loaded
         ├─ Check: Mimir counts = 950?
         ├─ Verify: All metadata present?
         └─ Fix: Any missing chunks?

10:30 AM: Run test queries
          ├─ Script: scripts/test_queries.py (with Mimir backend)
          ├─ Queries: All 10 from test suite
          ├─ Measure: Hit Rate@3, MRR, NDCG, latency
          ├─ Expected: Hit Rate ≥ 75%
          └─ Output: test_results_final.json

12:00 PM: LUNCH

1:00 PM: Analyze test results
         ├─ Review each query result
         ├─ Check: Relevance scores reasonable?
         ├─ Check: Source attribution correct?
         ├─ Log: Any issues for Phase S1.5

2:00 PM: Performance benchmarking
         ├─ Latency test: 100 concurrent queries
         ├─ Memory usage: Qdrant + Mimir
         ├─ CPU usage: Should be < 50%?
         └─ Document: Baseline metrics

3:00 PM: Git commit: "Phase S1.4 complete: 950 chunks live in Mimir"

5:00 PM: Standup summary
         └─ "All 950 chunks in Mimir, test queries validated, ready for final phase"

📊 S1.4 Success Criteria:
  ☐ 950 chunks ingested to Mimir
  ☐ Qdrant vectors loaded (1024-dim)
  ☐ All metadata preserved
  ☐ Search works end-to-end
  ☐ Hit Rate ≥ 75% (final validation)
  ☐ Latency < 500ms p99
```

---

### 🔵 PHASE S1.5: Final Validation + Buffer (May 27)
**Owner:** QA + Tech Lead  
**Goal:** Final acceptance tests → GO/NO-GO decision  
**Success Criterion:** All AC met, signed off

#### Thursday May 27 (Day 10) — FINAL GO/NO-GO
```
9:00 AM: FINAL STANDUP
         └─ Review all 5 phases

9:30 AM: Run comprehensive test suite
         ├─ Extraction: Did we get 950 chunks? ✅/❌
         ├─ Entity: Do we have 500+ entities? ✅/❌
         ├─ Graph: Do we have 1000+ relationships? ✅/❌
         ├─ Integration: Is Mimir working? ✅/❌
         ├─ Search: Hit Rate ≥ 75%? ✅/❌
         ├─ PII: Zero PII detected? ✅/❌
         └─ Performance: Latency < 500ms? ✅/❌

11:00 AM: Final decision meeting
          ├─ Tech Lead + QA review all metrics
          ├─ Decision: GO ✅ or NO-GO ❌?
          │
          ├─ If GO:
          │   ├─ Document: "Sprint S1 approved for production"
          │   ├─ Create tag: v1.0.0-insurance-s1
          │   ├─ Notify stakeholders
          │   └─ Plan S2 next
          │
          └─ If NO-GO:
              ├─ Identify gaps
              ├─ Create action items for S2
              ├─ Decide: Proceed anyway or extend sprint?
              └─ Document: Lessons learned

12:00 PM: LUNCH

1:00 PM: Documentation + Cleanup
         ├─ Update README.md with results
         ├─ Document: Known issues + workarounds
         ├─ Commit: "Sprint S1 complete [GO/NO-GO]"
         ├─ Create: Handoff doc for next team
         └─ Archive: Log files, metrics

2:00 PM: Team retrospective (15 min)
         ├─ What went well?
         ├─ What was hard?
         ├─ What would you change?
         └─ Document: Retrospective notes

3:00 PM: Celebration + Planning
         ├─ Celebrate sprint completion 🎉
         ├─ Review: What's next (S2)?
         ├─ Plan: Follow-up actions
         └─ Assign: S2 team members

5:00 PM: Final standup
         └─ "Sprint S1 COMPLETE [GO/NO-GO]: 950 chunks, Hit Rate [X%]"

📊 FINAL ACCEPTANCE CRITERIA:
  ☐ 950 chunks extracted
  ☐ 500+ entities identified
  ☐ 1000+ relationships mapped
  ☐ Zero PII detected
  ☐ Hit Rate@3 ≥ 75%
  ☐ Latency < 500ms p99
  ☐ All tests passing
  ☐ Documentation complete
  ☐ Code committed + tagged
  ☐ Team sign-off ✅
```

---

## 📊 Quick Reference: Daily Capacity

```
Data Engineer (1.5 FTE):
  S1.1 → S1.2 chunking support
  Daily task: Extraction, cleaning, quality checks

Backend (1.5 FTE):
  S1.2 → S1.3 entity extraction
  S1.4 → Mimir integration
  Daily task: NER, Neo4j, Mimir APIs

QA (0.75 FTE):
  S1.2 → Skuggi validation (PII)
  S1.5 → Test queries + Hit Rate measurement
  Daily task: Validation scripts, metrics, reporting

Tech Lead (0.25 FTE):
  Daily: Standup, unblocking, metrics tracking
```

---

## 🚨 Risk Mitigation

**What could go wrong?**

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|-----------|
| Hit Rate < 75% on May 22 | Medium | HIGH | Plan B: Switch to Typhoon model (2hr) |
| PII detected in chunks | Low | HIGH | Redact + re-validate (6-8 hrs) |
| Neo4j out of memory | Low | HIGH | Optimize queries or shard graph |
| Mimir ingestion slow | Medium | MEDIUM | Batch size tuning (2-4 hrs) |
| URL rate limiting | Low | MEDIUM | Add delays, rotate IPs (2 hrs) |
| Team member sick | Medium | LOW | Cross-train other members |

**Buffer days:** Sat May 23 + Sun May 24 available for overflow

---

## ✅ Print & Post

**Save this and print for your team room:**

```
┌──────────────────────────────────────────────┐
│       S1 SPRINT CALENDAR: MAY 18-27          │
├──────────────────────────────────────────────┤
│ May 18-19: S1.1 EXTRACTION (5 URLs)          │
│ May 20-21: S1.2 CHUNKING + VALIDATION (PII)  │
│ May 22:    🚨 HIT RATE DECISION GATE         │
│ May 23-24: S1.3 ENTITIES + NEO4J             │
│ May 25-26: S1.4 MIMIR INGESTION              │
│ May 27:    🏁 FINAL GO/NO-GO                  │
└──────────────────────────────────────────────┘
```

**Status:** ✅ Ready to execute  
**Next:** Print + review with team Monday  
**GO DATE:** Tuesday May 18, 9:00 AM 🚀

