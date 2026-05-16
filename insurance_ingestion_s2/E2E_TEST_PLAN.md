# End-to-End (E2E) Test Plan
## Sprint 2 Insurance Ingestion Pipeline

**Status:** ⏳ Not Yet Implemented | Tests Needed: 45+ scenarios

---

## 📋 Current Test Coverage

### ✅ Unit Tests (Exist)
- Phase 1 URL extraction (positive cases)
- Phase 1 file uploads (PDF, DOCX, TXT, images)
- Chunking logic
- Product classification
- Channel detection

### ❌ E2E Tests (MISSING)
- Complete pipeline execution (1-5)
- Multi-insurer isolation verification
- Data consistency across layers
- Error handling and recovery
- Negative/edge cases

### ❌ UI Tests (MISSING)
- Source management UI
- Product filtering by insurer/type/channel
- Search quality validation
- Error message display

---

## 🎯 E2E Test Scenarios (45 Cases)

### CATEGORY 1: Happy Path (Positive Cases)

#### 1.1 Single Insurer Full Pipeline
```
Scenario: Extract, normalize, and ingest single insurer (Prudential)
├─ Phase 1: Extract 2 URLs → 5 chunks
├─ Phase 2: Normalize → all 12 fields present
├─ Phase 3: Extract 20 entities + relationships
├─ Phase 4: Ingest to Mimir (collection: insurance_products_001)
├─ Phase 4: Index in Qdrant (namespace: 001)
├─ Phase 4: Index in Neo4j (database: prudential_entities_001)
└─ Phase 5: Validate ✅ (Hit Rate ≥75%)
```

#### 1.2 Multi-Insurer Full Pipeline
```
Scenario: Extract all 14 insurers, verify isolation
├─ Phase 1: Extract 28 URLs → 156 chunks (all 14 insurers)
├─ Phase 2: Normalize → group by insurer_id
├─ Phase 3: Extract 5000+ entities
├─ Phase 4: Verify 14 isolated collections in Mimir
├─ Phase 4: Verify 14 isolated namespaces in Qdrant
├─ Phase 4: Verify 14 isolated databases in Neo4j
└─ Phase 5: No data leakage between insurers ✅
```

#### 1.3 Product Metadata Classification
```
Scenario: Verify product type & channel are classified correctly
├─ Phase 1: Extract health URLs → product_type = "health" ✅
├─ Phase 1: Extract life URLs → product_type = "life" ✅
├─ Phase 1: Extract direct domain URLs → channel = "direct" ✅
├─ Phase 1: Extract UOB URLs → channel = "uob" ✅
└─ Phase 1: All chunks have product_name extracted ✅
```

#### 1.4 Temporal Metadata (Product Active Period)
```
Scenario: Product launch date and status tracking
├─ Phase 1: Extract launch dates from content ✅
├─ Phase 2: Normalize dates to ISO format ✅
├─ Phase 4: Store product_launch_date in all layers ✅
├─ Phase 5: Query by date range: success ✅
└─ Phase 5: is_active calculated correctly ✅
```

#### 1.5 Thai Language Support
```
Scenario: Extract and ingest Thai-language products
├─ Phase 1: Extract Thai URLs → language="th" ✅
├─ Phase 2: Detect Thai content ✅
├─ Phase 3: NER on Thai text works ✅
├─ Phase 5: Thai queries return results ✅
└─ Phase 5: Hit Rate ≥70% on Thai queries ✅
```

---

### CATEGORY 2: Negative Cases (Error Handling)

#### 2.1 URL Failures
```
Scenario: Some URLs fail during extraction
├─ Phase 1: URL 1 → 404 Not Found ❌
├─ Phase 1: URL 2 → Connection timeout ❌
├─ Phase 1: URL 3 → Success ✅
├─ Expected: Continue with successful URLs
└─ Result: 1/3 URLs extracted, log warnings ⚠️
```

#### 2.2 Malformed Content
```
Scenario: HTML with no readable content
├─ Phase 1: URL returns empty page ❌
├─ Phase 1: URL returns only JavaScript ❌
├─ Phase 1: URL returns redirect loop ❌
└─ Expected: Skip or handle gracefully, continue ✅
```

#### 2.3 Empty Extraction
```
Scenario: No URLs provided / no files to process
├─ Phase 1: urls=[], file_paths=[] ❌
├─ Phase 1: Should produce empty chunks.jsonl ✅
├─ Phase 2: Empty input handled gracefully ✅
├─ Phase 4: No data ingested ✅
└─ Phase 5: Skipped (no data) ✅
```

#### 2.4 Mimir Connection Failure
```
Scenario: Mimir service unreachable during Phase 4
├─ Phase 4: POST /api/ingest → 503 Service Unavailable ❌
├─ Expected: Raise IngestionError ✅
├─ Expected: Rollback (don't partial commit) ✅
└─ Expected: Log detailed error with retry hint ✅
```

#### 2.5 Qdrant Connection Failure
```
Scenario: Qdrant service down during Phase 4
├─ Phase 4: Generate embeddings ✅
├─ Phase 4: Index in Qdrant → Connection refused ❌
├─ Expected: Raise IngestionError ✅
├─ Expected: Mimir remains valid (partial state) ⚠️
└─ Expected: Log that Qdrant is missing ✅
```

#### 2.6 Neo4j Connection Failure
```
Scenario: Neo4j authentication fails during Phase 4
├─ Phase 4: Connect to Neo4j → 401 Unauthorized ❌
├─ Expected: Raise error and stop ✅
├─ Expected: Mimir + Qdrant already committed ⚠️
└─ Expected: Clear error message about credentials ✅
```

#### 2.7 Invalid Configuration
```
Scenario: Bad config values in main.py
├─ target_tokens=0 ❌
├─ batch_size=-1 ❌
├─ mimir_url="" ❌
└─ Expected: Validation error during config parse ✅
```

---

### CATEGORY 3: Data Isolation (Multi-Insurer Security)

#### 3.1 Insurer Data Doesn't Leak
```
Scenario: Prudential data doesn't appear in AXA queries
├─ Phase 4: Ingest Prudential (insurer_001) ✅
├─ Phase 4: Ingest AXA (insurer_002) ✅
├─ Phase 5: Query AXA → only AXA results ✅
├─ Verify: Zero Prudential chunks in AXA results ✅
└─ Security: Data isolation validated ✅
```

#### 3.2 Cross-Insurer Query Requires Explicit List
```
Scenario: Can't accidentally query multiple insurers
├─ Query without insurer_id filter → Error ❌
├─ Query with {"insurer_id": "001"} → OK ✅
├─ Query with {"$in": ["001", "002"]} → OK ✅
├─ Query with just product_type → Error (ambiguous) ❌
└─ Safety: Implicit cross-insurer blocked ✅
```

#### 3.3 Collections Are Per-Insurer
```
Scenario: Verify Mimir collections are isolated
├─ Expected collections:
│  ├─ insurance_products_001 (Prudential)
│  ├─ insurance_products_002 (AXA)
│  └─ insurance_products_003...014 (others)
├─ Verify: GET /api/collections lists all 14 ✅
└─ Verify: No shared collection across insurers ✅
```

#### 3.4 Namespaces Are Per-Insurer
```
Scenario: Verify Qdrant namespaces are isolated
├─ Expected namespaces in Qdrant:
│  ├─ 001 (Prudential)
│  ├─ 002 (AXA)
│  └─ 003...013 (others)
├─ Verify: GET /collections/.../namespaces ✅
└─ Verify: Vector search on ns:001 only ✅
```

#### 3.5 Databases Are Per-Insurer
```
Scenario: Verify Neo4j databases are isolated
├─ Expected databases:
│  ├─ prudential_entities_001
│  ├─ axa_entities_002
│  └─ thai_health_entities_003...etc
├─ Verify: SHOW DATABASES lists all 14 ✅
└─ Verify: Query on db A doesn't see db B ✅
```

---

### CATEGORY 4: Filter Functionality

#### 4.1 Single-Level Filters
```
Scenario: Filter by insurer only
├─ Query: {insurer_id: "001"} → Only Prudential ✅
├─ Query: {insurer_id: "002"} → Only AXA ✅
└─ Expected: Correct insurer isolation ✅
```

#### 4.2 Hierarchical Filters
```
Scenario: Filter by insurer + product_type
├─ Query: {insurer_id: "001", product_type: "health"}
│  → Only Prudential health products ✅
├─ Query: {insurer_id: "002", product_type: "life"}
│  → Only AXA life products ✅
└─ Combinatorial: All combinations work ✅
```

#### 4.3 Temporal Filters
```
Scenario: Filter by product active period
├─ Query: {is_active: true} → Only current products ✅
├─ Query: {status: "discontinued"} → Only old products ✅
├─ Query: {product_launch_date: {"gte": "2020-01-01"}}
│  → Products launched after 2020 ✅
└─ Date ranges work correctly ✅
```

#### 4.4 Channel Filters
```
Scenario: Filter by distribution channel
├─ Query: {insurer_id: "001", channel: "direct"}
│  → Only Prudential direct sales ✅
├─ Query: {insurer_id: "001", channel: "uob"}
│  → Only Prudential UOB partnerships ✅
└─ All channels (direct/uob/ttb/cimb) work ✅
```

---

### CATEGORY 5: Deduplication & Quality

#### 5.1 Duplicate Detection (Phase 2)
```
Scenario: Identical chunks from multiple URLs
├─ Phase 1: Extract same content from 2 URLs ✅
├─ Phase 2: Similarity > 0.95 → Mark duplicate ✅
├─ Phase 3: Deduplicated → Single entity ✅
└─ Result: No redundant entities in graph ✅
```

#### 5.2 PII Abstraction (Phase 2)
```
Scenario: Personal/sensitive data is abstracted
├─ Phase 2: URLs hidden (vendor abstracted) ✅
├─ Phase 2: Company names abstracted ✅
├─ Phase 5: Search results don't leak PII ✅
└─ Security: No raw URLs in output ✅
```

#### 5.3 Metadata Consistency (Phase 2)
```
Scenario: All chunks have all 12+ required fields
├─ Phase 1: Extract → Has 8 base fields
├─ Phase 2: Normalize → Add 4 more (product_type, etc)
├─ Verify: 100% chunks have all fields ✅
└─ Quality: No missing data ✅
```

---

### CATEGORY 6: Query Quality

#### 6.1 Hit Rate Validation (Phase 5)
```
Scenario: Query quality meets acceptance criteria
├─ 10 test queries (mix of difficulty levels)
├─ English queries: Hit Rate@3 ≥75% ✅
├─ Thai queries: Hit Rate@3 ≥70% ✅
└─ Fallback: Activate Plan B if <50% ⚠️
```

#### 6.2 Latency Validation (Phase 5)
```
Scenario: Query response time is acceptable
├─ Single query latency <500ms ✅
├─ Batch of 10 queries <60s total ✅
└─ Performance: Within SLA ✅
```

#### 6.3 Relevance Ranking (Phase 5)
```
Scenario: Top-3 results are relevant
├─ Query: "health insurance coverage"
├─ Result[0]: Health product detail ✅
├─ Result[1]: Health product feature ✅
├─ Result[2]: Health product premium ✅
└─ Ranking: Semantic relevance verified ✅
```

---

### CATEGORY 7: Metadata Updates (Phase 6)

#### 7.1 Single Chunk Update
```
Scenario: Update product_launch_date for one chunk
├─ Before: product_launch_date: ""
├─ Update: product_launch_date: "2020-01-15"
├─ Verify Mimir: Updated ✅
├─ Verify Qdrant: Payload updated ✅
├─ Verify Neo4j: Properties updated ✅
└─ Result: All 3 layers in sync ✅
```

#### 7.2 Batch Update by Filter
```
Scenario: Update status for all Prudential health products
├─ Filter: {insurer_id: "001", product_type: "health"}
├─ Update: status: "discontinued"
├─ Verify: 50 chunks updated in Mimir ✅
├─ Verify: 50 vectors updated in Qdrant ✅
├─ Verify: 50 entities updated in Neo4j ✅
└─ Consistency: All layers updated together ✅
```

#### 7.3 Product Lifecycle Transition
```
Scenario: Transition product: active → sunset → discontinued
├─ Day 1: status: "active" ✅
├─ Day 2: Update to "sunset" (end_date approaching) ✅
├─ Day 90: Update to "discontinued" ✅
└─ Result: Lifecycle tracked correctly ✅
```

---

## 📝 Test Case Template

Each test case should have:

```python
def test_[category]_[scenario](self):
    """
    Test: [Human-readable description]
    
    Given: [Setup/preconditions]
    When: [Action taken]
    Then: [Expected result]
    """
    # Arrange
    # ...setup...
    
    # Act
    # ...execute...
    
    # Assert
    # ...verify...
    
    # Cleanup (if needed)
```

---

## 🏗️ E2E Test Architecture

### Framework
- **Pytest** for test orchestration
- **pytest-mock** for mocking services
- **hypothesis** for property-based testing (edge cases)

### Fixtures
```python
# conftest.py
@pytest.fixture
def temp_config():
    """Temporary config with test directories"""
    return PipelineConfig(
        output_dir="/tmp/test_output",
        ...
    )

@pytest.fixture
def mock_k8s_services():
    """Mock Mimir, Qdrant, Neo4j endpoints"""
    with patch('requests.post') as mock_post:
        yield mock_post
```

### Test Organization
```
tests/
├── e2e/
│   ├── test_pipeline_happy_path.py       (1.1-1.5)
│   ├── test_pipeline_error_handling.py   (2.1-2.7)
│   ├── test_data_isolation.py            (3.1-3.5)
│   ├── test_filtering.py                 (4.1-4.4)
│   ├── test_data_quality.py              (5.1-5.3)
│   ├── test_query_quality.py             (6.1-6.3)
│   └── test_metadata_updates.py          (7.1-7.3)
└── fixtures/
    ├── sample_k8s_responses.py
    └── test_data_sets.py
```

---

## 🚀 Execution Plan

### Phase 1: Setup (Day 1)
- [ ] Create test directory structure
- [ ] Set up pytest + fixtures
- [ ] Create mock K8s service responses
- [ ] Implement basic test runner

### Phase 2: Happy Path Tests (Days 2-3)
- [ ] 1.1 Single insurer
- [ ] 1.2 Multi-insurer
- [ ] 1.3 Product classification
- [ ] 1.4 Temporal metadata
- [ ] 1.5 Thai language

### Phase 3: Error Handling Tests (Days 4-5)
- [ ] 2.1-2.7 Negative cases
- [ ] Rollback verification
- [ ] Error message validation

### Phase 4: Data Isolation Tests (Day 6)
- [ ] 3.1-3.5 Multi-insurer isolation
- [ ] Cross-insurer safety rules

### Phase 5: Filter & Quality Tests (Day 7)
- [ ] 4.1-4.4 Filter combinations
- [ ] 5.1-5.3 Quality checks
- [ ] 6.1-6.3 Query validation

### Phase 6: Metadata & UI Tests (Day 8)
- [ ] 7.1-7.3 Metadata updates
- [ ] UI test scenarios (add later)

---

## ✅ Success Criteria

- [ ] 45+ E2E test cases implemented
- [ ] 100% of happy path scenarios pass
- [ ] All negative cases handled gracefully
- [ ] Data isolation verified for all 14 insurers
- [ ] All filter combinations working
- [ ] Hit Rate ≥75% (English) / ≥70% (Thai)
- [ ] No data leakage between insurers
- [ ] All errors logged with actionable messages

---

## 📊 Coverage Matrix

| Scenario | Unit | E2E | UI |
|----------|------|-----|-----|
| Single insurer pipeline | ✅ | ⏳ | ❌ |
| Multi-insurer pipeline | ⚠️ | ⏳ | ❌ |
| Product classification | ✅ | ⏳ | ❌ |
| Temporal filtering | ❌ | ⏳ | ❌ |
| Error handling | ⚠️ | ⏳ | ❌ |
| Data isolation | ❌ | ⏳ | ❌ |
| Query quality | ❌ | ⏳ | ❌ |
| Metadata updates | ❌ | ⏳ | ❌ |
| UI interactions | ❌ | ❌ | ⏳ |

---

**Status:** 📋 Plan ready, 0/45 tests implemented | Ready to build ✅
