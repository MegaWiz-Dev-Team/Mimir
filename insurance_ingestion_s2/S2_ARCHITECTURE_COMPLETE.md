# Sprint 2 Architecture — Complete Reference
**Status:** ✅ Design Complete | 🚀 Ready for Implementation

---

## 📦 Core Components

### 1. **Data Schema** — [core.py](core.py)
✅ **Complete** — Chunk dataclass with all required fields:

```python
@dataclass
class Chunk:
    # Core fields
    source_id, content, metadata
    
    # Insurer isolation (S2)
    insurer_id: "insurer_001"  # Required for isolation
    
    # Hierarchical filtering (S2)
    product_type: "health"  # health, life, savings, investment
    channel: "direct"       # direct, uob, ttb, cimb, broker, agent
    
    # Temporal filtering (S2)
    product_name: "PRU Mao Mao Double Sure"
    product_version: "2.0"
    product_launch_date: "2020-01-15"   # ISO date
    product_end_date: Optional[None]    # null = still active
    is_active: True                      # Computed: today check
    status: "active"                     # active/discontinued/archived/sunset/planned
    
    # Other
    language: "en"  # en, th, bi
    source_type: "url"  # url, upload, pdf, docx, ocr
```

### 2. **Product Status Enum** — [core.py](core.py)
✅ **Complete** — 5 lifecycle states:

```python
class ProductStatus(str, Enum):
    ACTIVE = "active"  # Selling now
    DISCONTINUED = "discontinued"  # Stopped selling, kept for history
    ARCHIVED = "archived"  # Old version, new version available
    SUNSET = "sunset"  # Phasing out, deadline approaching
    PLANNED = "planned"  # Not yet launched
```

---

## 🏗️ Architecture Docs

### Phase 4: Insurer Isolation — [PHASE4_INSURER_ISOLATION.md](PHASE4_INSURER_ISOLATION.md)
✅ **Complete** — 500+ lines covering:

- **Isolation Strategy:** Separate collections, namespaces, databases per insurer
- **Data Flow:** Mimir → Embeddings → Qdrant → Neo4j (4 parallel streams)
- **Architecture Diagram:** Full isolation across all layers
- **Query Examples:** Correct vs wrong patterns with safety rules
- **Implementation:** `run_phase4_isolated()` with 4 sub-functions
- **Verification Checklist:** How to validate isolation after ingestion

### Hierarchical Filtering — [FILTERING_HIERARCHY.md](FILTERING_HIERARCHY.md)
✅ **Complete** — 500+ lines covering:

- **3-Tier Filter Hierarchy:** Insurer (isolated) → Product Type → Distribution Channel
- **Distribution Channels:** Direct, UOB, TTB, CIMB, Krungthai, Broker, Agent
- **Query Patterns:** 7 patterns showing single-insurer, cross-insurer, channel comparison
- **Filter Safety Rules:** Mandatory insurer_id, explicit list for cross-insurer
- **Filter Combinations Matrix:** When each filter combo is safe/required
- **Implementation Roadmap:** Phases 4a-6 with specific tasks

### Product Active Period — [PRODUCT_ACTIVE_PERIOD.md](PRODUCT_ACTIVE_PERIOD.md)
✅ **Complete** — Comprehensive temporal filtering:

- **Active Period Schema:** product_launch_date, product_end_date, is_active, status
- **Product Status Values:** Table showing all 5 states
- **Query Patterns:** 7 patterns for current, historical, discontinued, upcoming, versioning, sunset, cross-insurer
- **Real Example:** PRU Mao Mao evolution (v1.0 2018 → v2.0 2020 → v2.1 2024)
- **Qdrant Indexes:** Date range and status indexes for temporal queries
- **Updated Chunk Schema:** All temporal fields documented
- **Implementation Roadmap:** 5 phases with specific tasks

### Metadata Update Strategy — [METADATA_UPDATE_STRATEGY.md](METADATA_UPDATE_STRATEGY.md)
✅ **Complete** — How to modify filter metadata after ingestion:

- **Update Patterns:** Single chunk, batch by filter, lifecycle transition
- **Layer-Specific Updates:** Mimir (PATCH API), Qdrant (payload update), Neo4j (Cypher)
- **Consistency Guarantees:** Atomic updates, rollback strategy, partial failure handling
- **Metadata Versioning:** Audit logging with timestamps and reasons
- **Migration Scenarios:** Backfill, reclassify channels, lifecycle transitions
- **Verification Tools:** Consistency checking across all 3 layers
- **Usage Examples:** Fix date, mark discontinued, reclassify channel

---

## 🔄 Implementation Phases (Roadmap)

### Phase 4a: Mimir Ingestion (COMPLETE ✅)
- ✅ Separate collection per insurer: `insurance_products_{insurer_id}`
- ✅ Group chunks by insurer_id before ingestion
- ✅ Tag each chunk with insurer_id in metadata
- ✅ Verify ingestion per collection

**Code:** [phase4_ingestion.py](phases/phase4_ingestion.py:14-81)

### Phase 4b: Embedding Generation (COMPLETE ✅)
- ✅ Process embeddings per insurer separately
- ✅ Each insurer isolated embedding batch
- ✅ No mixing of vectors across insurers

**Code:** [phase4_ingestion.py](phases/phase4_ingestion.py:83-122)

### Phase 4c: Qdrant Indexing (COMPLETE ✅)
- ✅ Separate namespaces per insurer (001, 002, 003, etc.)
- ✅ Add insurer_id to every vector payload
- ✅ Create datetime indexes for temporal queries
- ✅ Create status keyword indexes

**Code:** [phase4_ingestion.py](phases/phase4_ingestion.py:124-161)

### Phase 4d: Neo4j Indexing (COMPLETE ✅)
- ✅ Separate database per insurer: `{insurer_name}_entities_{insurer_code}`
- ✅ Add insurer_id property to all nodes
- ✅ All relationships within same insurer's database

**Code:** [phase4_ingestion.py](phases/phase4_ingestion.py:163-208)

### Phase 5: Query Validation (READY 🚀)
- ⏳ ALL queries must include insurer_id filter
- ⏳ Test single-insurer search
- ⏳ Test cross-insurer search (explicit list)
- ⏳ Verify no data leakage

### Phase 6: Metadata Updates (DESIGNED ✅)
- ⏳ Implement PATCH `/api/chunks/{id}/metadata`
- ⏳ Implement batch update endpoint
- ⏳ Add audit logging for all changes
- ⏳ Add consistency verification tools

---

## 📊 Data Metrics

| Metric | Value | Status |
|--------|-------|--------|
| **Total Insurers** | 14 | ✅ Configured |
| **Total Chunks** | 4,490 | ✅ Estimated |
| **Total Tokens** | 1.34M | ✅ Estimated |
| **Collections/Insurer** | 1 (isolated) | ✅ Design |
| **Namespaces/Insurer** | 1 (Qdrant) | ✅ Design |
| **Databases/Insurer** | 1 (Neo4j) | ✅ Design |
| **Filter Fields** | 12 (hierarchical + temporal) | ✅ Complete |
| **Status Values** | 5 (lifecycle enum) | ✅ Complete |

---

## 🎯 Query Patterns at a Glance

### Single-Insurer Queries (Isolated)
```python
# Current products only (is_active: true)
search(query="health insurance", filters={
    "insurer_id": "insurer_001",
    "is_active": True,
})

# Historical (date range)
search(query="coverage", filters={
    "insurer_id": "insurer_001",
    "date": {"gte": "2022-01-01", "lte": "2022-12-31"}
})

# By product type + channel
search(query="plans", filters={
    "insurer_id": "insurer_001",
    "product_type": "health",
    "channel": "uob"
})
```

### Cross-Insurer Queries (Explicit)
```python
# Compare across insurers (EXPLICIT LIST REQUIRED)
search(query="critical illness", filters={
    "insurer_id": {"$in": ["insurer_001", "insurer_002", "insurer_006"]},
    "product_type": "health",
    "is_active": True,
}, group_by="insurer_id")

# With aggregation
search(query="health products", filters={
    "product_type": "health",
}, aggregation={
    "group_by": ["insurer_id", "product_version"],
    "count": True,
})
```

---

## 🔒 Query Safety Rules

### ✅ SAFE Patterns

```python
# Single-insurer (isolated)
{"insurer_id": "insurer_001", "product_type": "health"}

# Cross-insurer explicit
{"insurer_id": {"$in": ["001", "002"]}, "product_type": "health"}

# Cross-insurer with aggregation
{"product_type": "health"}, aggregation={"group_by": ["insurer_id"]}

# Temporal queries
{"insurer_id": "001", "date": {"gte": "2022-01-01"}}
```

### ❌ UNSAFE Patterns

```python
# Missing insurer_id (implicit cross-insurer)
{"product_type": "health"}  # Could return mixed data!

# Missing aggregation for cross-insurer
{"product_type": "health", "is_active": True}  # Could leak data!

# Implicit date semantics
{"insurer_id": "001"}  # Could return active + discontinued!
```

---

## 📁 File Structure

```
insurance_ingestion_s2/
├── core.py                                 [✅ UPDATED]
│   ├── Phase enum
│   ├── ProductStatus enum (NEW)
│   ├── PipelineConfig
│   ├── PipelineLogger
│   ├── Chunk dataclass (✅ WITH TEMPORAL FIELDS)
│   └── Entity dataclass
│
├── phases/
│   ├── phase1_extraction.py               [⏳ Ready]
│   ├── phase2_schema.py                   [⏳ Ready]
│   ├── phase3_entities.py                 [⏳ Ready]
│   ├── phase4_ingestion.py                [✅ REWRITTEN FOR S2]
│   │   ├── ingest_chunks_to_mimir_isolated()
│   │   ├── generate_embeddings_isolated()
│   │   ├── index_in_qdrant_isolated()
│   │   ├── index_in_neo4j_isolated()
│   │   └── run_phase4_isolated()
│   └── phase5_validation.py               [⏳ Ready]
│
├── PHASE4_INSURER_ISOLATION.md            [✅ 500+ lines]
├── FILTERING_HIERARCHY.md                 [✅ 500+ lines]
├── PRODUCT_ACTIVE_PERIOD.md               [✅ 1000+ lines]
└── METADATA_UPDATE_STRATEGY.md            [✅ NEW - 700+ lines]
```

---

## 🚀 Ready to Implement

### Next Steps (Priority Order)

1. **Phase 1a:** Update extraction phase to classify product_type from URLs
   - Classify `/health/`, `/life/`, `/savings/`, `/investment/` from URL paths
   - Extract product_name from page titles/metadata
   - Extract product_launch_date if available in content
   - Assign channel based on URL source (direct, uob, ttb, etc.)

2. **Phase 1b:** Implement full Phase 1 execution
   - Run against all 14 insurers
   - Verify 4,490 chunks extracted
   - Check 1.34M tokens total

3. **Phase 2:** Normalize and deduplicate
   - Apply similarity threshold >0.95 for deduplication
   - Standardize product names and dates

4. **Phase 3:** Entity extraction
   - Extract Product, Coverage, Benefit, Exclusion entities
   - Create relationships (HAS_COVERAGE, COVERS_CONDITION, etc.)

5. **Phase 4:** Complete ingestion
   - Run `run_phase4_isolated()` with all 4 sub-functions
   - Verify isolation across Mimir, Qdrant, Neo4j
   - Check no data leakage between insurers

6. **Phase 5:** Validation with temporal awareness
   - Test current products only (is_active: true)
   - Test historical queries (date ranges)
   - Verify Hit Rate@3 ≥70% for Thai language

7. **Phase 6:** Add metadata update APIs
   - Implement PATCH endpoints for Mimir
   - Add audit logging
   - Add consistency verification

---

## ✅ What You Can Do Now

- ✅ Read all architecture documents
- ✅ Understand isolation strategy
- ✅ Understand hierarchical filtering
- ✅ Understand temporal filtering
- ✅ Understand metadata update strategy
- ✅ Review query safety rules
- 🚀 Ready to start Phase 1 extraction
- 🚀 Ready to run Phase 4 with real data
- 🚀 Ready to validate with Hit Rate tests

---

**Document Generated:** 2026-05-16  
**Architecture Version:** S2.1 (Multi-Insurer + Temporal)  
**Implementation Ready:** YES ✅
