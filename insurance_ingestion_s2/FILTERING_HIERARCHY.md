# Hierarchical Filtering Architecture
## Product + Channel + Cross-Insurer Comparison

---

## 🏛️ Filter Hierarchy

```
┌─────────────────────────────────────────────────────────────┐
│               ASGARD INSURANCE PLATFORM                      │
│               (Tenant: asgard_insurance)                     │
└─────────────────────────────────────────────────────────────┘
                         ↓
        ┌────────────────┴────────────────┐
        ↓                                  ↓
┌──────────────────┐            ┌──────────────────┐
│  INSURER LEVEL   │            │  CROSS-INSURER   │
│  (Isolated)      │            │  COMPARISON      │
└────────┬─────────┘            └──────────────────┘
         ↓
    insurer_001
    insurer_002
    insurer_003
         ↓
    ┌────────────────────────────┐
    ↓      PRODUCT LEVEL         ↓
  ┌──────────────────────────────────┐
  │ health | life | savings | invest │
  └────────────────┬─────────────────┘
                   ↓
        ┌──────────────────────┐
        ↓    CHANNEL LEVEL     ↓
    ┌──────────────────────────────┐
    │ url | upload | bancassurance │
    └──────────────────────────────┘
```

---

## 📊 Data Schema with Hierarchical Filters

```json
{
  "chunk_id": "chunk_001",
  "insurer_id": "insurer_001",
  "product_type": "health",
  "channel": "url",
  "content": "PRU Mao Mao Double Sure covers hospitalization...",
  "metadata": {
    "source_url": "https://prudential.co.th/en/products/health/",
    "product_type": "health",
    "channel": "url",
    "insurer_id": "insurer_001",
    "document_type": "product_catalog",
    "language": "en",
  },
  "tokens": 156,
  "language": "en",
}
```

### **Filter Fields Added:**

| Field | Type | Values | Purpose |
|-------|------|--------|---------|
| `insurer_id` | string | insurer_001-014 | Isolate by company |
| `product_type` | string | health, life, savings, investment | Filter by product |
| `channel` | string | url, upload, bancassurance, broker | Filter by data source |
| `language` | string | en, th, bi | Filter by language |

---

## 🔍 Query Patterns (Hierarchical + Cross-Insurer)

### **Pattern 1: Single Insurer, Single Product**

```python
# Find all health insurance plans from Prudential
result = mimir.search(
    query="hospitalization coverage",
    filters={
        "insurer_id": "insurer_001",      # ✅ ISOLATED
        "product_type": "health",          # ✅ PRODUCT FILTER
    },
    namespace="001",
    top_k=10
)
# Result: PRU health products only
```

### **Pattern 2: Single Insurer, All Products**

```python
# Find all products from AXA (any type)
result = mimir.search(
    query="coverage plans",
    filters={
        "insurer_id": "insurer_002",      # ✅ ISOLATED
        # product_type: omitted (all products)
    },
    namespace="002",
    top_k=10
)
# Result: AXA health + life + savings products
```

### **Pattern 3: Single Insurer, Specific Distribution Channel**

```python
# Find health products from Prudential sold through UOB (bancassurance)
result = mimir.search(
    query="hospital benefits",
    filters={
        "insurer_id": "insurer_001",      # ✅ ISOLATED
        "product_type": "health",
        "channel": "uob",                 # ✅ DISTRIBUTION CHANNEL FILTER
    },
    namespace="001",
    top_k=10
)
# Result: PRU health plans distributed through UOB (e.g., UOB Essential Health)
```

### **Pattern 4: Cross-Insurer Product Comparison** ⭐

```python
# Compare health insurance plans across multiple insurers
result = mimir.search(
    query="critical illness coverage",
    filters={
        "insurer_id": {
            "$in": ["insurer_001", "insurer_002", "insurer_006"]  # ✅ EXPLICIT LIST
        },
        "product_type": "health",          # ✅ PRODUCT FILTER
    },
    top_k=10
)
# Result: [
#   {content: "PRU...", insurer_id: "insurer_001", product_type: "health"},
#   {content: "AXA...", insurer_id: "insurer_002", product_type: "health"},
#   {content: "AIA...", insurer_id: "insurer_006", product_type: "health"},
# ]
```

### **Pattern 5: Cross-Insurer Channel Comparison** ⭐

```python
# Compare products from URLs vs uploaded documents across 3 insurers
result = mimir.search(
    query="product details",
    filters={
        "insurer_id": {"$in": ["insurer_001", "insurer_002", "insurer_003"]},
        "channel": {"$in": ["url", "upload"]}  # ✅ COMPARE CHANNELS
    },
    top_k=10
)
# Result: Mixed data from 3 insurers, 2 channels, compare coverage
```

### **Pattern 6: Product Trend Analysis** ⭐

```python
# Find which products are most common across insurers
result = mimir.search(
    query="critical illness",
    filters={
        # No insurer_id = search ALL (for analysis only)
        "product_type": "health",          # ✅ PRODUCT FILTER
    },
    aggregation={
        "group_by": ["insurer_id", "product_type"],
        "count": True,
    },
    top_k=50
)
# Result: [
#   {insurer_id: "insurer_001", product_type: "health", count: 12, products: [...]},
#   {insurer_id: "insurer_002", product_type: "health", count: 8, products: [...]},
#   {insurer_id: "insurer_006", product_type: "health", count: 15, products: [...]},
# ]
```

### **Pattern 7: Language + Product + Insurer**

```python
# Find Thai-language health products from all insurers
result = mimir.search(
    query="ความคุ้มครองสุขภาพ",
    filters={
        # No insurer_id = all insurers
        "product_type": "health",
        "language": "th",                  # ✅ LANGUAGE FILTER
    },
    top_k=10
)
# Result: All Thai health insurance products (cross-insurer)
```

---

## 📋 Metadata Schema Update

### **Before (Current)**
```json
{
  "insurer_id": "insurer_001",
  "content": "...",
  "metadata": { ... }
}
```

### **After (With Hierarchical Filters)**
```json
{
  "insurer_id": "insurer_001",
  "product_type": "health",
  "channel": "url",
  "language": "en",
  "content": "...",
  "metadata": {
    "insurer_id": "insurer_001",
    "product_type": "health",
    "channel": "url",
    "source_url": "https://prudential.co.th/en/products/health/",
    "document_type": "product_catalog",
    "language": "en",
  }
}
```

---

## 🏢 Product Type Classification

### **Health Insurance**
- Hospitalization coverage
- Medical insurance
- Critical illness protection
- Medical check-up
- Emergency care

### **Life Insurance**
- Term life
- Whole life
- Endowment
- Annuity
- Investment-linked

### **Savings & Retirement**
- Savings plans
- Endowment plans
- Retirement plans
- Pension plans

### **Investment**
- Unit-linked insurance
- Investment-linked plans
- Wealth management

---

## 📡 Channel Classification (Distribution Channels)

### **Direct**
- Company's own website
- Company's own channels
- Direct sales team
- Company call center

### **UOB (Bancassurance Partner)**
- UOB Essential Health
- UOB co-branded products
- UOB branch distribution

### **TTB (Bancassurance Partner)**
- TTB Mhao Mhao Ultra Care
- TTB co-branded plans
- TTB branch distribution

### **CIMB (Bancassurance Partner)**
- CIMB Thai Cover Care Plus
- CIMB co-branded products
- CIMB branch distribution

### **Krungthai (Government)**
- Krungthai Bank distribution
- Government employee plans

### **Broker**
- Independent brokers
- Broker networks
- Aggregator platforms

### **Agent**
- Insurance agents
- Financial advisors
- Partner agents

---

## 🔐 Query Safety Rules

### **✅ SAFE: Insurer Isolated**
```python
filters={
    "insurer_id": "insurer_001",          # REQUIRED for single-insurer
    "product_type": "health",             # Optional
    "channel": "url",                     # Optional
}
```

### **✅ SAFE: Cross-Insurer Explicit**
```python
filters={
    "insurer_id": {"$in": ["insurer_001", "insurer_002"]},  # EXPLICIT LIST
    "product_type": "health",             # Optional
}
```

### **✅ SAFE: Cross-Insurer with Aggregation**
```python
filters={
    # Omitting insurer_id for analysis
    "product_type": "health",
},
aggregation={
    "group_by": ["insurer_id"],           # Must group by insurer
    "count": True,
}
```

### **❌ UNSAFE: Implicit Cross-Insurer**
```python
filters={
    "product_type": "health",
    # Missing insurer_id = undefined behavior!
}
# Could return mixed data without grouping
```

---

## 📊 Implementation: Updated Data Pipeline

### **Phase 1: Extraction (Enhanced)**

```python
for insurer_id, insurer_info in insurers.items():
    for url in insurer_info['urls']:
        chunks = extract_from_url(url)
        
        # Classify product type from URL
        if '/health/' in url:
            product_type = 'health'
        elif '/life/' in url:
            product_type = 'life'
        elif '/savings/' in url:
            product_type = 'savings'
        
        for chunk in chunks:
            chunk.product_type = product_type
            chunk.channel = 'url'
            chunk.insurer_id = insurer_id
```

### **Phase 4: Mimir Ingestion (Enhanced)**

```python
payload = {
    "tenant_id": "asgard_insurance",
    "collection_name": f"insurance_products_{insurer_id}",
    "chunks": [
        {
            "source_id": c.source_id,
            "content": c.content,
            "insurer_id": c.insurer_id,
            "product_type": c.product_type,  # NEW
            "channel": c.channel,            # NEW
            "language": c.language,
            "metadata": {...}
        }
    ]
}
```

### **Qdrant Index with Filters**

```python
# Create index with filter support
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="insurer_id",
    field_type="keyword",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="product_type",  # NEW
    field_type="keyword",
)
qdrant.create_payload_index(
    collection_name="insurance_products_embeddings",
    field_name="channel",       # NEW
    field_type="keyword",
)
```

---

## 📈 Query Examples by Use Case

### **Use Case 1: Customer Shopping (Insurer-Specific)**
```python
# Customer browsing Prudential website, looking for health plans
search(
    query="What health plans do you offer?",
    filters={"insurer_id": "insurer_001", "product_type": "health"},
)
# Result: Only PRU health products
```

### **Use Case 2: Comparison Shopping (Cross-Insurer)**
```python
# Customer comparing health insurance across brands
search(
    query="Critical illness coverage",
    filters={
        "insurer_id": {"$in": ["insurer_001", "insurer_002", "insurer_006"]},
        "product_type": "health",
    },
    group_by="insurer_id",
)
# Result: Top plans from each insurer
```

### **Use Case 3: Data Quality Check (Channel Comparison)**
```python
# Verify data consistency between website and uploaded documents
search(
    query="coverage limits",
    filters={
        "insurer_id": "insurer_001",
        "product_type": "health",
        "channel": {"$in": ["url", "upload"]},  # Compare both sources
    },
)
# Result: Same products from different channels for validation
```

### **Use Case 4: Market Analysis (All Data)**
```python
# Find market trends in critical illness coverage
search(
    query="critical illness 63 diseases",
    filters={
        "product_type": "health",  # Only health products
    },
    aggregation={
        "group_by": ["insurer_id", "channel"],
        "metrics": ["coverage_count", "premium_range"],
    },
)
# Result: Which insurers offer what, from which channels
```

---

## 🎯 Filter Combinations Matrix

| Scenario | Insurer | Product | Channel | Cross-Insurer | Isolation |
|----------|---------|---------|---------|---|---|
| Single product lookup | ✅ Required | Optional | Optional | ❌ No | Isolated |
| Compare plans A vs B | ✅ List | Optional | Optional | ✅ Explicit | Safe |
| All products (one insurer) | ✅ Required | ❌ Omit | Optional | ❌ No | Isolated |
| Product trends (all) | ❌ Omit | ✅ Required | Optional | ✅ Aggregated | Safe |
| Channel validation | ✅ Required | Optional | ✅ Required | ❌ No | Isolated |
| Language analysis | ❌ Omit | Optional | Optional | ✅ Aggregated | Safe |

---

## ✅ Implementation Roadmap

### **Phase 4a: Add Metadata Fields**
- [ ] Add `product_type` to chunk schema
- [ ] Add `channel` to chunk schema
- [ ] Classify all chunks during extraction

### **Phase 4b: Create Indexes**
- [ ] Mimir: Create indexes for product_type, channel
- [ ] Qdrant: Add payload indexes
- [ ] Neo4j: Add properties to nodes

### **Phase 4c: Query Layer**
- [ ] Create filter validation library
- [ ] Enforce insurer_id rules
- [ ] Add aggregation support

### **Phase 5: Comparison Queries**
- [ ] Build cross-insurer search interface
- [ ] Create comparison templates
- [ ] Add grouping/aggregation

### **Phase 6: Analytics**
- [ ] Product trend analysis
- [ ] Channel quality metrics
- [ ] Cross-insurer benchmarking

---

## 🔄 Example: Full Implementation

### **Input Chunk (Phase 1)**
```json
{
  "source_id": "url_insurer_001_health_0",
  "content": "PRU Mao Mao Double Sure covers hospitalization...",
  "insurer_id": "insurer_001",
  "product_type": "health",
  "channel": "url",
  "language": "en",
}
```

### **Query: Compare Health Plans**
```python
results = mimir.search(
    query="critical illness coverage",
    filters={
        "insurer_id": {"$in": ["insurer_001", "insurer_002", "insurer_006"]},
        "product_type": "health",
    },
    group_by="insurer_id",
    top_k=5_per_group=True,
)

# Results:
# insurer_001 (Prudential):
#   - PRU Mao Mao Double Sure (critical illness 63 diseases)
#   - PRUBetter Care (critical illness 7 diseases)
#
# insurer_002 (AXA):
#   - AXA Health Plus (critical illness coverage included)
#   - AXA Critical Care (specialized)
#
# insurer_006 (AIA):
#   - AIA Critical Protect (63 diseases)
#   - AIA Care Plus (comprehensive)
```

---

## 📝 Notes

- **Isolation First:** Insurer isolation is the default, not optional
- **Cross-Insurer is Explicit:** Requires explicit list or aggregation
- **Product Classification:** Automated during extraction, can be manual override
- **Channel Tracking:** Automatic from source_type (url, upload, bancassurance)
- **Scalable:** Adding filters doesn't change core isolation logic
- **Analytics-Ready:** Aggregation supports trend analysis without compromising isolation
