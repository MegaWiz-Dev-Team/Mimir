# Medical Guidelines Batch Pipeline — Setup & Fix

## Problem

AASM Sleep Apnea Management 2023 batch failed:
```
Chunks: 0 | KG: 0e / 0r | Status: FAILED
Error: "No chunks found — run sync first"
```

---

## Root Cause

The guideline document is registered as a **data_source** but:
1. ❌ No document URL/file configured
2. ❌ No sync has run to fetch the document
3. ❌ No chunks exist to process
4. ❌ Pipeline stages depend on chunks → all fail

**Pipeline Flow:**
```
register_source → sync (fetch doc) → chunk → embed → KG extract → QA → index
                    ↑
                 MISSING: no doc fetched
                    ↓
          All downstream steps fail
```

---

## Prerequisites

### What You Need

1. **PDF Source** — The actual AASM Sleep Apnea Management 2023 guideline PDF
   - Option A: URL (e.g., publisher website)
   - Option B: Local file path
   - Option C: Cloud storage (S3, GCS)

2. **Document Metadata** — Title, source, publication date, etc.

3. **Configuration** — How to sync the document

---

## Solution A: Register as Web Source (URL)

### Step 1: Get Document URL

```bash
# AASM publishes guidelines at:
# https://aasm.org/clinical-resources/position-papers/

# For Sleep Apnea Management (example):
GUIDELINE_URL="https://aasm.org/files/PDFs/2023-AASM-Sleep-Apnea-Management.pdf"
```

### Step 2: Register Data Source

```bash
mysql -u mimir -p mimir << EOF
INSERT INTO data_sources (
    tenant_id,
    name,
    source_type,
    config_json,
    schedule,
    created_at
) VALUES (
    'asgard_medical',
    'AASM Sleep Apnea Management 2023',
    'web',
    JSON_OBJECT(
        'url', 'https://aasm.org/files/PDFs/2023-AASM-Sleep-Apnea-Management.pdf',
        'document_type', 'guideline',
        'category', 'sleep_medicine',
        'year', 2023,
        'organization', 'American Academy of Sleep Medicine',
        'retry_policy', 'exponential_backoff'
    ),
    'Manual',
    NOW()
) ON DUPLICATE KEY UPDATE updated_at=NOW();
EOF
```

### Step 3: Verify Registration

```bash
mysql -u mimir -p mimir << EOF
SELECT id, name, source_type, JSON_EXTRACT(config_json, '$.url') as url
FROM data_sources
WHERE name LIKE 'AASM%' AND tenant_id='asgard_medical';
EOF
```

**Expected Output:**
```
id  | name                              | source_type | url
5   | AASM Sleep Apnea Management 2023  | web         | https://aasm.org/...
```

### Step 4: Trigger Sync

```bash
# Option A: Via API
curl -X POST http://localhost:8080/api/v1/tenants/asgard-medical/sources/5/sync \
  -H "Content-Type: application/json"

# Option B: Via CLI (if available)
python3 scripts/trigger_source_sync.py --source-id 5 --tenant asgard_medical

# Option C: Manual
# The system will auto-sync on next scheduled run (if schedule is not "Manual")
```

### Step 5: Monitor Pipeline

```bash
# Check sync status
mysql -u mimir -p mimir << EOF
SELECT name, last_sync_status, last_sync_at
FROM data_sources
WHERE name LIKE 'AASM%' AND tenant_id='asgard_medical';
EOF

# Monitor batch run
# Check /Users/mimir/Developer/Mimir/batch-pipeline.log for:
# [n/7] 🔄 Starting pipeline for: AASM Sleep Apnea...
```

### Step 6: Wait for Pipeline Completion

Pipeline stages (in order):
```
1. sync              → Fetch document from URL
2. chunk_check      → Verify chunks created
3. embed_chunks     → Create vector embeddings
4. kg_extraction    → Extract entities & relationships
5. qa_extraction    → Generate Q&A pairs
6. pageindex_gen    → Create page-level index
7. qa_indexing      → Index Q&A into vector DB
```

**Expected Success:**
```
✅ Completed in 420s — Steps: 
   sync=completed,
   chunk_check=completed,
   embed_chunks=completed,
   kg_extraction=completed,
   qa_extraction=completed,
   pageindex_generation=completed,
   qa_indexing=completed
```

---

## Solution B: Register as Document Source (Local File)

### Step 1: Place PDF File

```bash
# Create guidelines directory
mkdir -p /Users/mimir/Developer/Mimir/data/medical/guidelines

# Copy PDF
cp /path/to/AASM-Sleep-Apnea-2023.pdf \
   /Users/mimir/Developer/Mimir/data/medical/guidelines/

# Verify
ls -lh /Users/mimir/Developer/Mimir/data/medical/guidelines/
```

### Step 2: Register Data Source

```bash
mysql -u mimir -p mimir << EOF
INSERT INTO data_sources (
    tenant_id,
    name,
    source_type,
    config_json,
    schedule,
    created_at
) VALUES (
    'asgard_medical',
    'AASM Sleep Apnea Management 2023',
    'document',
    JSON_OBJECT(
        'file_path', '/Users/mimir/Developer/Mimir/data/medical/guidelines/AASM-Sleep-Apnea-2023.pdf',
        'document_type', 'guideline',
        'category', 'sleep_medicine',
        'year', 2023,
        'organization', 'American Academy of Sleep Medicine',
        'encoding', 'utf-8'
    ),
    'Manual',
    NOW()
) ON DUPLICATE KEY UPDATE updated_at=NOW();
EOF
```

### Step 3: Trigger Pipeline

Same as Solution A, Step 4-6

---

## Solution C: Set Up Batch Schedule (Recurring)

### For Continuous Ingestion

If you want to ingest multiple guidelines on a schedule:

```bash
mysql -u mimir -p mimir << EOF
-- Update to scheduled sync
UPDATE data_sources 
SET schedule = 'Daily at 02:00'
WHERE tenant_id='asgard_medical' 
AND source_type='document'
AND name LIKE '%Guideline%';

-- Or update to manual with last_sync_status='PENDING'
UPDATE data_sources 
SET last_sync_status='PENDING'
WHERE tenant_id='asgard_medical'
AND source_type='document';
EOF

# System will auto-process pending sources
```

---

## Troubleshooting

### Error 1: "No chunks found"

**Cause:** Document hasn't been synced yet

**Solution:**
```bash
# Check sync status
mysql -u mimir -p mimir << EOF
SELECT name, last_sync_status, last_sync_at
FROM data_sources
WHERE name LIKE 'AASM%';
EOF

# If PENDING → Manually trigger sync
curl -X POST http://localhost:8080/api/v1/sources/5/sync

# Wait 30+ seconds for sync to complete
```

---

### Error 2: "URL unreachable"

**Cause:** PDF URL is incorrect or requires authentication

**Solution:**
```bash
# Test URL manually
curl -I "https://aasm.org/files/PDFs/2023-AASM-Sleep-Apnea-Management.pdf"

# If 403/401 → Need authentication
# Either:
# 1. Use public URL from publisher
# 2. Download PDF manually and use local file (Solution B)
# 3. Configure auth headers in config_json:
#    "headers": {"Authorization": "Bearer token"}
```

---

### Error 3: "PDF extraction failed"

**Cause:** OCR or text extraction failed

**Solution:**
```bash
# Check if PDF is image-based (scanned)
pdftotext AASM-Sleep-Apnea-2023.pdf - | head -20

# If output is garbage → Scanned PDF needs OCR
# Options:
# 1. Use cloud OCR (Google Vision, Azure)
# 2. Use Tesseract locally:
#    tesseract AASM-Sleep-Apnea-2023.pdf AASM-Sleep-Apnea-2023-ocr

# 3. Or download native-PDF version from publisher
```

---

### Error 4: "Timeout during chunk_check"

**Cause:** Large document taking too long

**Solution:**
```bash
# Increase timeout in config
UPDATE data_sources 
SET config_json = JSON_SET(
    config_json,
    '$.chunk_timeout_seconds', 300,
    '$.max_chunk_size', 512
)
WHERE id=5;

# Retry sync
curl -X POST http://localhost:8080/api/v1/sources/5/sync
```

---

## Complete Setup Example

### Full Medical Guidelines Batch Setup

```bash
#!/bin/bash
# Setup 5 major guidelines for asgard-medical

TENANT="asgard_medical"
DB_HOST="localhost"
DB_USER="mimir"
DB_PASS="${MIMIR_DB_PASSWORD}"

mysql -h "$DB_HOST" -u "$DB_USER" -p "$DB_PASS" << EOF
USE mimir;

-- 1. ACC/AHA Hypertension 2023
INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
VALUES (
    '$TENANT',
    'ACC/AHA Hypertension Guidelines 2023',
    'web',
    JSON_OBJECT(
        'url', 'https://www.acc.org/getmedia/dc827f88-48ef-4a0d-b6f3-5665dd1a1e28/2023-acc-aha-htn-guidelines.pdf',
        'category', 'cardiology',
        'year', 2023
    ),
    'Manual'
);

-- 2. ESC Chest Pain 2021
INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
VALUES (
    '$TENANT',
    'ESC Chest Pain Guidelines 2021',
    'web',
    JSON_OBJECT(
        'url', 'https://academic.oup.com/eurheartj/article-pdf/42/36/3645/40526516/ehab368.pdf',
        'category', 'cardiology',
        'year', 2021
    ),
    'Manual'
);

-- 3. AASM Sleep Apnea 2023
INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
VALUES (
    '$TENANT',
    'AASM Sleep Apnea Management 2023',
    'web',
    JSON_OBJECT(
        'url', 'https://aasm.org/files/PDFs/2023-AASM-Sleep-Apnea-Management.pdf',
        'category', 'sleep_medicine',
        'year', 2023
    ),
    'Manual'
);

-- 4. AAP Pediatric ADHD 2022
INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
VALUES (
    '$TENANT',
    'AAP ADHD Screening Guidelines 2022',
    'web',
    JSON_OBJECT(
        'url', 'https://pediatrics.aappublications.org/content/early/2022/10/27/peds.2022-049050.full.pdf',
        'category', 'pediatrics',
        'year', 2022
    ),
    'Manual'
);

-- 5. ACEP Sepsis Protocol 2023
INSERT INTO data_sources (tenant_id, name, source_type, config_json, schedule)
VALUES (
    '$TENANT',
    'ACEP Sepsis Management 2023',
    'web',
    JSON_OBJECT(
        'url', 'https://www.acep.org/siteassets/uploads/new-pdfs/clinical-policies/2023-acp-clinical-policy-sepsis.pdf',
        'category', 'emergency',
        'year', 2023
    ),
    'Manual'
);

-- Verify
SELECT id, name, source_type, last_sync_status 
FROM data_sources 
WHERE tenant_id='$TENANT' AND source_type IN ('web', 'document')
ORDER BY id DESC LIMIT 5;
EOF
```

### Run Pipeline

```bash
# Trigger all guideline syncs
GUIDELINE_IDS=$(mysql -u mimir -p"$MIMIR_DB_PASSWORD" mimir << EOF
SELECT id FROM data_sources 
WHERE tenant_id='asgard_medical' 
AND source_type IN ('web', 'document')
ORDER BY id DESC LIMIT 5;
EOF
)

for source_id in $GUIDELINE_IDS; do
  echo "Syncing source #$source_id..."
  curl -X POST http://localhost:8080/api/v1/sources/$source_id/sync
  sleep 5
done

# Monitor progress
tail -f /Users/mimir/Developer/Mimir/batch-pipeline.log
```

---

## Verification

### Check Pipeline Success

```bash
mysql -u mimir -p mimir << EOF
SELECT 
    s.name,
    s.last_sync_status,
    s.last_sync_at,
    COUNT(c.id) as chunk_count,
    COUNT(e.id) as entity_count
FROM data_sources s
LEFT JOIN chunks c ON s.id = c.source_id
LEFT JOIN kg_entities e ON s.id = e.source_id
WHERE s.tenant_id='asgard_medical'
AND s.source_type IN ('web', 'document')
GROUP BY s.id, s.name
ORDER BY s.id DESC;
EOF
```

**Expected Output:**
```
name                                | last_sync_status | chunk_count | entity_count
AASM Sleep Apnea Management 2023    | COMPLETED        | 142         | 247
ACC/AHA Hypertension Guidelines 2023| COMPLETED        | 156         | 312
ESC Chest Pain Guidelines 2021      | COMPLETED        | 128         | 218
```

---

## Performance Expectations

| Stage | Time | Typical Output |
|-------|------|---|
| **sync** | 30-60s | Download PDF (1-10 MB) |
| **chunk_check** | 5-10s | Create text chunks (~400-600 tokens) |
| **embed_chunks** | 60-120s | Create vectors (BGE-M3, 1024-dim) |
| **kg_extraction** | 30-60s | Extract entities + relationships |
| **qa_extraction** | 30-60s | Generate Q&A pairs |
| **pageindex_generation** | 10-20s | Create page-level index |
| **qa_indexing** | 20-30s | Index into Qdrant vector DB |
| **TOTAL** | 3-5 minutes | Complete guideline processed |

---

## Best Practices

### ✅ DO

- ✓ Use official guideline URLs (ACC.org, ESC.org, AASM.org)
- ✓ Test URL accessibility before registering
- ✓ Include year and organization in metadata
- ✓ Monitor first run to catch errors early
- ✓ Keep PDF file sizes <20 MB for faster processing

### ❌ DON'T

- ✗ Use paywalled/restricted URLs (will fail with 403)
- ✗ Store PDFs in temporary directories
- ✗ Register multiple sources with identical names
- ✗ Assume sync happens instantly (takes 30+ seconds)
- ✗ Mix different document types in one source

---

## Reference: All Medical Guidelines to Add

| # | Guideline | Organization | Year | Category | Status |
|---|-----------|---------------|------|----------|--------|
| 1 | Hypertension | ACC/AHA | 2023 | Cardiology | ⏳ Ready |
| 2 | Chest Pain | ESC | 2021 | Cardiology | ⏳ Ready |
| 3 | Heart Failure | ACC/AHA | 2022 | Cardiology | ⏳ Ready |
| 4 | Arrhythmia | ESC | 2020 | Cardiology | ⏳ Ready |
| 5 | Sleep Apnea | AASM | 2023 | Sleep | ⏳ Ready |
| 6 | Insomnia | AASM | 2023 | Sleep | ⏳ Ready |
| 7 | ADHD | AAP | 2022 | Pediatrics | ⏳ Ready |
| 8 | Vaccination | AAP | 2023 | Pediatrics | ⏳ Ready |
| 9 | Sepsis | ACEP | 2023 | Emergency | ⏳ Ready |
| 10 | Chest Pain | ACEP | 2022 | Emergency | ⏳ Ready |

---

**Next Step:** Apply Solution A or B above to register AASM Sleep Apnea guideline and trigger pipeline. Should complete in 3-5 minutes.

