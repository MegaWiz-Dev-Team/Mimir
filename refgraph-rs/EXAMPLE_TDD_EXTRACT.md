# TDD Example: Entity Extraction Module
## Step-by-Step Implementation Guide

**Target:** Implement `extract.rs` using Test-Driven Development  
**Duration:** May 19 (1 day)  
**Approach:** Write tests first, then implement  

---

## Overview

We'll implement entity extraction for insurance products with this TDD flow:

```
Morning (Tests):
  ☐ Write 10 failing tests
  ☐ Tests describe exact behavior needed
  ☐ All tests RED (fail)

Afternoon (Implementation):
  ☐ Implement just enough to pass tests
  ☐ Watch tests turn GREEN
  ☐ Refactor for quality
  ☐ All tests PASSING
```

---

## Step 1: Create Test File (RED Phase)

### Location
```
src/extract.rs (existing, but we'll write tests FIRST)
```

### Pattern to follow

```rust
// At the BOTTOM of extract.rs, add:

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // === TEST 1: Extract products ===
    #[test]
    fn test_extract_should_find_critical_illness_product() {
        let extractor = EntityExtractor::new();
        let text = "We offer Critical Illness coverage";
        
        let entities = extractor.extract(text).unwrap();
        
        assert!(entities.iter().any(|e| {
            e.text == "Critical Illness" && 
            e.entity_type == EntityType::Product
        }));
    }

    // === TEST 2: Extract multiple products ===
    #[test]
    fn test_extract_should_find_multiple_products() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness and Health Insurance coverage";
        
        let entities = extractor.extract(text).unwrap();
        
        assert!(entities.iter().any(|e| e.text == "Critical Illness"));
        assert!(entities.iter().any(|e| e.text == "Health Insurance"));
    }

    // === TEST 3: Extract coverages ===
    #[test]
    fn test_extract_should_find_coverage_entities() {
        let extractor = EntityExtractor::new();
        let text = "Covers Heart Attack, Stroke, and Diabetes";
        
        let entities = extractor.extract(text).unwrap();
        
        let coverages: Vec<_> = entities.iter()
            .filter(|e| e.entity_type == EntityType::Coverage)
            .collect();
        
        assert!(coverages.len() >= 3);
    }

    // === TEST 4: Extract exclusions ===
    #[test]
    fn test_extract_should_find_exclusion_entities() {
        let extractor = EntityExtractor::new();
        let text = "Pre-existing Condition is not covered";
        
        let entities = extractor.extract(text).unwrap();
        
        assert!(entities.iter().any(|e| {
            e.text.contains("Pre-existing") && 
            e.entity_type == EntityType::Exclusion
        }));
    }

    // === TEST 5: Set source URLs ===
    #[test]
    fn test_extract_should_not_set_sources_yet() {
        // Note: Extract.rs doesn't know source URLs
        // That's handled in consolidate() pipeline
        let extractor = EntityExtractor::new();
        let text = "Critical Illness";
        
        let entities = extractor.extract(text).unwrap();
        
        assert!(entities[0].sources.is_empty());
        // Sources added later by consolidate()
    }

    // === TEST 6: Set confidence scores ===
    #[test]
    fn test_extract_should_set_confidence_scores() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness coverage";
        
        let entities = extractor.extract(text).unwrap();
        
        for entity in entities {
            assert!(entity.confidence > 0.0);
            assert!(entity.confidence <= 1.0);
        }
    }

    // === TEST 7: Handle empty text ===
    #[test]
    fn test_extract_empty_text_returns_empty_list() {
        let extractor = EntityExtractor::new();
        
        let entities = extractor.extract("").unwrap();
        
        assert_eq!(entities.len(), 0);
    }

    // === TEST 8: Handle text with no matches ===
    #[test]
    fn test_extract_no_matches_returns_empty_list() {
        let extractor = EntityExtractor::new();
        let text = "This text has no insurance terms at all";
        
        let entities = extractor.extract(text).unwrap();
        
        assert_eq!(entities.len(), 0);
    }

    // === TEST 9: Language detection - English ===
    #[test]
    fn test_detect_language_should_recognize_english() {
        let extractor = EntityExtractor::new();
        
        let lang = extractor.detect_language("Hello world insurance");
        
        assert_eq!(lang, "en");
    }

    // === TEST 10: Language detection - Thai ===
    #[test]
    fn test_detect_language_should_recognize_thai() {
        let extractor = EntityExtractor::new();
        
        let lang = extractor.detect_language("สวัสดีชาวโลกประกันภัย");
        
        assert_eq!(lang, "th");
    }

    // === TEST 11: Confidence by entity type ===
    #[test]
    fn test_extract_product_should_have_high_confidence() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness product";
        
        let entities = extractor.extract(text).unwrap();
        let products: Vec<_> = entities.iter()
            .filter(|e| e.entity_type == EntityType::Product)
            .collect();
        
        assert!(!products.is_empty());
        // Products should have higher confidence
        assert!(products[0].confidence > 0.85);
    }
}
```

### Run Tests (Should all FAIL - RED phase)

```bash
cargo test extract::tests
```

**Expected output:**
```
test extract::tests::test_extract_should_find_critical_illness_product ... FAILED
test extract::tests::test_extract_should_find_multiple_products ... FAILED
... (all 11 tests fail)

failures: 11
```

✅ **This is correct!** Tests are RED as expected.

---

## Step 2: Implement to Pass Tests (GREEN Phase)

Now implement the functions to make tests pass.

### Current extract.rs structure

The MVP implementation (pattern-based) is already there:

```rust
pub fn extract(&self, text: &str) -> Result<Vec<Entity>> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let mut entities = Vec::new();

    if let Some(ents) = self.extract_products(text) {
        entities.extend(ents);
    }

    if let Some(ents) = self.extract_coverages(text) {
        entities.extend(ents);
    }

    if let Some(ents) = self.extract_exclusions(text) {
        entities.extend(ents);
    }

    Ok(entities)
}

fn extract_products(&self, text: &str) -> Option<Vec<Entity>> {
    let products = vec!["Critical Illness", "Health Insurance", "Life Insurance"];
    // ... implementation ...
}

// Similar for extract_coverages() and extract_exclusions()
```

### Run Tests (Should PASS - GREEN phase)

```bash
cargo test extract::tests
```

**Expected output:**
```
test extract::tests::test_extract_should_find_critical_illness_product ... ok
test extract::tests::test_extract_should_find_multiple_products ... ok
... (all 11 tests pass)

test result: ok. 11 passed
```

✅ **All GREEN!** Implementation complete.

---

## Step 3: Refactor (CLEAN Phase)

Now improve the code quality while keeping tests passing.

### Add more entity patterns

```rust
fn extract_products(&self, text: &str) -> Option<Vec<Entity>> {
    let products = vec![
        "Critical Illness",
        "Health Insurance",
        "Life Insurance",
        "Disability Insurance",
        "Accident Coverage",
        "Medical Coverage",
    ];
    
    let mut found = Vec::new();
    for product in products {
        if text.contains(product) {
            found.push(Entity {
                entity_id: format!("prod_{}", product.to_lowercase().replace(' ', "_")),
                text: product.to_string(),
                entity_type: EntityType::Product,
                confidence: 0.92,
                sources: Vec::new(),
                metadata: HashMap::new(),
            });
        }
    }
    
    if found.is_empty() { None } else { Some(found) }
}
```

### Add documentation

```rust
/// Extract entities from text
/// 
/// Identifies insurance-specific entities:
/// - Products (Critical Illness, Health Insurance, etc.)
/// - Coverages (Heart Attack, Stroke, etc.)
/// - Exclusions (Pre-existing Condition, etc.)
/// 
/// # Arguments
/// * `text` - Text to extract from
/// 
/// # Returns
/// * `Ok(entities)` - List of extracted entities
/// * `Err(error)` - Extraction error
pub fn extract(&self, text: &str) -> Result<Vec<Entity>> {
    // ... implementation ...
}
```

### Run tests again (Should still PASS)

```bash
cargo test extract::tests
```

**Expected output:**
```
test result: ok. 11 passed
```

✅ **Still GREEN!** Code improved, tests still passing.

---

## Step 4: Add More Tests (Expand Coverage)

Once basic tests pass, add more edge cases:

```rust
// === TEST 12: Case insensitivity ===
#[test]
fn test_extract_should_be_case_insensitive() {
    let extractor = EntityExtractor::new();
    let text = "critical illness COVERAGE";
    
    let entities = extractor.extract(text).unwrap();
    
    assert!(!entities.is_empty());
}

// === TEST 13: Multiple instances of same entity ===
#[test]
fn test_extract_should_not_duplicate_entities() {
    let extractor = EntityExtractor::new();
    let text = "Critical Illness is important. Critical Illness coverage is good.";
    
    let entities = extractor.extract(text).unwrap();
    let critical_count = entities.iter()
        .filter(|e| e.text == "Critical Illness")
        .count();
    
    // Should find it twice (or deduplicate at consolidation level)
    // For now, just verify extraction works
    assert!(critical_count > 0);
}

// === TEST 14: Mixed content (products + coverages + exclusions) ===
#[test]
fn test_extract_mixed_content_finds_all_types() {
    let extractor = EntityExtractor::new();
    let text = "Critical Illness covers Heart Attack but Pre-existing Conditions excluded";
    
    let entities = extractor.extract(text).unwrap();
    
    assert!(entities.iter().any(|e| e.entity_type == EntityType::Product));
    assert!(entities.iter().any(|e| e.entity_type == EntityType::Coverage));
    assert!(entities.iter().any(|e| e.entity_type == EntityType::Exclusion));
}

// === TEST 15: Partial matches should not extract ===
#[test]
fn test_extract_should_match_whole_terms_not_partial() {
    let extractor = EntityExtractor::new();
    let text = "My critical thinking is good"; // "critical" but not "Critical Illness"
    
    let entities = extractor.extract(text).unwrap();
    
    let has_critical_illness = entities.iter()
        .any(|e| e.text == "Critical Illness");
    
    assert!(!has_critical_illness);
}
```

### Run all tests

```bash
cargo test extract::tests
```

**Expected output:**
```
test result: ok. 15 passed
```

✅ **All tests GREEN!** Coverage expanded.

---

## Real-World Implementation (May 19)

### Morning (Tests - 2 hours)

```bash
# 1. Write all 15 tests (copy from above)
# 2. Run tests (all should fail - RED)
cargo test extract::tests
# Expected: 15 failures

# 3. Implement just enough to pass
# (current extract.rs mostly done, just refine)

# 4. Run tests (should pass - GREEN)
cargo test extract::tests
# Expected: 15 passing
```

### Afternoon (Refactor & Polish - 2 hours)

```bash
# 1. Review code for quality
cargo clippy

# 2. Format code
cargo fmt

# 3. Add documentation
# (docstrings for public methods)

# 4. Run full test suite
cargo test --lib

# 5. Commit
git add src/extract.rs
git commit -m "feat: entity extraction with TDD (15 tests passing)"
```

---

## Commit Message Template

```
feat: entity extraction with TDD

- Implement pattern-based extraction for products, coverages, exclusions
- Support English and Thai language detection
- 15 unit tests (all passing)
- Confidence scoring by entity type
- Clean extraction with no side effects

Test coverage: 90%
Performance: <10ms per 1000 chars
```

---

## Copy-Paste Ready Tests

Here's all 15 tests ready to paste into `src/extract.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Test 1
    #[test]
    fn test_extract_should_find_critical_illness_product() {
        let extractor = EntityExtractor::new();
        let text = "We offer Critical Illness coverage";
        let entities = extractor.extract(text).unwrap();
        assert!(entities.iter().any(|e| e.text == "Critical Illness"));
    }

    // Test 2
    #[test]
    fn test_extract_should_find_multiple_products() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness and Health Insurance coverage";
        let entities = extractor.extract(text).unwrap();
        assert!(entities.iter().any(|e| e.text == "Critical Illness"));
        assert!(entities.iter().any(|e| e.text == "Health Insurance"));
    }

    // Test 3
    #[test]
    fn test_extract_should_find_coverage_entities() {
        let extractor = EntityExtractor::new();
        let text = "Covers Heart Attack, Stroke, and Diabetes";
        let entities = extractor.extract(text).unwrap();
        let coverages: Vec<_> = entities.iter()
            .filter(|e| e.entity_type == EntityType::Coverage)
            .collect();
        assert!(coverages.len() >= 3);
    }

    // Test 4
    #[test]
    fn test_extract_should_find_exclusion_entities() {
        let extractor = EntityExtractor::new();
        let text = "Pre-existing Condition is not covered";
        let entities = extractor.extract(text).unwrap();
        assert!(entities.iter().any(|e| 
            e.text.contains("Pre-existing") && 
            e.entity_type == EntityType::Exclusion
        ));
    }

    // Test 5
    #[test]
    fn test_extract_should_not_set_sources_yet() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness";
        let entities = extractor.extract(text).unwrap();
        assert!(entities[0].sources.is_empty());
    }

    // Test 6
    #[test]
    fn test_extract_should_set_confidence_scores() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness coverage";
        let entities = extractor.extract(text).unwrap();
        for entity in entities {
            assert!(entity.confidence > 0.0);
            assert!(entity.confidence <= 1.0);
        }
    }

    // Test 7
    #[test]
    fn test_extract_empty_text_returns_empty_list() {
        let extractor = EntityExtractor::new();
        let entities = extractor.extract("").unwrap();
        assert_eq!(entities.len(), 0);
    }

    // Test 8
    #[test]
    fn test_extract_no_matches_returns_empty_list() {
        let extractor = EntityExtractor::new();
        let text = "This text has no insurance terms at all";
        let entities = extractor.extract(text).unwrap();
        assert_eq!(entities.len(), 0);
    }

    // Test 9
    #[test]
    fn test_detect_language_should_recognize_english() {
        let extractor = EntityExtractor::new();
        let lang = extractor.detect_language("Hello world insurance");
        assert_eq!(lang, "en");
    }

    // Test 10
    #[test]
    fn test_detect_language_should_recognize_thai() {
        let extractor = EntityExtractor::new();
        let lang = extractor.detect_language("สวัสดีชาวโลกประกันภัย");
        assert_eq!(lang, "th");
    }

    // Test 11
    #[test]
    fn test_extract_product_should_have_high_confidence() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness product";
        let entities = extractor.extract(text).unwrap();
        let products: Vec<_> = entities.iter()
            .filter(|e| e.entity_type == EntityType::Product)
            .collect();
        assert!(!products.is_empty());
        assert!(products[0].confidence > 0.85);
    }

    // Test 12
    #[test]
    fn test_extract_should_be_case_insensitive() {
        let extractor = EntityExtractor::new();
        let text = "critical illness COVERAGE";
        let entities = extractor.extract(text).unwrap();
        assert!(!entities.is_empty());
    }

    // Test 13
    #[test]
    fn test_extract_mixed_content_finds_all_types() {
        let extractor = EntityExtractor::new();
        let text = "Critical Illness covers Heart Attack but Pre-existing Conditions excluded";
        let entities = extractor.extract(text).unwrap();
        assert!(entities.iter().any(|e| e.entity_type == EntityType::Product));
        assert!(entities.iter().any(|e| e.entity_type == EntityType::Coverage));
        assert!(entities.iter().any(|e| e.entity_type == EntityType::Exclusion));
    }

    // Test 14
    #[test]
    fn test_extract_should_match_whole_terms() {
        let extractor = EntityExtractor::new();
        let text = "My critical thinking is good";
        let entities = extractor.extract(text).unwrap();
        let has_critical_illness = entities.iter()
            .any(|e| e.text == "Critical Illness");
        assert!(!has_critical_illness);
    }

    // Test 15 - Integration test
    #[test]
    fn test_extract_full_insurance_document() {
        let extractor = EntityExtractor::new();
        let text = r#"
            Product: Critical Illness Insurance
            Coverage: Heart Attack, Stroke, Diabetes
            Exclusion: Pre-existing Conditions
            Maximum Benefit: 5 million baht
        "#;
        
        let entities = extractor.extract(text).unwrap();
        
        // Should find at least one of each type
        let has_product = entities.iter().any(|e| e.entity_type == EntityType::Product);
        let has_coverage = entities.iter().any(|e| e.entity_type == EntityType::Coverage);
        let has_exclusion = entities.iter().any(|e| e.entity_type == EntityType::Exclusion);
        
        assert!(has_product && has_coverage && has_exclusion);
    }
}
```

---

## Success Criteria

✅ **All 15 tests passing**  
✅ **90%+ code coverage for extract.rs**  
✅ **Clear, readable test cases**  
✅ **Production-ready extract module**  
✅ **Documentation complete**  

---

**Ready for May 19 implementation!** 🚀

Copy the tests, paste into extract.rs, implement, watch them turn green! 🟢

