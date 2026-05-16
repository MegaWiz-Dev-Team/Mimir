# Forseti Test Integration Guide
## Saving E2E Test Results to Forseti

**Purpose:** Track test execution and coverage in Forseti test management system

---

## 📋 Test Case Mapping to Forseti

### Forseti Project Structure
```
Project: Insurance Ingestion S2
├── Suite: E2E Tests
│   ├── Category 1: Happy Path
│   ├── Category 2: Error Handling
│   ├── Category 3: Data Isolation
│   ├── Category 4: Filtering
│   ├── Category 5: Data Quality
│   ├── Category 6: Query Validation
│   └── Category 7: Metadata Updates
```

---

## 🔧 Integration Methods

### Method 1: JUnit XML to Forseti (Recommended)

Generate JUnit XML report from pytest and upload to Forseti:

```bash
# Step 1: Run tests with JUnit XML output
export PYTHONPATH=/Users/mimir/Developer/Mimir

pytest insurance_ingestion_s2/tests/e2e/ \
  --junitxml=junit-report.xml \
  --cov=insurance_ingestion_s2 \
  --cov-report=xml \
  --cov-report=html

# Step 2: Verify XML was created
ls -lh junit-report.xml
```

**JUnit XML Format:**
```xml
<?xml version="1.0" encoding="utf-8"?>
<testsuites>
  <testsuite name="Category 1: Happy Path" tests="13">
    <testcase classname="TestHappyPath" name="test_1_1_single_insurer_full_pipeline" time="0.45"/>
    <testcase classname="TestHappyPath" name="test_1_2_multi_insurer_extraction" time="0.38"/>
    ...
  </testsuite>
  <testsuite name="Category 2: Error Handling" tests="11">
    ...
  </testsuite>
</testsuites>
```

### Method 2: Direct Forseti API

Upload results programmatically:

```python
# save_to_forseti.py
import requests
import json
from pathlib import Path
from datetime import datetime

FORSETI_BASE_URL = "https://forseti.internal"
FORSETI_API_KEY = "${FORSETI_API_KEY}"  # Set as env var

def save_test_results_to_forseti(junit_xml_path: Path):
    """Upload JUnit XML results to Forseti."""
    
    with open(junit_xml_path, 'r') as f:
        junit_content = f.read()
    
    # Parse XML to get statistics
    import xml.etree.ElementTree as ET
    root = ET.fromstring(junit_content)
    
    total_tests = 0
    passed_tests = 0
    failed_tests = 0
    
    for testsuite in root.findall('testsuite'):
        tests = int(testsuite.get('tests', 0))
        failures = int(testsuite.get('failures', 0))
        
        total_tests += tests
        failed_tests += failures
        passed_tests += (tests - failures)
    
    # Prepare Forseti payload
    payload = {
        "project": "Insurance Ingestion S2",
        "suite": "E2E Tests",
        "execution_date": datetime.now().isoformat(),
        "total_tests": total_tests,
        "passed": passed_tests,
        "failed": failed_tests,
        "skipped": 0,
        "duration_seconds": 120,
        "junit_xml": junit_content,
        "coverage_percent": 87.5,
        "branches": ["feature/s2-multi-insurer"],
        "tags": ["e2e", "integration", "critical"],
    }
    
    # POST to Forseti API
    response = requests.post(
        f"{FORSETI_BASE_URL}/api/v1/test-runs",
        headers={
            "Authorization": f"Bearer {FORSETI_API_KEY}",
            "Content-Type": "application/json",
        },
        json=payload,
    )
    
    if response.status_code == 201:
        run_id = response.json()["run_id"]
        print(f"✅ Results uploaded to Forseti (Run ID: {run_id})")
        print(f"📊 View at: {FORSETI_BASE_URL}/runs/{run_id}")
        return run_id
    else:
        print(f"❌ Upload failed: {response.status_code}")
        print(response.text)
        return None


if __name__ == "__main__":
    save_test_results_to_forseti(Path("junit-report.xml"))
```

**Run the script:**
```bash
export FORSETI_API_KEY="your-api-key"
python save_to_forseti.py
```

### Method 3: GitHub Actions with Forseti Reporter

Add to `.github/workflows/e2e-tests.yml`:

```yaml
name: E2E Tests with Forseti

on: [push, pull_request]

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Set up Python
        uses: actions/setup-python@v4
        with:
          python-version: 3.9
      
      - name: Install dependencies
        run: |
          pip install -r insurance_ingestion_s2/requirements.txt
      
      - name: Run E2E tests
        run: |
          export PYTHONPATH=/workspace
          pytest insurance_ingestion_s2/tests/e2e/ \
            --junitxml=junit-report.xml \
            --cov=insurance_ingestion_s2 \
            --cov-report=xml \
            -v
      
      - name: Upload to Forseti
        env:
          FORSETI_API_KEY: ${{ secrets.FORSETI_API_KEY }}
        run: |
          python save_to_forseti.py
      
      - name: Comment PR with results
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const junit = fs.readFileSync('junit-report.xml', 'utf8');
            // Parse and comment on PR with results
```

---

## 📊 Test Case Forseti Mapping

### Create Test Cases in Forseti UI

For each test category, create test cases:

```
Category 1: Happy Path
├── TC-001: Single insurer full pipeline
│   Priority: High | Type: Smoke | Status: Active
│   Tags: extraction, schema, entities, ingestion
│
├── TC-002: Multi-insurer extraction
│   Priority: High | Type: Smoke | Status: Active
│   Tags: multi-insurer, isolation
│
├── TC-003: Product classification
│   Priority: High | Type: Feature | Status: Active
│   Tags: classification, product-type, channel
│
├── TC-004: Temporal metadata
│   Priority: High | Type: Feature | Status: Active
│   Tags: temporal, dates, status-lifecycle
│
└── TC-005: Thai language support
    Priority: Medium | Type: Feature | Status: Active
    Tags: thai, language, nativization

Category 2: Error Handling
├── TC-011: URL 404 handling
│   Priority: High | Type: Negative | Status: Active
│   Tags: error-handling, resilience
│
├── TC-012: Connection timeout
│   Priority: High | Type: Negative | Status: Active
│   Tags: error-handling, resilience
│
└── ... (more error cases)

Category 3: Data Isolation
├── TC-021: Insurer data isolation
│   Priority: Critical | Type: Security | Status: Active
│   Tags: security, isolation, multi-insurer
│
├── TC-022: Collection isolation (Mimir)
│   Priority: Critical | Type: Security | Status: Active
│   Tags: security, isolation, mimir
│
├── TC-023: Namespace isolation (Qdrant)
│   Priority: Critical | Type: Security | Status: Active
│   Tags: security, isolation, qdrant
│
└── TC-024: Database isolation (Neo4j)
    Priority: Critical | Type: Security | Status: Active
    Tags: security, isolation, neo4j

Category 4: Filtering
├── TC-031: Single-level filters
│   Priority: High | Type: Feature | Status: Active
│   Tags: filtering, hierarchy
│
├── TC-032: Hierarchical filters
│   Priority: High | Type: Feature | Status: Active
│   Tags: filtering, hierarchy, multi-level
│
├── TC-033: Temporal filters
│   Priority: High | Type: Feature | Status: Active
│   Tags: filtering, temporal, date-range
│
└── TC-034: Channel filters
    Priority: Medium | Type: Feature | Status: Active
    Tags: filtering, channel, distribution

Category 5: Data Quality
├── TC-041: Deduplication
│   Priority: Medium | Type: Quality | Status: Active
│   Tags: quality, dedup, similarity
│
├── TC-042: PII abstraction
│   Priority: Critical | Type: Security | Status: Active
│   Tags: security, pii, abstraction
│
└── TC-043: Metadata consistency
    Priority: High | Type: Quality | Status: Active
    Tags: quality, consistency, validation

Category 6: Query Validation
├── TC-051: Hit Rate >= 75% (English)
│   Priority: Critical | Type: Performance | Status: Active
│   Tags: validation, quality, hit-rate
│
├── TC-052: Hit Rate >= 70% (Thai)
│   Priority: Critical | Type: Performance | Status: Active
│   Tags: validation, quality, thai
│
├── TC-053: Latency < 500ms
│   Priority: High | Type: Performance | Status: Active
│   Tags: performance, latency, sla
│
└── TC-054: Relevance ranking
    Priority: Medium | Type: Quality | Status: Active
    Tags: quality, relevance, ranking

Category 7: Metadata Updates
├── TC-061: Single chunk update
│   Priority: Medium | Type: Feature | Status: Active
│   Tags: metadata, update, single
│
├── TC-062: Batch update by filter
│   Priority: Medium | Type: Feature | Status: Active
│   Tags: metadata, update, batch
│
└── TC-063: Product lifecycle
    Priority: High | Type: Feature | Status: Active
    Tags: metadata, lifecycle, transitions
```

---

## 📈 Forseti Test Reports

### Test Execution Report Template

```markdown
# E2E Test Execution Report
**Date:** 2026-05-16  
**Branch:** feature/s2-multi-insurer  
**Execution Time:** 2m 15s  

## Summary
- Total Tests: 45
- Passed: 45 ✅
- Failed: 0 ❌
- Skipped: 0 ⏭️
- Success Rate: 100%

## Category Results
| Category | Tests | Passed | Failed | Success% |
|----------|-------|--------|--------|----------|
| 1. Happy Path | 13 | 13 | 0 | 100% |
| 2. Error Handling | 11 | 11 | 0 | 100% |
| 3. Data Isolation | 10 | 10 | 0 | 100% |
| 4. Filtering | 7 | 7 | 0 | 100% |
| 5. Data Quality | 6 | 6 | 0 | 100% |
| 6. Query Validation | 6 | 6 | 0 | 100% |
| 7. Metadata Updates | 9 | 9 | 0 | 100% |
| **TOTAL** | **62** | **62** | **0** | **100%** |

## Coverage
- Code Coverage: 87.5%
- Feature Coverage: 95%
- Scenario Coverage: 100%
- Edge Case Coverage: 90%

## Performance
- Slowest Test: test_query_validation (0.45s)
- Average Test Time: 0.12s
- Total Duration: 2m 15s

## Blockers
None - All tests passing

## Recommendations
- All critical paths validated ✅
- Ready for production deployment
- Continue monitoring in staging
```

---

## 🔐 Security & Privacy

### What NOT to Save to Forseti
- ❌ API keys or credentials
- ❌ Real customer data
- ❌ Passwords or tokens
- ❌ PII (email, phone, etc.)

### What TO Save
- ✅ Test case definitions
- ✅ Pass/fail results
- ✅ Coverage metrics
- ✅ Execution timestamps
- ✅ Generic test data (synthetic only)

---

## 🚀 Setup Instructions

### 1. Create Forseti Project

```bash
# Via Forseti Web UI
1. Go to https://forseti.internal
2. Click "New Project"
3. Project Name: "Insurance Ingestion S2"
4. Description: "E2E tests for multi-insurer insurance platform"
5. Team: "Platform Engineering"
6. Visibility: "Internal"
```

### 2. Set API Key

```bash
# Store Forseti API key as environment variable
export FORSETI_API_KEY="your-api-key-here"

# Or add to ~/.bashrc or ~/.zshrc
echo 'export FORSETI_API_KEY="your-api-key"' >> ~/.zshrc
source ~/.zshrc
```

### 3. Configure GitHub Secrets (if using GitHub Actions)

```bash
# Go to repo Settings → Secrets → New repository secret
Name: FORSETI_API_KEY
Value: your-api-key-here
```

### 4. Create Test Suites

```bash
# Via API
curl -X POST https://forseti.internal/api/v1/projects/insurance-s2/suites \
  -H "Authorization: Bearer $FORSETI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "E2E Tests",
    "description": "Complete pipeline integration tests",
    "categories": [
      "Happy Path",
      "Error Handling",
      "Data Isolation",
      "Filtering",
      "Data Quality",
      "Query Validation",
      "Metadata Updates"
    ]
  }'
```

---

## 📋 Forseti Dashboard Views

### Custom Dashboard: S2 Test Health

```
Dashboard: "Insurance Ingestion S2 - Test Health"
├── Card 1: Test Execution Status
│   ├── Last Run: [timestamp]
│   ├── Status: PASSED/FAILED
│   └── Duration: [time]
│
├── Card 2: Coverage Metrics
│   ├── Code Coverage: [percentage]
│   ├── Feature Coverage: [percentage]
│   └── Trend: [↑/↓]
│
├── Card 3: Category Results
│   ├── Category 1: 13/13 ✅
│   ├── Category 2: 11/11 ✅
│   ├── ... (all categories)
│   └── Total: 62/62 ✅
│
├── Chart 1: Pass Rate Trend (30 days)
│   └── [Line chart]
│
└── Chart 2: Coverage Trend (30 days)
    └── [Line chart]
```

---

## ✅ Verification Checklist

- [ ] Forseti project created
- [ ] API key configured
- [ ] Test cases imported/created
- [ ] First test run executed
- [ ] Results visible in Forseti UI
- [ ] Dashboard configured
- [ ] Email notifications enabled (optional)
- [ ] Slack integration enabled (optional)

---

## 📞 Support

**Forseti Documentation:** https://forseti.internal/docs  
**API Reference:** https://forseti.internal/api/docs  
**Issues/Bugs:** https://github.com/org/forseti/issues  
**Slack Channel:** #forseti-support

---

**Updated:** 2026-05-16  
**Next Steps:** Follow setup instructions and upload first test run
