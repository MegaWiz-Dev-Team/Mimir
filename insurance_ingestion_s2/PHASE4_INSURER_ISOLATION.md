# Phase 4: Insurer-Level Data Isolation Architecture

**Objective:** Ensure complete data separation between insurers at ALL levels (Mimir, Vector DB, Graph DB, Page Index).

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│              ASGARD MIMIR (PostgreSQL)                           │
│  Tenant: asgard_insurance                                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Collection: insurance_products_001     [Prudential]             │
│  ├─ insurer_id: insurer_001 (indexed, unique)                   │
│  ├─ Chunks: 83                                                   │
│  ├─ Tokens: 24,900                                              │
│  └─ Query filter: WHERE insurer_id = 'insurer_001'             │
│                                                                   │
│  Collection: insurance_products_002     [AXA]                    │
│  ├─ insurer_id: insurer_002 (indexed, unique)                   │
│  ├─ Chunks: 1                                                    │
│  ├─ Tokens: 280                                                  │
│  └─ Query filter: WHERE insurer_id = 'insurer_002'             │
│                                                                   │
│  Collection: insurance_products_003     [Thai Health]            │
│  ├─ insurer_id: insurer_003 (indexed, unique)                   │
│  ├─ Chunks: 0 (pending)                                         │
│  └─ Query filter: WHERE insurer_id = 'insurer_003'             │
│                                                                   │
│  ... (insurance_products_004 → insurance_products_014)           │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
         ↓
┌─────────────────────────────────────────────────────────────────┐
│           QDRANT VECTOR DATABASE                                 │
│  Collection: insurance_products_embeddings                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Namespace: 001 (Prudential)                                     │
│  ├─ Vectors: 83                                                  │
│  ├─ Dimension: 1024 (BGE-M3)                                    │
│  └─ Payload: {insurer_id: "insurer_001", content_id, metadata}  │
│                                                                   │
│  Namespace: 002 (AXA)                                            │
│  ├─ Vectors: 1                                                   │
│  ├─ Dimension: 1024 (BGE-M3)                                    │
│  └─ Payload: {insurer_id: "insurer_002", content_id, metadata}  │
│                                                                   │
│  Namespace: 003 (Thai Health)                                    │
│  ├─ Vectors: 0 (pending)                                         │
│  └─ Payload filter: insurer_id = "insurer_003"                  │
│                                                                   │
│  ... (namespaces 004 → 014)                                      │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
         ↓
┌─────────────────────────────────────────────────────────────────┐
│           NEO4J GRAPH DATABASE                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Database: prudential_entities_001                               │
│  ├─ Nodes: 523 (products, coverages, conditions, benefits)      │
│  ├─ Edges: 1,247 (HAS_COVERAGE, COVERS_CONDITION, etc.)        │
│  └─ Property: insurer_id = "insurer_001" (all nodes)            │
│                                                                   │
│  Database: axa_entities_002                                      │
│  ├─ Nodes: 0 (pending, only 1 chunk extracted)                  │
│  └─ Property: insurer_id = "insurer_002"                        │
│                                                                   │
│  Database: thai_health_entities_003                              │
│  ├─ Nodes: 0 (pending)                                           │
│  └─ Property: insurer_id = "insurer_003"                        │
│                                                                   │
│  ... (prudential, axa, thai_health, ... generali_entities)       │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📊 Ingestion Flow (Phase 4)

```
Phase 2 Output: phase2_normalized.jsonl
├─ Chunk {insurer_id: "insurer_001", content, metadata}
├─ Chunk {insurer_id: "insurer_001", content, metadata}
├─ Chunk {insurer_id: "insurer_002", content, metadata}
├─ Chunk {insurer_id: "insurer_001", content, metadata}
└─ ...

                    ↓ Group by insurer_id

chunks_by_insurer = {
    "insurer_001": [chunk1, chunk2, chunk3, ...],  # 83 chunks
    "insurer_002": [chunk1, ...],                   # 1 chunk
    "insurer_003": [],                              # 0 chunks (pending)
}

                    ↓ Phase 4a: Ingest to Mimir

MIMIR Ingestion (4 parallel streams):
├─ insurer_001 → POST /api/ingest
│  ├─ collection_name: "insurance_products_001"
│  ├─ insurer_id: "insurer_001"
│  └─ Chunks: 83 ✅
├─ insurer_002 → POST /api/ingest
│  ├─ collection_name: "insurance_products_002"
│  ├─ insurer_id: "insurer_002"
│  └─ Chunks: 1 ✅
├─ insurer_003 → POST /api/ingest
│  ├─ collection_name: "insurance_products_003"
│  ├─ insurer_id: "insurer_003"
│  └─ Chunks: 0 (skipped)
└─ ... (insurer_004 → insurer_014)

                    ↓ Phase 4b: Generate Embeddings

HEIMDALL Embeddings (parallel per insurer):
├─ insurer_001: 83 texts → [embedding₁, embedding₂, ..., embedding₈₃]
├─ insurer_002: 1 text → [embedding₁]
└─ insurer_003: [] (empty)

                    ↓ Phase 4c: Index in Qdrant

QDRANT Indexing (separate namespaces):
├─ Namespace: 001
│  ├─ Collection: insurance_products_embeddings
│  ├─ Vectors: 83
│  └─ Upsert points with {id, vector, payload: {insurer_id: "insurer_001"}}
├─ Namespace: 002
│  ├─ Collection: insurance_products_embeddings
│  ├─ Vectors: 1
│  └─ Upsert points with {id, vector, payload: {insurer_id: "insurer_002"}}
└─ Namespace: 003
   ├─ Collection: insurance_products_embeddings
   └─ (empty)

                    ↓ Phase 4d: Index in Neo4j

NEO4J Indexing (separate databases):
├─ Database: prudential_entities_001
│  ├─ MERGE (Product:insurer_001) SET insurer_id = "insurer_001"
│  ├─ Nodes: 523
│  └─ Edges: 1,247
├─ Database: axa_entities_002
│  └─ (empty, 0 chunks)
└─ Database: thai_health_entities_003
   └─ (empty, pending)

                    ↓ Result

Phase 4 Complete:
{
  "status": "success",
  "mimir": {
    "insurer_001": {"collection": "insurance_products_001", "ingested": 83},
    "insurer_002": {"collection": "insurance_products_002", "ingested": 1},
    "insurer_003": {"collection": "insurance_products_003", "ingested": 0},
  },
  "qdrant": {
    "insurer_001": {"namespace": "001", "vector_count": 83},
    "insurer_002": {"namespace": "002", "vector_count": 1},
    "insurer_003": {"namespace": "003", "vector_count": 0},
  },
  "neo4j": {
    "insurer_001": {"database": "prudential_entities_001", "nodes": 523, "edges": 1247},
    "insurer_002": {"database": "axa_entities_002", "nodes": 0, "edges": 0},
    "insurer_003": {"database": "thai_health_entities_003", "nodes": 0, "edges": 0},
  },
}
```

---

## 🔍 Query Examples (Isolated by Insurer)

### **1. Search in Single Insurer (Mimir + Qdrant)**

```python
# ✅ CORRECT: Query Prudential data only
def search_prudential_health_plans():
    result = mimir.search(
        query="health insurance plans",
        filters={
            "insurer_id": "insurer_001",  # ISOLATED
        },
        namespace="001",  # QDRANT namespace
        top_k=10,
    )
    return result

# Result:
# [
#   {id: "chunk_001", score: 0.92, content: "PRU Mao Mao Double Sure...", insurer_id: "insurer_001"},
#   {id: "chunk_002", score: 0.87, content: "PRUBetter Care...", insurer_id: "insurer_001"},
#   ...
# ]
```

### **2. Search Across Multiple Insurers (Explicit)**

```python
# ✅ CORRECT: Cross-insurer search (explicit)
def search_all_critical_illness():
    result = mimir.search(
        query="critical illness coverage",
        filters={
            "insurer_id": {
                "$in": ["insurer_001", "insurer_002", "insurer_006"]  # Explicit list
            }
        },
        top_k=10,
    )
    return result

# Result: Mixed results but all tagged with insurer_id
```

### **3. Entity Graph Query (Neo4j, Single Insurer)**

```python
# ✅ CORRECT: Query Prudential's entity graph only
def find_critical_illness_coverage():
    query = """
    MATCH (product:Product {insurer_id: "insurer_001"})
          -[:HAS_COVERAGE]->
          (coverage:Coverage)
          -[:COVERS_CONDITION]->
          (condition:Condition)
    WHERE condition.name CONTAINS "Critical Illness"
    RETURN product.name, coverage.name, condition.name
    """
    # Connect to: bolt://localhost:7687/prudential_entities_001
    result = neo4j.run(query, database="prudential_entities_001")
    return result

# Result:
# [
#   {product: "PRU Mao Mao Double Sure", coverage: "Critical Illness Protection", condition: "Cancer"},
#   {product: "PRUBetter Care-IPD", coverage: "...", condition: "..."},
#   ...
# ]
```

### **4. ❌ WRONG: Mixed Data (Without Filter)**

```python
# ❌ WRONG: No insurer_id filter = potential data mixing
result = mimir.search(
    query="health insurance",
    top_k=10,
    # Missing: filters={"insurer_id": "..."}
)
# Could return mixed data from multiple insurers!

# ❌ WRONG: Querying multiple namespaces without isolation
result = qdrant.search(
    collection="insurance_products_embeddings",
    vector=embedding,
    limit=10,
    # Missing: filter={"insurer_id": {"$eq": "insurer_001"}}
)
# Could return mixed vectors from multiple insurers!
```

---

## 📋 Implementation Checklist

### **Phase 4a: Mimir Ingestion**
- ✅ Group chunks by `insurer_id` before ingestion
- ✅ Create separate `collection_name` per insurer (e.g., `insurance_products_001`)
- ✅ Tag each chunk with `insurer_id` in metadata
- ✅ Verify ingestion per collection

### **Phase 4b: Embedding Generation**
- ✅ Process texts per insurer separately
- ✅ Each insurer gets isolated embedding batch
- ✅ No mixing of embedding vectors across insurers

### **Phase 4c: Qdrant Indexing**
- ✅ Use separate namespaces per insurer (e.g., `001`, `002`, `003`)
- ✅ Add `insurer_id` to every vector payload
- ✅ Index only vectors for that insurer into that namespace
- ✅ Verify namespace isolation

### **Phase 4d: Neo4j Ingestion**
- ✅ Create separate database per insurer (e.g., `prudential_entities_001`)
- ✅ Add `insurer_id` property to all nodes
- ✅ All relationships must be within same insurer's database
- ✅ Verify database isolation

### **Phase 5: Validation (Query Filtering)**
- ✅ ALL queries must include `insurer_id` filter
- ✅ Test single-insurer search
- ✅ Test cross-insurer search (explicit list)
- ✅ Verify no data leakage between insurers

---

## 📊 Metadata Schema (Per Chunk)

```json
{
  "source_id": "url_insurer_001_health_0",
  "content": "PRU Mao Mao Double Sure covers...",
  "metadata": {
    "source_url": "https://prudential.co.th/en/products/health/",
    "document_type": "product_catalog",
    "language": "en",
    "extraction_date": "2026-05-16",
    "vendor": "VENDOR_INSURANCE_001",
    "insurer_id": "insurer_001",  # KEY: Isolation marker
    "collection": "insurance_products_001",  # KEY: Isolated collection
    "tenant_id": "asgard_insurance",
    "schema_version": "2.1.0",
    "compliance_status": "approved",
  },
  "insurer_id": "insurer_001",  # Top-level field for queries
  "chunk_index": 0,
  "tokens": 156,
  "language": "en",
}
```

---

## 🚀 Execution Command

```bash
# Run Phase 4 with insurer isolation
python main.py --phase 4

# Or directly:
python -c "
from insurance_ingestion_s2.core import PipelineConfig
from insurance_ingestion_s2.phases.phase4_ingestion import run_phase4_isolated

config = PipelineConfig()
result = run_phase4_isolated(
    config.output_dir / 'phase2_normalized.jsonl',
    config.output_dir / 'phase3_entities.jsonl',
    config,
)
print(result)
"
```

---

## ✅ Verification Checklist

After Phase 4 completes:

```bash
# 1. Verify Mimir collections per insurer
curl http://localhost:8000/api/collections | grep "insurance_products_"

# 2. Verify Qdrant namespaces
curl http://localhost:6333/collections/insurance_products_embeddings

# 3. Verify Neo4j databases
curl http://localhost:7687 -c "SHOW DATABASES;"

# 4. Test isolated query
curl http://localhost:8000/api/search \
  -d '{"query":"health insurance", "insurer_id":"insurer_001"}'

# 5. Verify no cross-insurer data
curl http://localhost:8000/api/search \
  -d '{"query":"health insurance", "insurer_id":"insurer_002"}' \
  # Should return ONLY insurer_002 data, not insurer_001
```

---

## 📝 Notes

- **Data Size:** ~1.34M tokens total, split across 14 insurers
- **Isolation Level:** Complete (Database, Collection, Namespace, Property-level)
- **Query Safety:** Requires explicit `insurer_id` filter (enforced in Phase 5)
- **Future Scaling:** Can add insurer_015, insurer_016, etc. without schema changes
