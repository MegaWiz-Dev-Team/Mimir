# Product Active Period Architecture
## Timeline Filtering + Historical Data

---

## 📅 Active Period Schema

```json
{
  "chunk_id": "chunk_001",
  "insurer_id": "insurer_001",
  "product_type": "health",
  "channel": "direct",
  
  ┌─────── ACTIVE PERIOD (NEW) ───────┐
  "product_name": "PRU Mao Mao Double Sure",
  "product_version": "2.0",
  "product_launch_date": "2020-01-15",    ← When product launched
  "product_end_date": null,               ← null = still active
  "is_active": true,                      ← Derived: current date check
  "status": "active",                     ← active | discontinued | archived | sunset
  └─────────────────────────────────────┘
  
  "content": "PRU Mao Mao Double Sure covers hospitalization...",
  "language": "en",
  "metadata": {
    "product_name": "PRU Mao Mao Double Sure",
    "product_version": "2.0",
    "product_launch_date": "2020-01-15",
    "product_end_date": null,
    "is_active": true,
    "status": "active",
  }
}
```

---

## ⏱️ Product Status Values

| Status | Meaning | Filter | Use Case |
|--------|---------|--------|----------|
| **active** | Selling now | `is_active: true` | Current offerings |
| **discontinued** | Stopped selling, kept for history | `status: "discontinued"` | Historical analysis |
| **archived** | Old version, new version available | `status: "archived"` | Version tracking |
| **sunset** | Phasing out, deadline approaching | `status: "sunset"` | Migration planning |
| **planned** | Not yet launched | `product_launch_date > today` | Upcoming products |

---

## 🔍 Query Patterns (With Active Period)

### **Pattern 1: Only Current Products (Active Now)**

```python
# Customer wants to buy NOW
result = mimir.search(
    query="health insurance",
    filters={
        "insurer_id": "insurer_001",
        "product_type": "health",
        "is_active": True,              # ✅ ONLY ACTIVE PRODUCTS
    }
)
# Result: PRU health plans available TODAY
# Excludes: Discontinued, archived, planned products
```

### **Pattern 2: Historical Analysis (Date Range)**

```python
# Market analysis: What was available in 2022?
result = mimir.search(
    query="health insurance",
    filters={
        "insurer_id": "insurer_001",
        "product_type": "health",
        "date": {
            "gte": "2022-01-01",          # ✅ As of January 1, 2022
            "lte": "2022-12-31"           # ✅ Through December 31, 2022
        }
    }
)
# Result: Products that were active anytime in 2022
```

### **Pattern 3: Discontinued Products**

```python
# Legacy support: Find old products for existing customers
result = mimir.search(
    query="critical illness",
    filters={
        "insurer_id": "insurer_001",
        "product_type": "health",
        "status": "discontinued",        # ✅ ONLY DISCONTINUED
    }
)
# Result: PRU health plans that are no longer sold
```

### **Pattern 4: Upcoming Products (Planned)**

```python
# Preview: What's launching soon?
result = mimir.search(
    query="investment",
    filters={
        "insurer_id": "insurer_001",
        "product_type": "investment",
        "status": "planned",             # ✅ PLANNED
        "product_launch_date": {
            "gte": "2026-06-01",
            "lte": "2026-12-31"
        }
    }
)
# Result: Investment products launching in H2 2026
```

### **Pattern 5: Version Comparison**

```python
# Compare product versions over time
result = mimir.search(
    query="coverage limits",
    filters={
        "insurer_id": "insurer_001",
        "product_type": "health",
        "product_name": "PRU Mao Mao Double Sure",
    },
    group_by="product_version"
)
# Result: [
#   {
#     product_version: "1.0",
#     product_launch_date: "2018-06-01",
#     product_end_date: "2020-01-14",
#     status: "archived",
#     content: "Old version...",
#   },
#   {
#     product_version: "2.0",
#     product_launch_date: "2020-01-15",
#     product_end_date: null,
#     status: "active",
#     is_active: true,
#     content: "Enhanced version...",
#   },
# ]
```

### **Pattern 6: Sunset/Deprecation Notice**

```python
# Find products being phased out soon
result = mimir.search(
    query="insurance",
    filters={
        "insurer_id": "insurer_001",
        "status": "sunset",
        "product_end_date": {
            "gte": "2026-06-01",         # ✅ Ending within 6 months
            "lte": "2026-12-31"
        }
    },
    sort_by="product_end_date"
)
# Result: Products with sunset dates, ordered by end date
```

### **Pattern 7: Cross-Insurer Current Offerings**

```python
# Compare what all insurers offer TODAY
result = mimir.search(
    query="critical illness",
    filters={
        "insurer_id": {"$in": ["001", "002", "006"]},
        "product_type": "health",
        "is_active": True,              # ✅ ACTIVE TODAY
    },
    group_by="insurer_id"
)
# Result: Current health plans from 3 insurers
# Automatically filters out discontinued products
```

---

## 📊 Real Example: PRU Mao Mao Evolution

```
Timeline:

2018-06-01 ─ PRU Mao Mao v1.0 LAUNCHED
│            (Old version with limited coverage)
│            is_active: true
│            status: "active"
│
│
2020-01-14 ─ PRU Mao Mao v1.0 DISCONTINUED
│            (Replaced by v2.0)
│            is_active: false
│            status: "archived"
│            product_end_date: 2020-01-14
│
2020-01-15 ─ PRU Mao Mao v2.0 LAUNCHED
│            (Enhanced with more benefits)
│            is_active: true
│            status: "active"
│            product_launch_date: 2020-01-15
│
│
2024-03-01 ─ PRU Mao Mao v2.1 LAUNCHED
│            (Minor updates)
│            is_active: true
│            status: "active"
│
│
2025-06-01 ─ SUNSET NOTICE: v2.0 ENDING
│            status: "sunset"
│            product_end_date: 2025-12-31
│
│
2025-12-31 ─ PRU Mao Mao v2.0 DISCONTINUED
│            (Customers migrated to v2.1)
│            is_active: false
│            status: "discontinued"
│            product_end_date: 2025-12-31
│
│
TODAY ───── PRU Mao Mao v2.1 CURRENT
            (Active product)
            is_active: true
            status: "active"

DATABASE STRUCTURE:

Chunk 1: PRU Mao Mao v1.0
├─ product_version: "1.0"
├─ product_launch_date: "2018-06-01"
├─ product_end_date: "2020-01-14"
├─ status: "archived"
├─ is_active: false
└─ content: "Old version coverage details..."

Chunk 2: PRU Mao Mao v2.0
├─ product_version: "2.0"
├─ product_launch_date: "2020-01-15"
├─ product_end_date: null (was active until)
├─ status: "discontinued" (today's date > sunset date)
├─ is_active: false
└─ content: "v2.0 coverage details..."

Chunk 3: PRU Mao Mao v2.1
├─ product_version: "2.1"
├─ product_launch_date: "2024-03-01"
├─ product_end_date: null
├─ status: "active"
├─ is_active: true
└─ content: "Latest version with enhancements..."

QUERIES:

1. What's the current PRU Mao Mao?
   filters={is_active: true}
   Result: v2.1 only ✅

2. Show product history
   filters={product_name: "PRU Mao Mao"}
   sort_by="product_launch_date"
   Result: v1.0 → v2.0 → v2.1 timeline ✅

3. Legacy support: find v2.0 docs
   filters={
     product_name: "PRU Mao Mao",
     product_version: "2.0"
   }
   Result: v2.0 archived docs ✅
```

---

## 🔐 Query Safety with Active Period

### **✅ SAFE: Explicit Status**
```python
filters={
    "insurer_id": "insurer_001",
    "is_active": True,           # ✅ Explicit
    "status": "active",          # ✅ Clear intent
}
```

### **✅ SAFE: Historical with Date Range**
```python
filters={
    "insurer_id": {"$in": ["001", "002", "006"]},
    "date": {
        "gte": "2022-01-01",     # ✅ Explicit date range
        "lte": "2022-12-31"
    }
}
```

### **❌ UNSAFE: Ambiguous Time**
```python
filters={
    "product_type": "health",
    # Missing: is_active? status? date range?
    # Could return: active, discontinued, archived, planned
}
```

---

## 📋 Updated Chunk Schema

```python
@dataclass
class Chunk:
    source_id: str
    content: str
    metadata: dict
    
    # Level 1: Insurer (Isolated)
    insurer_id: str = "insurer_001"
    
    # Level 2: Product Classification
    product_type: str = "health"          # health, life, savings, investment
    product_name: str = ""                # "PRU Mao Mao Double Sure"
    product_version: str = "1.0"          # "1.0", "2.0", "2.1"
    
    # Level 3: Distribution Channel
    channel: str = "direct"               # direct, uob, ttb, cimb, broker
    
    # Level 4: ACTIVE PERIOD (NEW)
    product_launch_date: str = "2020-01-15"  # ISO date format
    product_end_date: Optional[str] = None   # null = still active
    is_active: bool = True                   # Derived: today check
    status: str = "active"                   # active, discontinued, archived, sunset, planned
    
    # Other
    language: str = "en"                  # en, th, bi
    source_type: str = "url"              # url, upload, pdf, docx, ocr
    
    def to_jsonl(self) -> str:
        return json.dumps({
            "source_id": self.source_id,
            "content": self.content,
            "insurer_id": self.insurer_id,
            "product_type": self.product_type,
            "product_name": self.product_name,
            "product_version": self.product_version,
            "channel": self.channel,
            "product_launch_date": self.product_launch_date,
            "product_end_date": self.product_end_date,
            "is_active": self.is_active,
            "status": self.status,
            "language": self.language,
            "metadata": {
                **self.metadata,
                "product_launch_date": self.product_launch_date,
                "product_end_date": self.product_end_date,
                "is_active": self.is_active,
                "status": self.status,
            }
        })
```

---

## 🗓️ Qdrant Indexes (With Active Period)

```python
# Create payload indexes for filtering
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="insurer_id",
    field_type="keyword",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="product_type",
    field_type="keyword",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="product_name",
    field_type="text",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="product_version",
    field_type="keyword",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="channel",
    field_type="keyword",
)

# NEW: Date range indexes
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="product_launch_date",
    field_type="datetime",  # For range queries
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="product_end_date",
    field_type="datetime",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="is_active",
    field_type="bool",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="status",
    field_type="keyword",
)
```

---

## 📊 Use Cases

### **1. Customer Shopping (Real-Time)**
```python
# Show only products I can buy TODAY
filters={
    "insurer_id": "insurer_001",
    "is_active": True,  # ← MUST BE TRUE
}
```

### **2. Customer Support (Legacy)**
```python
# Help customer understand old PRU Mao Mao v2.0 (discontinued)
filters={
    "product_name": "PRU Mao Mao",
    "product_version": "2.0",
    "status": "discontinued",
}
```

### **3. Compliance (Archive)**
```python
# Audit historical product offerings
filters={
    "insurer_id": "insurer_001",
    "date": {"gte": "2023-01-01", "lte": "2023-12-31"},
}
# Returns all products active at any point in 2023
```

### **4. Product Management (Timeline)**
```python
# Show product lifecycle
filters={
    "insurer_id": "insurer_001",
    "product_name": "PRU Mao Mao",
},
sort_by="product_launch_date"
# Result: v1.0 (2018-06-01) → v2.0 (2020-01-15) → v2.1 (2024-03-01)
```

### **5. Migration Planning (Sunset)**
```python
# Find products being discontinued
filters={
    "insurer_id": "insurer_001",
    "status": "sunset",
    "product_end_date": {
        "lte": "2026-12-31"  # Ending this year
    }
}
```

---

## 🔄 Implementation Steps

### **Phase 1: Schema Update**
- [ ] Add product_launch_date to Chunk
- [ ] Add product_end_date to Chunk
- [ ] Add product_version to Chunk
- [ ] Add product_name to Chunk
- [ ] Add is_active (computed field)
- [ ] Add status enum

### **Phase 2: Extraction (Phase 1)**
- [ ] Extract product_launch_date from source
- [ ] Extract product_end_date if available
- [ ] Extract product_version (if in docs)
- [ ] Auto-calculate is_active

### **Phase 3: Ingestion (Phase 4)**
- [ ] Create date indexes in Qdrant
- [ ] Create status indexes in Qdrant
- [ ] Ingest with active period metadata

### **Phase 4: Query Layer (Phase 5)**
- [ ] Add date range filtering
- [ ] Add status-based filtering
- [ ] Add is_active default filtering
- [ ] Validate temporal queries

### **Phase 5: Analytics (Phase 6)**
- [ ] Product timeline analysis
- [ ] Version comparison reports
- [ ] Sunset/deprecation tracking
- [ ] Historical trend analysis

---

## ✅ Completion Checklist

| Feature | Status |
|---------|--------|
| product_launch_date field | ⏳ Ready |
| product_end_date field | ⏳ Ready |
| product_version tracking | ⏳ Ready |
| product_name field | ⏳ Ready |
| is_active computed field | ⏳ Ready |
| status enum (active/discontinued/archived/sunset) | ⏳ Ready |
| Date range filtering | ⏳ Ready |
| Status-based filtering | ⏳ Ready |
| Temporal query examples | ⏳ Ready |
| Historical data support | ⏳ Ready |
