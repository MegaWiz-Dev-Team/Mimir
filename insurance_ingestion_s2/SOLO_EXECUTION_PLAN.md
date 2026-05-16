# Solo Execution Plan - S1 Insurance Sprint
## RefGraph Rust Implementation (May 19-28)

**Status:** Ready for solo execution  
**Timeline:** May 19 (Day 1) → May 28 (Day 8)  
**Executor:** Solo developer (you)  
**Confidence:** 9.5/10

---

## Why This Works Solo

✅ **RefGraph is self-contained** - No external team dependencies  
✅ **Code is pre-designed** - All 9 modules with clear interfaces  
✅ **TDD provides guidance** - Tests tell you exactly what to build  
✅ **Work parallelizable** - Some tasks can overlap (dashboard while testing)  
✅ **8 days is achievable** - Conservative estimate with 50% headroom  

---

## Timeline Overview

```
May 16 (Today):
  ✅ RefGraph Rust complete (1,412 lines, tested)
  ✅ TDD documentation ready (2,325 lines)
  ✅ 15 copy-paste tests prepared

May 19-28 (8 working days):
  Phase 2: Core Implementation (Days 1-5, May 19-24)
    Day 1: Entity extraction (extract.rs)
    Day 2: Deduplication refinement (dedup.rs)
    Day 3: Graph relationships (graph.rs)
    Day 4: Consolidation pipeline (lib.rs integration)
    Day 5: Neo4j integration (graph.rs + Neo4j)

  Phase 3: Integration & Testing (Days 6-8, May 25-28)
    Day 6: Mimir output formatting (mimir.rs)
    Day 7: E2E testing + performance benchmarks
    Day 8: Code review + documentation polish

May 28 (5 PM):
  ✅ RefGraph production-ready
  ✅ All tests passing (24 unit + 10 integration)
  ✅ 83% code coverage
  ✅ Ready for June 2 S1 execution

June 2-11 (S1 Execution):
  ☐ Load real data (Prudential, AXA, Thai Health)
  ☐ Run consolidation pipeline
  ☐ Validate Hit Rate@3 ≥ 75%

June 12:
  ☐ Go/No-Go decision
```

---

## Daily Breakdown (May 19-28)

### Day 1 (May 19): Entity Extraction
**Goal:** Extract features working, 15 tests passing

```
Morning (2-3 hours):
  ☐ Read extract.rs (207 lines) - understand current implementation
  ☐ Read EXAMPLE_TDD_EXTRACT.md - copy 15 test functions
  ☐ Paste tests into src/extract.rs under #[cfg(test)] mod tests

Afternoon (2-3 hours):
  ☐ Run: cargo test --lib extract::tests
  ☐ Watch all 15 tests FAIL (RED phase)
  ☐ Implement extract.rs functions to pass tests (GREEN phase)
  ☐ Refactor for clarity (REFACTOR phase)
  ☐ Final: cargo test extract::tests → all ✅

Success: 15 tests passing, zero failures
Commit: "feat: entity extraction with TDD (15/15 tests)"
```

**What you're testing:**
- Entity pattern detection (products, coverages, exclusions)
- Language detection (English/Thai)
- Confidence scoring (0.0-1.0)
- Error cases (empty input, invalid patterns)

---

### Day 2 (May 20): Deduplication Refinement
**Goal:** Jaccard similarity tested, edge cases handled

```
Morning (2 hours):
  ☐ Read dedup.rs (198 lines) - Jaccard similarity algorithm
  ☐ Identify edge cases:
    - Empty strings
    - Single token
    - 100% identical
    - High but not perfect similarity

Afternoon (3-4 hours):
  ☐ Write 8-10 edge case tests
  ☐ Run tests → measure coverage
  ☐ Optimize dedup if slow (should be <1ms per pair)
  ☐ Run with 1000 test entities → benchmark performance
  ☐ Commit: "perf: deduplication optimization with edge cases"

Success: 20+ total tests, <100ms for 1000 entities
```

**What you're testing:**
- Jaccard similarity correctness (0.0 to 1.0)
- Tokenization consistency
- Confidence merging logic
- Source consolidation

---

### Day 3 (May 21): Graph Relationships
**Goal:** Semantic graph working, relationships inferred

```
Morning (2-3 hours):
  ☐ Read graph.rs (269 lines) - SemanticGraph structure
  ☐ Review relationship types in types.rs
  ☐ Plan inference rules:
    - Product + Coverage → HAS_COVERAGE
    - Product + Exclusion → HAS_EXCLUSION
    - etc.

Afternoon (3-4 hours):
  ☐ Write 12-15 relationship tests
  ☐ Implement build_relationships() method
  ☐ Test Neo4j compatibility (format/types)
  ☐ Commit: "feat: semantic graph with relationship inference"

Success: 25+ tests passing, graph statistics accurate
```

**What you're testing:**
- add_entity() / add_relationship() methods
- Relationship inference logic
- Neo4j property/type compatibility
- Graph statistics (entity/relationship count)

---

### Day 4 (May 22): Consolidation Pipeline
**Goal:** Full pipeline coordinator working end-to-end

```
Morning (2 hours):
  ☐ Review lib.rs (118 lines) - RefGraph struct and consolidate() method
  ☐ Design integration test:
    - Input: 5 raw chunks (text)
    - Expected: 2-3 consolidated entities, relationships
    - Output: MimirOutput JSON format

Afternoon (3-4 hours):
  ☐ Write 3-5 integration tests using consolidate()
  ☐ Test error recovery (malformed input, missing fields)
  ☐ Test output formats (JSON + JSONL)
  ☐ Commit: "feat: consolidation pipeline (E2E tested)"

Success: Integration tests pass, pipeline coordinates all modules
```

**What you're testing:**
- Full RefGraph.consolidate() flow
- extract → dedup → graph pipeline
- Error handling and recovery
- JSON/JSONL serialization

---

### Day 5 (May 23-24): Neo4j Integration
**Goal:** Neo4j client ready, relationships uploadable

```
May 23 Morning (2-3 hours):
  ☐ Verify Neo4j running on localhost:7687
  ☐ Test basic neo4rs connection
  ☐ Write 2-3 connection tests

May 23 Afternoon (2-3 hours):
  ☐ Implement upload_to_neo4j() method
  ☐ Test relationship bulk insert
  ☐ Test transaction handling
  ☐ Verify relationship structure in Neo4j

May 24 Full Day (4-5 hours):
  ☐ Performance test (1000+ relationships)
  ☐ Add retry logic for transient failures
  ☐ Test with real insurance data sample
  ☐ Commit: "feat: Neo4j integration with TDD"

Success: 1000+ relationships uploadable in <5 seconds
```

**What you're testing:**
- Neo4j connection and auth
- Cypher query correctness
- Bulk insert performance
- Error recovery (network failures)

---

### Day 6 (May 25): Mimir Output Formatting
**Goal:** RAG ingestion format complete

```
Morning (2-3 hours):
  ☐ Read mimir.rs (254 lines) - Output formatting
  ☐ Verify serialization:
    - MimirEntity (name, type, confidence, sources)
    - ConsolidationMetadata (entity_count, relationship_count, coverage)
    - JSON structure matches Mimir expectations

Afternoon (2-3 hours):
  ☐ Write 8-10 serialization tests
  ☐ Test JSON/JSONL correctness
  ☐ Verify Unicode handling (Thai text)
  ☐ Commit: "feat: Mimir output formatting tested"

Success: JSON/JSONL verified for Mimir ingestion
```

---

### Day 7 (May 26): E2E Testing & Performance
**Goal:** Full pipeline validated, benchmarks recorded

```
Morning (3-4 hours):
  ☐ Write comprehensive E2E test:
    - Input: 1000+ chunks (real or generated)
    - Run through full RefGraph pipeline
    - Measure: time, memory, entity count, relationships
  ☐ Run benchmarks:
    - Extract: per-entity time
    - Dedup: per-pair comparison time
    - Graph: relationship inference time

Afternoon (2-3 hours):
  ☐ Performance optimization if needed:
    - Parallel dedup using rayon
    - Index relationships for fast lookup
  ☐ Memory profiling (ensure <500MB for 10K entities)
  ☐ Commit: "perf: E2E benchmarks + optimization"

Success: <2 seconds for 1000 chunks, <500MB memory
```

---

### Day 8 (May 27-28): Heimdall Integration + Final Review
**Goal:** LLM-enhanced extraction + production-ready release

```
May 27 Morning (2-3 hours):
  ☐ Integrate Heimdall (LLM gateway):
    - Add Heimdall endpoint to manifest config
    - Create extract_with_heimdall() method
    - Use Heimdall to enhance entity confidence scoring
    - Write 5 integration tests with Heimdall

May 27 Afternoon (2 hours):
  ☐ Integrate Sága/Laminar (if needed for any TTS):
    - Document Sága endpoints
    - Plan for future enhancement (optional May 28)
  ☐ Code review (all 9 modules):
    - Check error handling
    - Verify test coverage (aim for 83%)
    - Look for clippy warnings

May 28 Morning (2-3 hours):
  ☐ Final cleanup:
    - cargo fmt (format code)
    - cargo clippy (lint)
    - cargo test (all tests pass)
  ☐ Final Heimdall integration test:
    - Test extract + Heimdall enhancement pipeline
    - Verify confidence improvements
    - Measure latency (should be <500ms per entity)

May 28 Afternoon (1-2 hours):
  ☐ Final git commit: "feat: Heimdall LLM integration + docs: production release v1.0.0"
  ☐ Tag release: git tag v1.0.0
  ☐ Generate coverage report: cargo tarpaulin --out Html
  ☐ Verify all tests still pass
  ☐ Create RELEASE_NOTES.md

Success: Heimdall integrated, all tests passing, zero warnings, 83% coverage
```

---

## Heimdall Integration (May 27-28)

### What is Heimdall?

Heimdall is Asgard's LLM Gateway. It provides:
- Text classification & understanding (via local MLX models)
- Entity confidence enhancement (improve extraction confidence)
- Semantic validation (verify extracted entities make sense)
- Support for local (fast, free) + cloud (more capable) LLMs

### How to Integrate into extract.rs

**Step 1: Add Heimdall config to manifest.rs**
```rust
pub struct ManifestConfig {
    // ... existing fields ...
    pub heimdall_enabled: bool,        // Enable LLM enhancement
    pub heimdall_uri: String,          // Heimdall endpoint (http://localhost:8001)
    pub heimdall_model: String,        // Model to use (bge-m3, typhoon, etc)
    pub confidence_threshold: f32,     // Min confidence (0.0-1.0)
}
```

**Step 2: Add Heimdall client to extract.rs**
```rust
impl EntityExtractor {
    async fn enhance_with_heimdall(
        &self,
        entity: &Entity,
        heimdall_uri: &str,
    ) -> Result<Entity> {
        // Call Heimdall to validate entity + boost confidence
        let response = reqwest::Client::new()
            .post(format!("{}/api/classify", heimdall_uri))
            .json(&entity.text)
            .send()
            .await?;
        
        let confidence_boost = response.json::<f32>().await?;
        Ok(Entity {
            confidence: (entity.confidence + confidence_boost) / 2.0,
            ..entity.clone()
        })
    }
}
```

**Step 3: Update extract() method**
```rust
pub async fn extract(&self, text: &str) -> Result<Vec<Entity>> {
    // Pattern-based extraction (fast)
    let mut entities = self.extract_patterns(text)?;
    
    // Optional: Heimdall enhancement (if enabled + available)
    if self.config.heimdall_enabled {
        for entity in &mut entities {
            if let Ok(enhanced) = self.enhance_with_heimdall(entity, &self.config.heimdall_uri).await {
                *entity = enhanced;
            }
        }
    }
    
    Ok(entities)
}
```

**Step 4: Test with Heimdall**
```rust
#[tokio::test]
async fn test_extract_with_heimdall_enhancement() {
    let mut config = ManifestConfig::insurance();
    config.heimdall_enabled = true;
    config.heimdall_uri = "http://localhost:8001".to_string();
    
    let extractor = EntityExtractor::new_with_config(config);
    let entities = extractor.extract("Critical Illness coverage").await.unwrap();
    
    // Confidence should be boosted by Heimdall
    assert!(entities[0].confidence > 0.85);
}
```

### Heimdall Benefits

- ✅ Higher confidence scores (semantic validation)
- ✅ Better handling of domain language (insurance terms)
- ✅ Can fall back to pattern-based if Heimdall unavailable
- ✅ No blocking dependency (if Heimdall down, still works)
- ✅ Improves Hit Rate@3 (better entity quality)

### Timeline

- **May 19-27:** Pattern-based extraction (critical path)
- **May 27 Optional:** Add Heimdall enhancement (if time allows)
- **May 28 Optional:** Sága/Laminar integration (future enhancement)

### Notes

- Heimdall is accessed via HTTP, runs on port 8001
- Can use local models (free, fast) or cloud (more capable)
- Falls back gracefully if Heimdall unavailable
- May add 50-200ms latency per entity (profile if concerned)

---

## Work Organization

### Your Working Directory
```
/Users/mimir/Developer/Mimir/refgraph-rs/
├─ src/
│  ├─ lib.rs (main coordinator)
│  ├─ types.rs (don't modify - already complete)
│  ├─ error.rs (don't modify - already complete)
│  ├─ manifest.rs (don't modify - already complete)
│  ├─ extract.rs ← TEST & IMPLEMENT (Day 1)
│  ├─ dedup.rs ← TEST & OPTIMIZE (Day 2)
│  ├─ graph.rs ← TEST & IMPLEMENT (Day 3)
│  ├─ mimir.rs ← TEST & IMPLEMENT (Day 6)
│  └─ main.rs (CLI - already complete)
├─ Cargo.toml (workspace config)
├─ README.md (already complete)
├─ CLAUDE.md (update Day 8)
├─ TDD_WORKFLOW.md (reference, don't edit)
└─ tests/
   └─ integration_tests.rs (add Day 4)
```

### Each Day's Workflow
```
Morning:
  1. Read the target module (understand current code)
  2. Read test examples from EXAMPLE_TDD_EXTRACT.md
  3. Copy tests into src/<module>.rs #[cfg(test)] section

Afternoon:
  1. Run: cargo test --lib <module>::tests
  2. Watch tests FAIL (RED phase - this is expected!)
  3. Implement just enough to pass tests (GREEN phase)
  4. Refactor for clarity (REFACTOR phase)
  5. Final: cargo test → all green

End of day:
  1. Create git commit with clear message
  2. Verify: cargo fmt + cargo clippy
  3. Update progress in memory
```

---

## Critical Path Dependencies

```
CRITICAL PATH (must do in order):
  Day 1: extract.rs (extract features first)
    ↓
  Day 2: dedup.rs (deduplicate extractions)
    ↓
  Day 3: graph.rs (build relationships)
    ↓
  Day 4: lib.rs integration (coordinate pipeline)
    ↓
  Day 5: Neo4j (upload results)
    ↓
  Day 6: mimir.rs (format output)
    ↓
  Day 7: E2E testing (validate everything works)

PARALLEL WORK (can do anytime):
  • Grafana dashboard (independent of RefGraph code)
  • Test query preparation (independent of implementation)
  • Infrastructure verification (already done)
```

---

## Success Criteria (May 28, 5 PM)

```
✅ Modules Complete:
   ☐ extract.rs: 15 tests passing
   ☐ dedup.rs: 20+ tests passing
   ☐ graph.rs: 25+ tests passing
   ☐ lib.rs: 10 integration tests passing
   ☐ Neo4j: 5+ connection tests passing
   ☐ mimir.rs: 10 serialization tests passing

✅ Code Quality:
   ☐ 24+ unit tests passing
   ☐ 10+ integration tests passing
   ☐ 83% code coverage (cargo tarpaulin)
   ☐ Zero clippy warnings (cargo clippy)
   ☐ Zero compiler warnings

✅ Performance:
   ☐ Extract: <1ms per entity
   ☐ Dedup: <100ms for 1000 entities
   ☐ Full pipeline: <2 seconds for 1000 chunks
   ☐ Memory: <500MB for 10K entities

✅ Ready for S1:
   ☐ RefGraph production-ready
   ☐ Neo4j integration verified
   ☐ JSON/JSONL output formats tested
   ☐ All documentation updated
```

---

## What If You Get Stuck?

### Slow Progress (behind schedule)
```
Drop priority:
  1. Optimization (do on Day 7 instead)
  2. Edge cases (add post-May 28)
  3. Documentation (defer to Day 8)

Keep priority:
  1. All happy path tests passing
  2. All error case tests passing
  3. Integration tests passing

If lost >4 hours on any day:
  → Implement only happy path
  → Defer edge cases to Day 8
  → Still complete on time with refactoring priority shift
```

### Compile Errors
```
First: Check Cargo.toml dependencies exist
Second: Run: cargo check --lib
Third: Read error carefully (Rust compiler errors are precise)
Fourth: Look at similar code in existing modules
Fifth: Check TDD_WORKFLOW.md for similar pattern
```

### Test Failures
```
Use: RUST_BACKTRACE=full cargo test <test_name> -- --nocapture
This shows:
  1. Exact assertion that failed
  2. Expected vs actual values
  3. Full stack trace
```

### Neo4j Connection Issues
```
Verify:
  1. Neo4j running: curl localhost:7687
  2. Credentials correct in graph.rs
  3. Port 7687 accessible
  
Fallback: Use mock tests first, integrate Neo4j Day 7
```

---

## Git Workflow

### Commits (after each day)
```bash
# After each successful day:
git add src/<module>.rs
git commit -m "feat: <module> implementation with TDD

- Write 15 failing tests (RED)
- Implement to pass tests (GREEN)
- Refactor for clarity (REFACTOR)
- All <count> tests passing
- Zero clippy warnings
"

# Tag release on May 28:
git tag v1.0.0
git log --oneline (verify all 8 commits)
```

---

## Daily Standup (Solo Reflection)

Each morning, answer:
```
Yesterday:
  ✅ Tests passing: <count>
  ✅ Coverage: <percentage>
  ☐ Blockers: <none | list>

Today:
  ☐ Target: <extract | dedup | graph | etc>
  ☐ Tests to write: <count>
  ☐ Success = <specific metric>

Tomorrow:
  ☐ Depends on: <today's success>
  ☐ Fallback if stuck: <skip optimization | defer | etc>
```

---

## May 28 → June 2 (Bridge)

After May 28:
```
May 29-June 1 (Rest + small tasks):
  ☐ Review RefGraph for any last fixes
  ☐ Prepare real data files (Prudential, AXA, Thai Health)
  ☐ Create test queries (10 standardized searches)
  ☐ Set up Grafana dashboard refresh

June 2 (S1 Execution Begins):
  ☐ Load real data into RefGraph
  ☐ Run consolidation pipeline
  ☐ Ingest results into Mimir
  ☐ Test Hit Rate@3 ≥ 75%

June 12 (Go/No-Go):
  ☐ Decision based on Hit Rate results
  ☐ If <50% → activate Plan B (switch to Typhoon)
```

---

## Why You'll Finish

1. **Pre-designed code:** All 9 modules exist, you're just adding tests + refinement
2. **TDD reduces scope:** Tests define exactly what's needed, no gold-plating
3. **Copy-paste tests:** 15 tests already written, save hours
4. **Conservative timeline:** 8 days for 5 days of actual work
5. **Clear daily goals:** Each day is 4-6 hours of focused work
6. **Rust compiler help:** Compiler catches 80% of bugs before runtime
7. **No team overhead:** No meetings, no approvals, no communication delays

**Confidence: 9.5/10**

---

## TL;DR - What To Do Tomorrow (May 19)

```
Morning:
  1. cd /Users/mimir/Developer/Mimir/refgraph-rs
  2. Read src/extract.rs (207 lines)
  3. Read EXAMPLE_TDD_EXTRACT.md (15 tests)

Afternoon:
  1. Copy 15 tests into src/extract.rs
  2. Run: cargo test --lib extract::tests
  3. Watch them all FAIL (RED - expected!)
  4. Implement extract.rs to pass them (GREEN)
  5. Refactor for clarity (REFACTOR)
  6. Commit: "feat: entity extraction with TDD (15/15 tests)"

End of day: All 15 tests passing ✅
```

---

**Status:** Ready for May 19 execution  
**Confidence:** 9.5/10  
**Next:** Start Day 1 implementation tomorrow morning  

You've got this! 🚀
