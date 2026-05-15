# Clinical Calculator Ingestion Failure - Root Cause & Fix

## Problem

Clinical calculators (CHADS2, eGFR, etc.) are failing to ingest:
```
CHADS2 Score (Stroke Risk)    → FAILED
eGFR (Kidney Function)        → FAILED
```

## Root Cause Analysis

The issue is in the `data_sources` table schema. The ingest script is using:

```python
sql = f"""
INSERT INTO data_sources (
    tenant_id, name, source_type, config_json, created_at
) VALUES (
    '{TENANT_ID}',
    {sql_escape(calc_data['name'])},
    'clinical_calculator',  # ← This value is causing the failure
    '{config}',
    NOW()
);
```

But the table schema has **schema version issues**:

### Schema Version 1 (Original - 20260225)
```sql
source_type ENUM('web', 'tabular', 'document', 'mcp')
-- Does NOT include 'clinical_calculator' → FAILS
```

### Schema Version 2 (Sprint 9 - 20260228) 
```sql
source_type VARCHAR(50)
-- Allows any string → Should work, but migration may not be applied
```

---

## Diagnostic Steps

### Step 1: Check Current Table Schema

```bash
mysql -u mimir -p -e "DESCRIBE mimir.data_sources \G" | grep source_type
```

**Expected Output (WORKING):**
```
Field: source_type
Type: varchar(50)
```

**If You See (BROKEN):**
```
Field: source_type
Type: enum('web','tabular','document','mcp')
```

### Step 2: Check Enum Values

```bash
mysql -u mimir -p -e "
SELECT COLUMN_TYPE FROM INFORMATION_SCHEMA.COLUMNS 
WHERE TABLE_NAME='data_sources' AND COLUMN_NAME='source_type';
"
```

---

## Solution A: Apply Missing Migration (Recommended)

The `source_type` column should be VARCHAR(50) to allow any source type (web, tabular, document, mcp, clinical_calculator, etc.)

```sql
-- Run this migration to fix the schema
ALTER TABLE data_sources MODIFY COLUMN source_type VARCHAR(50) NOT NULL;

-- Verify
DESCRIBE data_sources \G | grep source_type;
```

### Via kubectl (if using K3s):

```bash
kubectl exec -i -n asgard-infra deploy/mariadb -- mariadb -u mimir -p mimir << EOF
ALTER TABLE data_sources MODIFY COLUMN source_type VARCHAR(50) NOT NULL;
EOF
```

### Via Local MySQL:

```bash
mysql -u mimir -p mimir << EOF
ALTER TABLE data_sources MODIFY COLUMN source_type VARCHAR(50) NOT NULL;
EOF
```

---

## Solution B: Update Ingest Script (Workaround)

If you can't modify the schema, change the script to use an existing source_type:

**Current (FAILS):**
```python
'clinical_calculator',  # Not in enum
```

**Fixed (WORKS):**
```python
'tabular',  # Use existing enum value
# Then store calculator metadata in config_json
```

Modified ingest function:

```python
def ingest_clinical_calculators(dry_run: bool = False) -> int:
    """Load clinical calculator reference data."""
    print("\n🧮 Loading clinical calculators...")

    for calc_id, calc_data in CLINICAL_CALCULATORS.items():
        config = json.dumps({
            "calculator_type": "clinical_calculator",  # Store in JSON instead
            "inputs": calc_data["inputs"],
            "description": calc_data["description"],
            "category": calc_data["category"],
        }).replace("'", "\\'")

        sql = f"""
INSERT INTO data_sources (
    tenant_id, name, source_type, config_json, created_at
) VALUES (
    '{TENANT_ID}',
    {sql_escape(calc_data['name'])},
    'tabular',  # Use valid enum value
    '{config}',
    NOW()
) ON DUPLICATE KEY UPDATE updated_at=NOW();
"""
        rc, err = kubectl_mariadb(sql, dry_run)
        if rc != 0:
            print(f"  ✗ Failed to insert {calc_id}: {err}")
            return 1
        else:
            print(f"  ✓ {calc_data['name']}")

    return 0
```

---

## Solution C: Create Separate Table (Best Practice)

If clinical calculators are fundamentally different from web/tabular/document sources, create a dedicated table:

```sql
CREATE TABLE IF NOT EXISTS clinical_calculators (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    calculator_id VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    category VARCHAR(50),
    inputs JSON NOT NULL,
    description TEXT,
    formula TEXT,
    output_range VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uk_tenant_calc (tenant_id, calculator_id),
    INDEX idx_category (category)
);
```

Then insert directly:

```python
def ingest_clinical_calculators_v2(dry_run: bool = False) -> int:
    """Load clinical calculators into dedicated table."""
    print("\n🧮 Loading clinical calculators...")

    # Create table if needed
    create_sql = """
CREATE TABLE IF NOT EXISTS clinical_calculators (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    calculator_id VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    category VARCHAR(50),
    inputs JSON NOT NULL,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_tenant_calc (tenant_id, calculator_id)
);
"""
    kubectl_mariadb(create_sql, dry_run)

    for calc_id, calc_data in CLINICAL_CALCULATORS.items():
        inputs_json = json.dumps(calc_data["inputs"]).replace("'", "\\'")
        
        sql = f"""
INSERT INTO clinical_calculators (
    tenant_id, calculator_id, name, category, inputs, description
) VALUES (
    '{TENANT_ID}',
    '{calc_id}',
    {sql_escape(calc_data['name'])},
    {sql_escape(calc_data['category'])},
    '{inputs_json}',
    {sql_escape(calc_data['description'])}
) ON DUPLICATE KEY UPDATE name=VALUES(name);
"""
        rc, err = kubectl_mariadb(sql, dry_run)
        if rc != 0:
            print(f"  ✗ Failed: {err}")
            return 1
        else:
            print(f"  ✓ {calc_data['name']}")

    return 0
```

---

## Recommended Fix (IMMEDIATE)

### Step 1: Apply Schema Migration

```bash
mysql -u mimir -p mimir << EOF
ALTER TABLE data_sources MODIFY COLUMN source_type VARCHAR(50) NOT NULL;
EOF
```

### Step 2: Retry Ingestion

```bash
cd /Users/mimir/Developer/Mimir
python3 scripts/ingest_medical_sources.py --source clinical-calc
```

### Step 3: Verify

```bash
mysql -u mimir -p mimir << EOF
SELECT name, source_type, last_sync_status 
FROM data_sources 
WHERE source_type IN ('clinical_calculator', 'tabular') 
AND tenant_id='asgard_medical';
EOF
```

---

## Long-term Fix (BEST PRACTICE)

Create dedicated tables for each source type:

1. **clinical_calculators** — calculator definitions + formulas
2. **clinical_guidelines** — guideline metadata + content
3. **drug_interactions** — drug pair interactions
4. **icd10_codes** — diagnostic codes

This is cleaner than forcing everything into the generic `data_sources` table.

---

## Error Messages & Solutions

### Error 1: "Incorrect value for column 'source_type'"
```
ERROR 1054 (42000): Unknown column 'source_type' in 'where clause'
```
**Solution:** Schema hasn't been migrated yet. Run the ALTER TABLE command above.

### Error 2: "Data too long for column 'config_json'"
```
ERROR 1406 (22001): Data too long for column 'config_json'
```
**Solution:** The JSON payload is too large. Simplify the config or split into multiple rows.

### Error 3: "Duplicate entry for key 'PRIMARY'"
```
ERROR 1062 (23000): Duplicate entry '1-CHADS2'
```
**Solution:** Calculator already exists. Use ON DUPLICATE KEY UPDATE or delete first:
```bash
mysql -u mimir -p mimir << EOF
DELETE FROM data_sources 
WHERE tenant_id='asgard_medical' 
AND name LIKE 'CHADS%';
EOF
```

---

## Verification Checklist

After applying fix:

```bash
# 1. Check schema
mysql -u mimir -p mimir -e "DESCRIBE data_sources;" | grep source_type

# 2. Check ingested calculators
mysql -u mimir -p mimir -e "
SELECT COUNT(*) as count, source_type 
FROM data_sources 
WHERE tenant_id='asgard_medical' 
GROUP BY source_type;
"

# 3. Expected output:
# count │ source_type
# 7     │ clinical_calculator   (or 'tabular' if using workaround)

# 4. Check last sync status
mysql -u mimir -p mimir -e "
SELECT name, last_sync_status, last_sync_at 
FROM data_sources 
WHERE tenant_id='asgard_medical' 
AND source_type='clinical_calculator' LIMIT 3;
"

# Expected:
# name                    │ last_sync_status │ last_sync_at
# CHADS2 Score            │ COMPLETED        │ 2026-05-15 08:50:00
# eGFR (Kidney Function)  │ COMPLETED        │ 2026-05-15 08:50:01
```

---

## Implementation Steps

```bash
# 1. Fix schema
mysql -u mimir -p << EOF
use mimir;
ALTER TABLE data_sources MODIFY COLUMN source_type VARCHAR(50) NOT NULL;
EOF

# 2. Verify schema change
mysql -u mimir -p -e "DESCRIBE mimir.data_sources;" | grep source_type

# 3. Clear old failed records (optional)
mysql -u mimir -p mimir << EOF
DELETE FROM data_sources 
WHERE tenant_id='asgard_medical' 
AND source_type IN ('clinical_calculator', 'tabular')
AND last_sync_status='FAILED';
EOF

# 4. Re-run ingestion
cd /Users/mimir/Developer/Mimir
python3 scripts/ingest_medical_sources.py --source clinical-calc

# 5. Verify success
mysql -u mimir -p mimir << EOF
SELECT COUNT(*) as ingested_calculators
FROM data_sources
WHERE tenant_id='asgard_medical'
AND source_type='clinical_calculator'
AND last_sync_status='COMPLETED';
EOF
```

---

**Recommendation:** Apply **Step 1** (Schema Migration) immediately. This unblocks all clinical calculator ingestion.

**Timeline:** 5 minutes to fix
**Impact:** Enables diagnostic coding + risk stratification for agents
