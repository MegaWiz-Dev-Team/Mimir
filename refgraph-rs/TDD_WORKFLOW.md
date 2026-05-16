# Test-Driven Development (TDD) Workflow
## RefGraph Rust Implementation

**Principle:** Write tests FIRST, then implement code  
**Status:** Ready for May 19 implementation  
**Target:** 80%+ code coverage by May 28

---

## TDD Workflow (Cycle)

```
1. RED: Write failing test
   └─ Describes what feature should do
   └─ Test fails (code doesn't exist yet)

2. GREEN: Write minimal implementation
   └─ Just enough to pass the test
   └─ Ignore edge cases for now

3. REFACTOR: Clean up code
   └─ Remove duplication
   └─ Improve design
   └─ Keep tests passing

4. REPEAT for next feature
```

---

## Example: Add Entity to Graph (TDD)

### Step 1: RED - Write the test first

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_add_entity_to_graph_should_store_entity() {
        // ARRANGE: Set up test data
        let mut graph = SemanticGraph::new();
        let entity = ConsolidatedEntity {
            entity_id: "ent_001".to_string(),
            text: "Critical Illness".to_string(),
            entity_type: EntityType::Product,
            confidence: 0.95,
            sources: vec!["prudential.co.th".to_string()],
            compressed_refs: vec![],
            merged_from: vec![],
            metadata: HashMap::new(),
        };

        // ACT: Call the method
        let result = graph.add_entity(entity.clone());

        // ASSERT: Verify behavior
        assert!(result.is_ok());
        assert_eq!(graph.entities().len(), 1);
        assert_eq!(graph.get_entity("ent_001").unwrap().text, "Critical Illness");
    }
}
```

**Run test:** `cargo test test_add_entity_to_graph_should_store_entity`

**Result:** ❌ FAILS (graph.rs doesn't implement add_entity yet)

---

### Step 2: GREEN - Write minimal implementation

```rust
// In src/graph.rs
impl SemanticGraph {
    pub fn add_entity(&mut self, entity: ConsolidatedEntity) -> Result<()> {
        self.entities.insert(entity.entity_id.clone(), entity);
        Ok(())
    }

    pub fn get_entity(&self, id: &str) -> Option<&ConsolidatedEntity> {
        self.entities.get(id)
    }

    pub fn entities(&self) -> Vec<ConsolidatedEntity> {
        self.entities.values().cloned().collect()
    }
}
```

**Run test:** `cargo test test_add_entity_to_graph_should_store_entity`

**Result:** ✅ PASSES

---

### Step 3: REFACTOR - Improve design

Add validation + error handling:

```rust
impl SemanticGraph {
    pub fn add_entity(&mut self, entity: ConsolidatedEntity) -> Result<()> {
        if entity.entity_id.is_empty() {
            return Err(crate::error::Error::graph("Entity ID cannot be empty"));
        }
        self.entities.insert(entity.entity_id.clone(), entity);
        Ok(())
    }
}
```

Write new test for error case:

```rust
#[test]
fn test_add_entity_with_empty_id_should_fail() {
    let mut graph = SemanticGraph::new();
    let entity = ConsolidatedEntity {
        entity_id: "".to_string(), // Empty!
        text: "Bad Entity".to_string(),
        entity_type: EntityType::Product,
        confidence: 0.95,
        sources: vec![],
        compressed_refs: vec![],
        merged_from: vec![],
        metadata: HashMap::new(),
    };

    let result = graph.add_entity(entity);
    assert!(result.is_err());
}
```

**Run:** `cargo test test_add_entity`

**Result:** ✅ BOTH TESTS PASS

---

## Test Structure (AAA Pattern)

Every test follows **Arrange → Act → Assert**:

```rust
#[test]
fn test_feature_should_do_something() {
    // ARRANGE: Set up preconditions
    let mut system = System::new();
    let input = TestData::create();

    // ACT: Perform the action
    let result = system.do_something(input);

    // ASSERT: Verify the outcome
    assert_eq!(result.status, Status::Success);
    assert_eq!(result.count, 5);
}
```

---

## Test Categories

### 1. Unit Tests (Fast, Isolated)

```rust
// Test ONE function in isolation
#[test]
fn test_jaccard_similarity_identical_strings() {
    let dedup = Deduplicator::new(0.95);
    let similarity = dedup.jaccard_similarity("hello world", "hello world");
    assert_eq!(similarity, 1.0);
}
```

**When to use:** Default for all development  
**Speed:** <1ms  
**Coverage:** 70%+ of codebase

---

### 2. Integration Tests (Medium, Multiple components)

```rust
// Test multiple modules working together
#[tokio::test]
async fn test_consolidate_pipeline_end_to_end() {
    let config = ManifestConfig::insurance();
    let mut graph = RefGraph::new(config)?;
    
    let chunks = vec![
        RawChunk { /* ... */ },
        RawChunk { /* ... */ },
    ];
    
    let output = graph.consolidate(chunks).await?;
    
    assert_eq!(output.metadata.entity_count, 2);
    assert!(output.metadata.entity_count > 0);
}
```

**When to use:** After unit tests pass  
**Speed:** 10-100ms  
**Coverage:** 15%+ of interactions

---

### 3. CLI Integration Tests (Via --test flag)

```bash
./target/release/refgraph --test
```

**Tested scenarios:**
1. Create RefGraph with config
2. Consolidate empty chunks
3. Consolidate real sample chunks

**Output:**
```
✅ Test 1: RefGraph creation
✅ Test 2: Empty consolidation
✅ Test 3: Sample consolidation
✅ All tests passed!
```

---

## Test File Organization

```
src/
├─ lib.rs
│  └─ #[cfg(test)] mod tests { ... }
├─ graph.rs
│  └─ #[cfg(test)] mod tests { ... }
├─ dedup.rs
│  └─ #[cfg(test)] mod tests { ... }
├─ manifest.rs
│  └─ #[cfg(test)] mod tests { ... }
└─ ... (all modules have tests)
```

**Benefits:**
- Tests live next to code
- Easy to find + update
- Compile with `#[cfg(test)]` (not in release builds)

---

## Common Test Patterns

### Test 1: Happy Path

```rust
#[test]
fn test_dedup_merges_identical_entities() {
    let dedup = Deduplicator::new(0.95);
    let entities = vec![
        Entity { text: "Critical Illness", ... },
        Entity { text: "Critical Illness", ... }, // duplicate
    ];
    
    let consolidated = dedup.deduplicate(entities).unwrap();
    
    assert_eq!(consolidated.len(), 1); // Should merge
    assert_eq!(consolidated[0].merged_from.len(), 1);
}
```

### Test 2: Error Handling

```rust
#[test]
fn test_graph_add_entity_empty_id_fails() {
    let mut graph = SemanticGraph::new();
    let entity = ConsolidatedEntity {
        entity_id: "".to_string(), // Invalid!
        ... 
    };
    
    let result = graph.add_entity(entity);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::GraphError(_)));
}
```

### Test 3: Edge Cases

```rust
#[test]
fn test_jaccard_similarity_empty_strings() {
    let dedup = Deduplicator::new(0.95);
    let similarity = dedup.jaccard_similarity("", "");
    assert_eq!(similarity, 0.0); // Edge case handled
}
```

### Test 4: State Changes

```rust
#[test]
fn test_graph_relationships_count_increases() {
    let mut graph = SemanticGraph::new();
    assert_eq!(graph.all_relationships().len(), 0);
    
    // Add entities
    graph.add_entity(entity1).unwrap();
    graph.add_entity(entity2).unwrap();
    
    // Build relationships
    graph.build_relationships().unwrap();
    
    // Verify state changed
    assert!(graph.all_relationships().len() > 0);
}
```

---

## Running Tests

### Run all unit tests
```bash
cargo test --lib
```

### Run tests in specific module
```bash
cargo test graph::tests
```

### Run single test
```bash
cargo test test_jaccard_similarity_identical
```

### Run with output
```bash
cargo test -- --nocapture
```

### Run single-threaded (for debugging)
```bash
cargo test -- --test-threads=1
```

### Run with backtrace on panic
```bash
RUST_BACKTRACE=1 cargo test
```

### Generate coverage report (requires tarpaulin)
```bash
cargo tarpaulin --out Html
```

---

## TDD Implementation Plan (May 19-28)

### Phase 2: Core Implementation (May 19-24)

#### Day 1 (May 19): Entity Extraction
```
Morning:
  ☐ Write 10 tests for extract.rs
  ☐ Test entity patterns (products, coverages, exclusions)
  ☐ Test language detection
  ☐ Test confidence scoring

Afternoon:
  ☐ Implement extract.rs to pass tests
  ☐ Code review + refactor
  ☐ Commit with message: "feat: entity extraction with TDD"
  
Target: 15 tests, 100% passing
```

#### Day 2 (May 20): Deduplication Refinement
```
Morning:
  ☐ Write edge case tests for dedup.rs
  ☐ Test performance on large datasets
  ☐ Test confidence averaging
  ☐ Test source merging

Afternoon:
  ☐ Optimize dedup algorithm if needed
  ☐ Add benchmarks
  ☐ Commit: "perf: deduplication optimization with tests"

Target: 20 tests, all passing, <100ms for 1000 entities
```

#### Day 3 (May 21): Graph Relationships
```
Morning:
  ☐ Write 15 tests for graph.rs
  ☐ Test relationship inference
  ☐ Test Neo4j compatibility
  ☐ Test graph statistics

Afternoon:
  ☐ Implement relationship logic
  ☐ Add Neo4j integration stubs
  ☐ Commit: "feat: semantic graph with relationship inference"

Target: 25 tests, graph module complete
```

#### Day 4 (May 22): Consolidation Pipeline
```
Morning:
  ☐ Write 10 integration tests
  ☐ Test end-to-end consolidation
  ☐ Test error recovery
  ☐ Test output formats (JSON/JSONL)

Afternoon:
  ☐ Implement consolidate() method
  ☐ Add error handling
  ☐ Commit: "feat: consolidation pipeline (E2E tested)"

Target: 10 integration tests, pipeline complete
```

#### Day 5 (May 23-24): Neo4j Integration
```
May 23:
  ☐ Write tests for Neo4j connection
  ☐ Test relationship creation
  ☐ Test transaction handling
  ☐ Implement Neo4j client

May 24:
  ☐ Performance testing (relationship bulk insert)
  ☐ Error handling + retry logic
  ☐ Commit: "feat: Neo4j integration with TDD"

Target: 12 tests, Neo4j ready
```

### Phase 3: Integration & Testing (May 25-28)

#### Day 6 (May 25): Mimir Output
```
☐ Write tests for mimir.rs output formatting
☐ Test JSON serialization
☐ Test JSONL streaming
☐ Test metadata accuracy
☐ Implement final touches
```

#### Day 7 (May 26): E2E & Performance
```
☐ Full pipeline tests (1000+ chunks)
☐ Performance benchmarks
☐ Memory profiling
☐ Stress testing
```

#### Day 8 (May 27-28): Documentation & Review
```
☐ Code review (all modules)
☐ Update CLAUDE.md with test examples
☐ Create testing guide for future devs
☐ Final git commit
```

---

## Coverage Goals

| Module | Target | Method |
|--------|--------|--------|
| types.rs | 90% | Unit tests (types are straightforward) |
| error.rs | 85% | Unit tests (error variants) |
| manifest.rs | 85% | Unit + validation tests |
| extract.rs | 80% | Unit tests (many patterns) |
| dedup.rs | 90% | Unit tests (algorithm is critical) |
| graph.rs | 85% | Unit + integration tests |
| mimir.rs | 80% | Unit + serialization tests |
| lib.rs | 75% | Integration tests |

**Overall Target:** 83% code coverage by May 28

---

## Test Checklist Template

Use this for each feature:

```rust
// ✅ Happy path test
#[test]
fn test_feature_name_succeeds() { ... }

// ✅ Error case test
#[test]
fn test_feature_name_invalid_input_fails() { ... }

// ✅ Edge case test
#[test]
fn test_feature_name_empty_input() { ... }

// ✅ Boundary case test
#[test]
fn test_feature_name_large_input() { ... }

// ✅ Integration test
#[tokio::test]
async fn test_feature_integrates_with_pipeline() { ... }
```

---

## Debugging Failed Tests

### Test panics
```bash
# Run with backtrace
RUST_BACKTRACE=full cargo test test_name

# See the exact assertion
cargo test test_name -- --nocapture
```

### Test times out
```bash
# Increase timeout for slow tests
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn slow_test() { ... }
```

### Flaky tests (sometimes fail)
```bash
# Run test multiple times
for i in {1..10}; do cargo test test_name || break; done
```

---

## Example: Complete TDD Implementation

Let's implement `update_confidence()` with TDD:

### Step 1: Write tests (RED)

```rust
#[test]
fn test_update_confidence_sets_gauge() {
    let gauge = confidence_gauge();
    update_confidence(0.85);
    // How to assert gauge value? Need inspection method
}

#[test]
fn test_update_confidence_rejects_invalid() {
    assert!(update_confidence(-0.1).is_err());
    assert!(update_confidence(1.5).is_err());
}

#[test]
fn test_update_confidence_accepts_valid_range() {
    assert!(update_confidence(0.0).is_ok());
    assert!(update_confidence(0.5).is_ok());
    assert!(update_confidence(1.0).is_ok());
}
```

### Step 2: Implement (GREEN)

```rust
pub fn update_confidence(value: f32) -> Result<()> {
    if !(0.0..=1.0).contains(&value) {
        return Err(Error::Unknown("Confidence must be 0.0-1.0".to_string()));
    }
    confidence_gauge().set(value as f64);
    Ok(())
}
```

### Step 3: Refactor (CLEAN)

```rust
/// Update average confidence score gauge
/// 
/// # Arguments
/// * `value` - Confidence score (0.0 to 1.0)
/// 
/// # Errors
/// Returns error if value outside valid range
pub fn update_confidence(value: f32) -> Result<()> {
    const MIN: f32 = 0.0;
    const MAX: f32 = 1.0;
    
    if !(MIN..=MAX).contains(&value) {
        return Err(Error::Unknown(
            format!("Confidence must be between {} and {}", MIN, MAX)
        ));
    }
    
    confidence_gauge().set(value as f64);
    Ok(())
}
```

All tests still pass ✅

---

## TDD Rules

1. **Red First:** Always write failing test first
2. **One Test at a Time:** Don't write multiple failing tests
3. **Keep Tests Simple:** Each test = one behavior
4. **Clear Names:** Test name explains what it tests
5. **No Logic in Tests:** Tests shouldn't have if/loops (usually)
6. **Fast Feedback:** Run tests after every change
7. **Commit Green:** Only commit when all tests pass

---

## Benefits of TDD for RefGraph

✅ **Confidence:** Every feature is tested  
✅ **Documentation:** Tests show how to use code  
✅ **Refactoring:** Safe to improve design  
✅ **Bug Prevention:** Catch issues early  
✅ **Code Quality:** Better design emerges  
✅ **Coverage:** 80%+ naturally from TDD  

---

## Tools

```bash
# Run tests
cargo test

# Run with output + backtrace
RUST_BACKTRACE=1 cargo test -- --nocapture

# Check coverage
cargo tarpaulin --out Html

# Format code
cargo fmt

# Lint code
cargo clippy

# Watch for changes (requires cargo-watch)
cargo watch -x test
```

---

**Status:** Ready for May 19 implementation  
**Confidence:** 9.5/10  
**Target:** 24 unit tests + 10 integration tests by May 28  

Let's ship this! 🚀
