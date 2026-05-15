# Embed Chunks Failed — 400 Bad Request Diagnosis

## Problem

CPG (Clinical Practice Guidelines) batch failed at embed_chunks stage:

```
Chunks: 0 | KG: 0e / 0r | Status: FAILED

Pipeline Progress:
  ✅ chunk_check     → completed (chunks created)
  ❌ embed_chunks    → failed (400 Bad Request)
  ⏳ pageindex_gen   → pending (blocked)
  ⏳ kg_extraction   → pending (blocked)
```

**Error:**
```
POST https://api.asgard.internal/api/v1/sources/26/extract-ai 400 (Bad Request)
Error: "Source has no S3 file — nothing to extract"
```

---

## Root Cause Analysis

The pipeline flow requires:

```
sync (fetch & upload to S3)
         ↓
    s3_key stored in DB
         ↓
   embed_chunks reads s3_key
         ↓
   downloads from S3
         ↓
   creates embeddings
```

**What's Happening:**

1. ✅ Chunks created from text (chunk_check passed)
2. ❌ But source record has NO s3_key
3. ❌ embed_chunks tries to access s3_key
4. ❌ Returns 400: "Source has no S3 file"

**Why No S3 Key:**

```
If source_type = 'web':
  - URL fetched during sync step
  - File should be uploaded to S3
  - s3_key should be stored in data_sources table
  
Current Status:
  - sync step may not have run properly
  - OR sync succeeded but didn't upload to S3
  - s3_key column is NULL in DB
```

---

## Diagnosis Steps

### Step 1: Check Source Configuration

```bash
mysql -u mimir -p mimir << EOF
SELECT 
  id,
  name,
  source_type,
  config_json,
  s3_key,
  last_sync_status,
  last_sync_at
FROM data_sources
WHERE id = 26 AND tenant_id='asgard_medical';
EOF
```

**Expected (WORKING):**
```
id  | name | source_type | s3_key                      | last_sync_status
26  | cpg  | web         | sources/cpg-document.pdf    | COMPLETED
```

**If You See (BROKEN):**
```
id  | name | source_type | s3_key | last_sync_status
26  | cpg  | web         | NULL   | COMPLETED
```

If s3_key is NULL → That's the problem!

---

### Step 2: Check Config JSON

```bash
mysql -u mimir -p mimir << EOF
SELECT JSON_PRETTY(config_json)
FROM data_sources
WHERE id=26;
EOF
```

**Expected:**
```json
{
  "url": "https://...",
  "document_type": "guideline",
  ...
}
```

---

### Step 3: Check S3 Bucket

```bash
# List files in S3 for this source
aws s3 ls s3://asgard-mimir/sources/ | grep -i cpg

# Or check local MinIO if using that
minio-client ls minio/asgard-mimir/sources/ | grep cpg
```

---

## Solution A: Re-run Sync with S3 Upload

### Trigger Fresh Sync

```bash
# Mark as pending to re-trigger sync
mysql -u mimir -p mimir << EOF
UPDATE data_sources
SET last_sync_status='PENDING', last_sync_at=NULL
WHERE id=26;
EOF

# Trigger sync (wait 60+ seconds)
curl -X POST http://localhost:8080/api/v1/sources/26/sync

# Monitor logs
tail -f /Users/mimir/Developer/Mimir/batch-pipeline.log | grep "source_id=26\|cpg\|S3"

# Verify s3_key was populated
mysql -u mimir -p mimir << EOF
SELECT id, name, s3_key, last_sync_status 
FROM data_sources 
WHERE id=26;
EOF
```

### Expected Output

After sync:
```
s3_key: sources/asgard_medical/cpg-20260515-1234567890.pdf
last_sync_status: COMPLETED
```

---

## Solution B: Manually Upload to S3 and Update DB

### If You Have PDF Locally

```bash
# 1. Identify PDF
GUIDELINE_PDF="/path/to/cpg-guideline.pdf"

# 2. Upload to S3 (MinIO example)
aws s3 cp "$GUIDELINE_PDF" \
  s3://asgard-mimir/sources/asgard_medical/cpg-guideline.pdf

# 3. Update DB with s3_key
mysql -u mimir -p mimir << EOF
UPDATE data_sources
SET s3_key = 'sources/asgard_medical/cpg-guideline.pdf',
    last_sync_status = 'COMPLETED',
    last_sync_at = NOW()
WHERE id=26;
EOF

# 4. Verify
mysql -u mimir -p mimir << EOF
SELECT s3_key FROM data_sources WHERE id=26;
EOF
```

---

## Solution C: Fix Sync Configuration

If sync is running but not uploading to S3, check config:

```bash
mysql -u mimir -p mimir << EOF
-- Check if sync has "upload_to_s3" configured
SELECT 
  id, 
  name,
  JSON_EXTRACT(config_json, '$.url') as url,
  JSON_EXTRACT(config_json, '$.skip_s3_upload') as skip_s3
FROM data_sources
WHERE id=26;
EOF
```

**If skip_s3_upload is TRUE:**

```bash
mysql -u mimir -p mimir << EOF
UPDATE data_sources
SET config_json = JSON_SET(
  config_json,
  '$.skip_s3_upload', FALSE
)
WHERE id=26;
EOF
```

---

## Solution D: Check Sync Logs

Look for sync errors in pipeline logs:

```bash
# Search for sync errors for source_id=26
grep -A 20 "source_id.*26\|cpg" /Users/mimir/Developer/Mimir/batch-pipeline.log | grep -i "error\|fail\|s3\|upload"

# Example error messages:
# - "S3 upload failed: timeout"
# - "S3 bucket not found"
# - "S3 credentials missing"
# - "Network error downloading from URL"
```

If you see S3 errors → Check S3 credentials and connectivity

```bash
# Test S3 connectivity
aws s3 ls s3://asgard-mimir/ --region us-west-2

# Or check environment
echo $AWS_ACCESS_KEY_ID
echo $AWS_SECRET_ACCESS_KEY
echo $AWS_S3_BUCKET
```

---

## Complete Fix Workflow

```bash
#!/bin/bash
set -e

SOURCE_ID=26
TENANT="asgard_medical"
DB_USER="mimir"
PDF_URL="https://example.com/cpg-guideline.pdf"

echo "🔧 Fixing embed_chunks failure for source_id=$SOURCE_ID..."

# Step 1: Reset sync status
echo "1. Resetting sync status to PENDING..."
mysql -u $DB_USER -p << EOF
UPDATE data_sources
SET last_sync_status='PENDING', last_sync_at=NULL
WHERE id=$SOURCE_ID AND tenant_id='$TENANT';
EOF

# Step 2: Trigger fresh sync
echo "2. Triggering sync (waiting 60 seconds)..."
curl -X POST http://localhost:8080/api/v1/sources/$SOURCE_ID/sync
sleep 60

# Step 3: Check if s3_key was populated
echo "3. Checking s3_key..."
S3_KEY=$(mysql -u $DB_USER -p -se "
SELECT s3_key FROM data_sources WHERE id=$SOURCE_ID;
")

if [ -z "$S3_KEY" ]; then
  echo "   ❌ s3_key still empty - sync failed"
  echo "   Check logs: tail -f /Users/mimir/Developer/Mimir/batch-pipeline.log"
  exit 1
else
  echo "   ✅ s3_key populated: $S3_KEY"
fi

# Step 4: Re-trigger pipeline
echo "4. Re-triggering batch pipeline..."
curl -X POST http://localhost:8080/api/v1/sources/$SOURCE_ID/batch-run

# Step 5: Monitor progress
echo "5. Monitoring progress (watch for 'embed_chunks' status)..."
sleep 5
tail -f /Users/mimir/Developer/Mimir/batch-pipeline.log | grep -E "embed_chunks|COMPLETED|FAILED" | head -20

echo "✅ Fix initiated! Monitor logs for completion."
```

---

## Verification

### After Fix Applied

```bash
# Check 1: s3_key is populated
mysql -u mimir -p mimir << EOF
SELECT id, name, s3_key, last_sync_status
FROM data_sources
WHERE id=26;
EOF
# Should show: s3_key = 'sources/asgard_medical/cpg-...'

# Check 2: Chunks exist
mysql -u mimir -p mimir << EOF
SELECT COUNT(*) as chunk_count
FROM chunks
WHERE source_id=26;
EOF
# Should show: chunk_count > 0

# Check 3: Pipeline completed
mysql -u mimir -p mimir << EOF
SELECT source_id, status, 
  JSON_EXTRACT(steps, '$.embed_chunks') as embed_status
FROM pipeline_runs
WHERE source_id=26
ORDER BY created_at DESC LIMIT 1;
EOF
# Should show: embed_chunks = 'completed'
```

---

## Common Root Causes & Solutions

| Cause | Symptom | Fix |
|-------|---------|-----|
| Sync never ran | s3_key NULL, last_sync_status=PENDING | Re-trigger sync manually |
| S3 upload failed | last_sync_status=FAILED, error in logs | Check S3 credentials/connectivity |
| Wrong S3 bucket | Sync says OK but s3_key not found | Verify AWS_S3_BUCKET env var |
| URL unreachable | S3 upload failed, network error | Test URL with curl |
| Credentials missing | Auth error during sync | Check AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY |
| PDF corrupted | Chunks created but embedding fails | Re-download or use different source |

---

## Prevention Tips

✅ **DO**
- Verify URL is accessible before registering
- Check S3 credentials before syncing
- Monitor first sync carefully
- Verify s3_key after sync completes

❌ **DON'T**
- Assume sync ran (check status)
- Assume S3 upload succeeded (verify s3_key)
- Register same source twice
- Mix web/document source types

---

## Reference: Pipeline Dependency Chain

```
1. sync
   ├─ Download from URL
   └─ Upload to S3 ← Creates s3_key
      
2. chunk_check
   ├─ Verify chunks exist
   └─ (Independent of s3_key)
   
3. embed_chunks
   ├─ Read from s3_key
   └─ Create vector embeddings ← REQUIRES s3_key
   
4. pageindex_generation
   ├─ Create page index
   └─ (Needs chunks)
   
5. kg_extraction
   ├─ Extract entities
   └─ (Needs chunks)
   
6-8. QA & indexing stages
   └─ (Depends on prior stages)
```

**Critical:** embed_chunks MUST have s3_key or it fails

---

## Next Actions

1. **Immediate:** Run Diagnosis Steps 1-3 above
2. **If s3_key is NULL:** Apply Solution A (re-run sync)
3. **Monitor:** Watch batch-pipeline.log for embed_chunks completion
4. **Verify:** Run verification queries
5. **Report:** Document what fixed it for team

---

**Timeline:** 
- Diagnosis: 5 minutes
- Fix: 2-5 minutes (depending on sync time)
- Verification: 2 minutes
- **Total: ~15 minutes**

