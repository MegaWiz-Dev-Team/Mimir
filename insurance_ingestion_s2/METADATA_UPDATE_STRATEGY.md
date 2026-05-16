# Metadata Update Strategy
## Modifying Filter Metadata After Ingestion

---

## 📋 Overview

When chunks are already ingested into Mimir, Qdrant, and Neo4j, filter metadata (product_type, channel, product_launch_date, status, etc.) becomes immutable across three storage layers. This document describes how to retroactively update filter metadata while maintaining consistency.

**Scenarios:**
1. **Product metadata correction** - Launch date entered wrong, need to fix
2. **Status transition** - Product moves from "active" → "sunset" → "discontinued"
3. **Version classification** - Retrospectively tag product versions
4. **Channel reclassification** - Product channel changed (direct → uob)
5. **Bulk classification** - Add product metadata to previously untagged data

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│ PHASE 4: Ingestion (Initial State)                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│ Chunk: {                                                          │
│   source_id: "chunk_001",                                         │
│   content: "PRU Mao Mao...",                                      │
│   product_launch_date: "2020-01-15",  ← Filter metadata          │
│   status: "active",                   ← Immutable across layers  │
│ }                                                                 │
│                                                                   │
│     ↓ Ingest                ↓ Embed                ↓ Graph        │
│                                                                   │
│  MIMIR                     QDRANT                  NEO4J          │
│  ├─ Chunk 1               ├─ Vector 1            ├─ Product Node │
│  │  metadata: {           │  payload: {          │  properties: { │
│  │    product_launch..    │    product_launch..  │    product_l..│
│  │    status: "active"    │    status: "active"  │    status: ... │
│  │  }                     │  }                   │  }            │
│  └─ Chunk 2               └─ Vector 2            └─ Entity nodes │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ UPDATE: Change status "active" → "discontinued"                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  UPDATE REQUEST:                                                  │
│  {                                                                │
│    "source_ids": ["chunk_001", "chunk_002"],                     │
│    "updates": {                                                   │
│      "status": "discontinued",                                    │
│      "product_end_date": "2025-12-31"                            │
│    }                                                              │
│  }                                                                │
│                                                                   │
│     ↓ Apply to all layers                                         │
│                                                                   │
│  MIMIR                     QDRANT                  NEO4J          │
│  UPDATE chunk SET          UPDATE payload          MATCH Product │
│  metadata = {...,          SET {...,              SET status =.. │
│    status: "disc.."        status: "disc.."     RETURN status   │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ RESULT: All layers updated consistently                          │
├─────────────────────────────────────────────────────────────────┤
│  MIMIR                     QDRANT                  NEO4J          │
│  status: "discontinued" ✅ status: "disc." ✅ status: "disc." ✅ │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🔄 Update Patterns

### Pattern 1: Update Single Chunk Metadata

**Use case:** Fix a single product's metadata (typo correction, launch date)

```python
# Strategy: Update all 3 layers atomically
def update_chunk_metadata(
    chunk_id: str,
    updates: dict,  # {"product_launch_date": "2020-01-15", ...}
    mimir_client,
    qdrant_client,
    neo4j_driver,
) -> dict:
    """
    Update filter metadata for a single chunk across Mimir, Qdrant, Neo4j.
    
    Args:
        chunk_id: source_id to update
        updates: dict of fields to update
        
    Returns:
        {
            "status": "success",
            "mimir": {"updated": True, "chunk_id": "..."},
            "qdrant": {"updated": True, "vectors": 1},
            "neo4j": {"updated": True, "nodes": 5},
        }
    """
    results = {"status": "success"}
    
    # Step 1: Update Mimir (chunk metadata)
    try:
        mimir_result = mimir_client.update_chunk_metadata(
            chunk_id=chunk_id,
            metadata_updates=updates,
        )
        results["mimir"] = mimir_result
    except Exception as e:
        results["mimir"] = {"error": str(e)}
        results["status"] = "partial"
    
    # Step 2: Update Qdrant (vector payloads)
    try:
        qdrant_result = qdrant_client.update_payload(
            collection_name="insurance_products_embeddings",
            payload_updates=updates,
            filter={"source_id": {"$eq": chunk_id}},
        )
        results["qdrant"] = qdrant_result
    except Exception as e:
        results["qdrant"] = {"error": str(e)}
        results["status"] = "partial"
    
    # Step 3: Update Neo4j (entity properties)
    try:
        neo4j_result = neo4j_driver.execute_write(
            """
            MATCH (e:Entity {source_id: $chunk_id})
            SET e += $updates
            RETURN count(e) as updated_count
            """,
            chunk_id=chunk_id,
            updates=updates,
        )
        results["neo4j"] = {"updated": True, "nodes": neo4j_result}
    except Exception as e:
        results["neo4j"] = {"error": str(e)}
        results["status"] = "partial"
    
    return results
```

### Pattern 2: Batch Update by Insurer + Filter

**Use case:** Update all health products from Prudential (insurer_001) → status "discontinued"

```python
def batch_update_by_filter(
    filters: dict,  # {"insurer_id": "insurer_001", "product_type": "health"}
    updates: dict,  # {"status": "discontinued"}
    mimir_client,
    qdrant_client,
    neo4j_driver,
) -> dict:
    """
    Update metadata for ALL chunks matching filters.
    
    Args:
        filters: Chunk filters to match (insurer_id, product_type, channel, etc.)
        updates: Fields to update
        
    Returns:
        {
            "status": "success",
            "mimir": {"matched": 47, "updated": 47},
            "qdrant": {"matched": 47, "updated": 47},
            "neo4j": {"matched": 235, "updated": 235},  # More entities than chunks
        }
    """
    results = {"status": "success"}
    
    # Step 1: Find all chunk IDs matching filters
    chunk_ids = mimir_client.search(
        filters=filters,
        limit=10000,
    )
    chunk_ids = [hit["source_id"] for hit in chunk_ids]
    
    if not chunk_ids:
        return {"status": "no_matches", "matched": 0}
    
    # Step 2: Update Mimir (all matching chunks)
    try:
        mimir_result = mimir_client.batch_update_metadata(
            chunk_ids=chunk_ids,
            metadata_updates=updates,
        )
        results["mimir"] = {"matched": len(chunk_ids), "updated": mimir_result["count"]}
    except Exception as e:
        results["mimir"] = {"error": str(e)}
        results["status"] = "partial"
    
    # Step 3: Update Qdrant (vectors matching chunk IDs)
    try:
        # Convert filters to Qdrant payload format
        qdrant_filter = {
            "must": [
                {"key": k, "match": {"value": v}}
                for k, v in filters.items()
            ]
        }
        qdrant_result = qdrant_client.update_payload(
            collection_name="insurance_products_embeddings",
            payload_updates=updates,
            filter=qdrant_filter,
        )
        results["qdrant"] = {
            "matched": qdrant_result["updated"],
            "updated": qdrant_result["updated"],
        }
    except Exception as e:
        results["qdrant"] = {"error": str(e)}
        results["status"] = "partial"
    
    # Step 4: Update Neo4j (all entities from matching chunks)
    try:
        cypher_filter_conditions = " AND ".join(
            f"e.{k} = ${k}" for k in filters.keys()
        )
        neo4j_result = neo4j_driver.execute_write(
            f"""
            MATCH (e:Entity)
            WHERE {cypher_filter_conditions}
            SET e += $updates
            RETURN count(e) as updated_count
            """,
            **filters,
            updates=updates,
        )
        results["neo4j"] = {"matched": neo4j_result, "updated": neo4j_result}
    except Exception as e:
        results["neo4j"] = {"error": str(e)}
        results["status"] = "partial"
    
    return results
```

### Pattern 3: Product Lifecycle Transition

**Use case:** Product transitions from "active" → "sunset" (end date approaching) → "discontinued"

```python
def transition_product_status(
    product_name: str,
    insurer_id: str,
    from_status: str,
    to_status: str,
    end_date: Optional[str] = None,
    mimir_client=None,
    qdrant_client=None,
    neo4j_driver=None,
) -> dict:
    """
    Transition product through lifecycle stages with validation.
    
    Status progression: active → sunset → discontinued
    (or active → archived if version replaced)
    
    Args:
        product_name: e.g., "PRU Mao Mao Double Sure"
        insurer_id: e.g., "insurer_001"
        from_status: current status (for validation)
        to_status: new status
        end_date: ISO date when product ends (for "discontinued")
        
    Returns:
        {
            "status": "success",
            "transition": "active → discontinued",
            "chunks_updated": 15,
            "effective_date": "2025-12-31",
        }
    """
    # Validate transition
    valid_transitions = {
        "active": ["sunset", "archived"],
        "sunset": ["discontinued"],
        "discontinued": [],  # Terminal state
        "archived": [],      # Terminal state
        "planned": ["active", "discontinued"],
    }
    
    if to_status not in valid_transitions.get(from_status, []):
        return {
            "status": "error",
            "message": f"Invalid transition: {from_status} → {to_status}",
        }
    
    updates = {"status": to_status}
    if to_status in ["discontinued", "sunset"] and end_date:
        updates["product_end_date"] = end_date
    
    # Update all chunks for this product
    result = batch_update_by_filter(
        filters={
            "insurer_id": insurer_id,
            "product_name": product_name,
        },
        updates=updates,
        mimir_client=mimir_client,
        qdrant_client=qdrant_client,
        neo4j_driver=neo4j_driver,
    )
    
    result["transition"] = f"{from_status} → {to_status}"
    result["effective_date"] = end_date
    
    return result
```

---

## 📊 Layer-Specific Update Strategies

### 1️⃣ Mimir: Update Chunk Metadata

**API Endpoint:** `PATCH /api/chunks/{chunk_id}/metadata`

```bash
# Update single chunk
curl -X PATCH http://localhost:8000/api/chunks/chunk_001/metadata \
  -H "Content-Type: application/json" \
  -d '{
    "product_launch_date": "2020-01-15",
    "status": "discontinued"
  }'

# Batch update (new endpoint)
curl -X PATCH http://localhost:8000/api/chunks/metadata/batch \
  -H "Content-Type: application/json" \
  -d '{
    "chunk_ids": ["chunk_001", "chunk_002", "chunk_003"],
    "updates": {
      "status": "discontinued",
      "product_end_date": "2025-12-31"
    }
  }'
```

**Implementation (Python):**

```python
def update_chunk_metadata(collection_name: str, chunk_id: str, updates: dict) -> bool:
    """Update chunk metadata in Mimir."""
    # SQL (if using PostgreSQL backend)
    query = """
    UPDATE chunks
    SET metadata = jsonb_set(metadata, '{...}', to_jsonb($updates))
    WHERE source_id = $chunk_id
      AND collection = $collection_name
    """
    # Execute and return success
    return execute_query(query, chunk_id=chunk_id, updates=updates)
```

### 2️⃣ Qdrant: Update Vector Payloads

**API Endpoint:** `POST /collections/{collection}/points/update-payload`

```bash
# Update vectors matching filter
curl -X POST http://localhost:6333/collections/insurance_products_embeddings/points/update-payload \
  -H "Content-Type: application/json" \
  -d '{
    "payload": {
      "status": "discontinued",
      "product_end_date": "2025-12-31"
    },
    "filter": {
      "must": [
        {
          "key": "insurer_id",
          "match": {
            "value": "insurer_001"
          }
        },
        {
          "key": "status",
          "match": {
            "value": "active"
          }
        }
      ]
    }
  }'
```

**Implementation (Python):**

```python
from qdrant_client import QdrantClient
from qdrant_client.models import FieldCondition, MatchValue, HasPayloadCondition

client = QdrantClient("localhost", port=6333)

# Update payload with filter
client.set_payload(
    collection_name="insurance_products_embeddings",
    payload={
        "status": "discontinued",
        "product_end_date": "2025-12-31",
    },
    points_selector={
        "filter": {
            "must": [
                FieldCondition(
                    key="insurer_id",
                    match=MatchValue(value="insurer_001"),
                ),
                FieldCondition(
                    key="status",
                    match=MatchValue(value="active"),
                ),
            ]
        }
    },
)
```

### 3️⃣ Neo4j: Update Entity Properties

**Cypher Query:**

```cypher
-- Update single node
MATCH (e:Entity {source_id: 'chunk_001'})
SET e.status = 'discontinued',
    e.product_end_date = '2025-12-31'
RETURN count(e) as updated

-- Batch update by filters
MATCH (e:Entity)
WHERE e.insurer_id = 'insurer_001'
  AND e.product_type = 'health'
  AND e.status = 'active'
SET e.status = 'discontinued',
    e.product_end_date = '2025-12-31'
RETURN count(e) as updated_count
```

**Implementation (Python):**

```python
from neo4j import GraphDatabase

driver = GraphDatabase.driver("bolt://localhost:7687", auth=("neo4j", "password"))

def update_entities_by_filter(filters: dict, updates: dict) -> int:
    """Update Neo4j entities matching filters."""
    with driver.session(database="prudential_entities_001") as session:
        where_clause = " AND ".join(f"e.{k} = ${k}" for k in filters.keys())
        query = f"""
        MATCH (e:Entity)
        WHERE {where_clause}
        SET e += $updates
        RETURN count(e) as updated_count
        """
        result = session.run(query, **filters, updates=updates)
        return result.single()["updated_count"]
```

---

## ✅ Consistency Guarantees

### Atomic Updates (Single Chunk)

When updating a single chunk across all layers:

```python
def atomic_update(chunk_id: str, updates: dict) -> dict:
    """
    Update order:
    1. MIMIR (source of truth)
    2. QDRANT (derived from mimir content)
    3. NEO4J (derived from entities)
    
    If any step fails: ROLLBACK all previous steps
    """
    
    # Backup original state
    original = mimir.get_chunk(chunk_id)
    
    try:
        # Step 1: Update Mimir
        mimir.update_metadata(chunk_id, updates)
        
        # Step 2: Update Qdrant
        qdrant.update_payload(chunk_id, updates)
        
        # Step 3: Update Neo4j
        neo4j.update_entities(chunk_id, updates)
        
        return {"status": "success"}
        
    except Exception as e:
        # Rollback all changes
        mimir.restore(chunk_id, original)
        qdrant.restore(chunk_id, original)
        neo4j.restore(chunk_id, original)
        
        return {"status": "error", "error": str(e)}
```

### Batch Updates (Partial Failure Handling)

For batch updates, some chunks may succeed while others fail:

```python
def batch_update_with_partial_failure(
    chunk_ids: list,
    updates: dict,
) -> dict:
    """
    Update chunks individually to isolate failures.
    """
    results = {
        "status": "partial",
        "successful": [],
        "failed": [],
    }
    
    for chunk_id in chunk_ids:
        try:
            atomic_update(chunk_id, updates)
            results["successful"].append(chunk_id)
        except Exception as e:
            results["failed"].append({
                "chunk_id": chunk_id,
                "error": str(e),
            })
    
    # Determine overall status
    if not results["failed"]:
        results["status"] = "success"
    
    return results
```

---

## 🗓️ Metadata Versioning

To track metadata changes over time:

```python
@dataclass
class MetadataAuditLog:
    """Track all metadata changes."""
    chunk_id: str
    timestamp: str  # ISO datetime
    old_values: dict
    new_values: dict
    updated_by: str  # User email
    reason: str  # Why changed (e.g., "Status correction", "Product lifecycle")
    layers_updated: list[str]  # ["mimir", "qdrant", "neo4j"]
    status: str  # "success", "partial", "failed"

def log_metadata_update(
    chunk_id: str,
    old_values: dict,
    new_values: dict,
    reason: str,
    updated_by: str,
) -> bool:
    """Log metadata changes to audit trail."""
    log_entry = MetadataAuditLog(
        chunk_id=chunk_id,
        timestamp=datetime.utcnow().isoformat(),
        old_values=old_values,
        new_values=new_values,
        updated_by=updated_by,
        reason=reason,
        layers_updated=["mimir", "qdrant", "neo4j"],
        status="success",
    )
    
    # Store in audit table (PostgreSQL)
    db.insert("metadata_audit_log", log_entry.to_dict())
    return True
```

---

## 🚀 Migration Scenarios

### Scenario 1: Add Missing Product Metadata

**Problem:** Phase 1 extracted chunks without product names or launch dates. Now need to add them.

**Solution:** Bulk classification + batch update

```python
def backfill_product_metadata():
    """
    1. Classify all chunks with missing product_name/launch_date
    2. Extract from content using pattern matching or LLM
    3. Batch update all layers
    """
    
    # Find chunks with empty product_name
    chunks_to_update = mimir.search(
        filters={"product_name": ""},
        limit=10000,
    )
    
    # Classify each chunk
    for chunk in chunks_to_update:
        # Use LLM or pattern matching to extract product_name
        product_name = extract_product_name(chunk["content"])
        product_launch_date = extract_launch_date(chunk["content"])
        
        # Update single chunk
        update_chunk_metadata(
            chunk_id=chunk["source_id"],
            updates={
                "product_name": product_name,
                "product_launch_date": product_launch_date,
            },
        )
    
    print(f"✅ Updated {len(chunks_to_update)} chunks")
```

### Scenario 2: Reclassify Channels

**Problem:** All insurance products initially marked as "direct", but actually distributed via "uob", "ttb", "cimb" partners. Need to reclassify.

**Solution:** Map-and-update based on product name

```python
CHANNEL_MAP = {
    "PRU": "direct",  # Prudential direct
    "UOB Essential": "uob",  # UOB bancassurance
    "TTB Mhao Mhao": "ttb",  # TTB bancassurance
    "CIMB Thai": "cimb",  # CIMB bancassurance
}

def reclassify_channels():
    """Update channel classification based on product name."""
    
    for product_pattern, correct_channel in CHANNEL_MAP.items():
        # Find all chunks matching product pattern
        chunks = mimir.search(
            filters={"product_name": product_pattern},
            limit=10000,
        )
        
        # Batch update channel
        batch_update_by_filter(
            filters={"product_name": product_pattern},
            updates={"channel": correct_channel},
        )
        
        print(f"✅ Reclassified {len(chunks)} chunks to channel={correct_channel}")
```

### Scenario 3: Product Lifecycle Transition

**Problem:** Product PRU Mao Mao v2.0 reaches end-of-life on 2025-12-31. Need to transition status chain: active → sunset → discontinued

**Solution:** Scheduled transitions

```python
from datetime import datetime, timedelta

def schedule_product_transitions():
    """
    Automatically transition products through lifecycle.
    Run daily to update approaching sunset dates.
    """
    
    # Find products approaching end date
    products = mimir.search(
        filters={
            "status": "active",
            "product_end_date": {
                "gte": datetime.now().isoformat(),
                "lte": (datetime.now() + timedelta(days=30)).isoformat(),
            }
        }
    )
    
    for product in products:
        # Transition to "sunset" status
        transition_product_status(
            product_name=product["product_name"],
            insurer_id=product["insurer_id"],
            from_status="active",
            to_status="sunset",
            end_date=product["product_end_date"],
        )
        print(f"✅ {product['product_name']} transitioned to sunset")
```

---

## 🔍 Verification & Auditing

### Verify Consistency Across Layers

```python
def verify_metadata_consistency(chunk_id: str) -> dict:
    """Verify metadata is consistent across all 3 layers."""
    
    # Read from Mimir
    mimir_chunk = mimir.get_chunk(chunk_id)
    mimir_status = mimir_chunk["metadata"]["status"]
    
    # Read from Qdrant
    qdrant_vector = qdrant.get_point_by_id(chunk_id)
    qdrant_status = qdrant_vector["payload"]["status"]
    
    # Read from Neo4j
    neo4j_entity = neo4j.get_entity_by_source_id(chunk_id)
    neo4j_status = neo4j_entity["status"]
    
    # Check consistency
    consistent = (mimir_status == qdrant_status == neo4j_status)
    
    return {
        "chunk_id": chunk_id,
        "consistent": consistent,
        "mimir": mimir_status,
        "qdrant": qdrant_status,
        "neo4j": neo4j_status,
    }

# Check all chunks in a collection
def audit_collection_consistency(insurer_id: str) -> dict:
    """Check metadata consistency across all chunks in collection."""
    
    chunk_ids = mimir.search(
        filters={"insurer_id": insurer_id},
        limit=10000,
    )
    
    results = [verify_metadata_consistency(c["source_id"]) for c in chunk_ids]
    
    inconsistent = [r for r in results if not r["consistent"]]
    
    return {
        "total": len(results),
        "consistent": len(results) - len(inconsistent),
        "inconsistent": len(inconsistent),
        "issues": inconsistent,
    }
```

---

## 📝 Implementation Checklist

### Phase 4a: Add Update APIs

- [ ] Mimir: `PATCH /api/chunks/{id}/metadata` (single update)
- [ ] Mimir: `PATCH /api/chunks/metadata/batch` (batch update)
- [ ] Qdrant: Implement `update_payload()` with filters
- [ ] Neo4j: Implement Cypher batch update queries

### Phase 4b: Add Audit Logging

- [ ] Create `metadata_audit_log` table in PostgreSQL
- [ ] Log all updates with timestamp, user, reason
- [ ] Add `MetadataAuditLog` dataclass

### Phase 4c: Add Verification Tools

- [ ] Implement `verify_metadata_consistency()`
- [ ] Implement `audit_collection_consistency()`
- [ ] Add CLI commands for verification

### Phase 4d: Add Lifecycle Management

- [ ] Implement `transition_product_status()`
- [ ] Add validation for valid status transitions
- [ ] Create scheduled transition runner

---

## 🎯 Usage Examples

### Example 1: Fix Product Launch Date

```python
# User corrected the launch date for PRU Mao Mao
result = update_chunk_metadata(
    chunk_id="chunk_prudential_001",
    updates={
        "product_launch_date": "2020-01-15",  # Was: 2020-01-16
    },
)
# Result: Updated in Mimir, Qdrant, Neo4j ✅
```

### Example 2: Mark Product as Discontinued

```python
# PRU Mao Mao v2.0 end-of-life reached
result = transition_product_status(
    product_name="PRU Mao Mao Double Sure",
    insurer_id="insurer_001",
    from_status="sunset",
    to_status="discontinued",
    end_date="2025-12-31",
)
# Result: All 15 chunks updated across layers ✅
```

### Example 3: Bulk Reclassify Channel

```python
# All UOB products were marked as "direct", correct to "uob"
result = batch_update_by_filter(
    filters={
        "insurer_id": "insurer_001",
        "product_name": {"$regex": "UOB.*"},
    },
    updates={"channel": "uob"},
)
# Result: 23 chunks updated ✅
```

---

## ✅ Completion Checklist

| Item | Status |
|------|--------|
| Core.py: Add temporal fields to Chunk | ✅ Done |
| Metadata update patterns documented | ✅ Done |
| Layer-specific strategies defined | ✅ Done |
| Atomic update strategy designed | ✅ Done |
| Consistency verification tools outlined | ✅ Done |
| Migration scenarios documented | ✅ Done |
| Lifecycle management designed | ✅ Done |
| Audit logging strategy defined | ✅ Done |
| Implementation checklist created | ⏳ Ready |
