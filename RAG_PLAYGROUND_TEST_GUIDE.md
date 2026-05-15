# RAG Studio Playground Test Guide

## Overview

This guide provides automated test suites to verify the RAG Studio functionality on the `asgard-medical` tenant.

### What is Tested?

The test suites verify:

1. **Infrastructure & Connectivity**
   - Service health checks (`/health` endpoint)
   - Response times and availability

2. **Tenant Verification**
   - Tenant exists in the system
   - Tenant configuration is correct
   - Domain routing is working

3. **Document Management**
   - List ingested documents
   - Ingest new medical documents
   - Document indexing

4. **RAG Query Capabilities**
   - Vector search mode
   - Hybrid search mode
   - General queries
   - Source citation
   - Answer generation

## Quick Start

### Prerequisites

Choose based on your environment:

**Node.js Option** (requires Node.js 18+):
```bash
cd /Users/mimir/Developer/Mimir
node test_rag_playground_medical.js
```

**Python Option** (requires Python 3.8+):
```bash
cd /Users/mimir/Developer/Mimir
python test_rag_playground_medical.py
# or
python3 test_rag_playground_medical.py
```

### Custom URL

To test against a different Mimir instance:

**Node.js**:
```bash
MIMIR_URL=https://mimir.asgard.internal node test_rag_playground_medical.js
```

**Python**:
```bash
MIMIR_URL=https://mimir.asgard.internal python test_rag_playground_medical.py
```

## Test Output Example

```
[2026-05-15T08:35:42.123456] === RAG Studio Test Suite for asgard-medical ===
[2026-05-15T08:35:42.234567] Base URL: http://localhost:3002
[2026-05-15T08:35:42.345678] Tenant: asgard-medical

[2026-05-15T08:35:42.456789] Phase 1: Infrastructure & Connectivity
[2026-05-15T08:35:42.567890] ✓ Health check passed

[2026-05-15T08:35:42.678901] Phase 2: Tenant Verification
[2026-05-15T08:35:42.789012] ✓ Tenant 'asgard-medical' exists (Domain: medical.megacare.com)

[2026-05-15T08:35:42.890123] Phase 3: Document Management
[2026-05-15T08:35:42.901234] ✓ Documents listed: 42 documents found
  - "Cardiology Guidelines 2024" (ID: 1042)
  - "Sleep Medicine Protocols" (ID: 1043)
  - "ENT Management Standards" (ID: 1044)
[2026-05-15T08:35:43.012345] ✓ Document ingested (ID: 1045, Status: indexed)

[2026-05-15T08:35:43.123456] Phase 4: RAG Queries
[2026-05-15T08:35:43.234567] ✓ Vector search successful (Mode: vector, Sources: 3)

  Answer Preview:
  "Respiratory infections present with fever (>38°C), cough (dry or productive), and..."

[2026-05-15T08:35:43.345678] ✓ Hybrid search successful (Mode: hybrid, Sources: 5)
[2026-05-15T08:35:43.456789] ✓ General query successful (Sources: 4)

[2026-05-15T08:35:43.567890] === Test Summary ===

Passed:  7/7
Failed:  0/7
Success: 100.0%
```

## Understanding Test Results

### Success Indicators

- ✓ Symbol indicates a passing test
- Status code 200 for API calls
- Valid JSON responses
- Proper source citations

### Failure Indicators

- ✗ Symbol indicates a failed test
- HTTP error codes (404, 500, etc.)
- Timeout errors (>30 seconds)
- Malformed responses

### Common Issues & Solutions

#### Issue: "Tenant 'asgard-medical' not found"
**Solution:** 
- Verify the medical tenant exists: check if `asgard_medical` is in the database
- Run the recovery script: `mysql < scripts/recover-asgard-tenant.sql`
- Ensure the Bifrost service is running

#### Issue: "Health check failed"
**Solution:**
- Verify the Mimir service is running
- Check the base URL is correct
- Check network connectivity to the service
- Review Mimir logs: `docker logs mimir-ro-ai-bridge`

#### Issue: "Vector search failed"
**Solution:**
- Verify Qdrant vector database is running
- Check that documents are properly indexed
- Verify embedding model is loaded
- Check available free memory (embedding requires ~4GB)

#### Issue: "Document ingestion failed"
**Solution:**
- Ensure MariaDB is running
- Verify tenant has write permissions
- Check disk space for document storage
- Review Mimir logs for detailed error messages

## Advanced Usage

### Running Against Local K8s Deployment

For testing the K8s deployment in OrbStack:

```bash
# Port forward to local service
kubectl port-forward -n asgard-infra svc/mimir-ro-ai-bridge 3002:8080

# In another terminal, run the tests
MIMIR_URL=http://localhost:3002 python test_rag_playground_medical.py
```

### Running Against Production Asgard

For testing the live medical tenant:

```bash
# Ensure you're on the Asgard VPN/Tailscale network
MIMIR_URL=https://mimir.asgard.internal python test_rag_playground_medical.py
```

### Custom Queries

To test with custom medical questions, edit the query strings in:
- **Node.js**: Lines in `testVectorSearch()`, `testHybridSearch()`, `testEmptyQuery()`
- **Python**: Methods `test_vector_search()`, `test_hybrid_search()`, `test_general_query()`

Example custom query:
```python
query = {
    "question": "What are the contraindications for ACE inhibitors?",
    "mode": "vector"
}
```

## Integration with CI/CD

### GitHub Actions

Add to your CI workflow:

```yaml
- name: Test RAG Studio - Medical Tenant
  env:
    MIMIR_URL: http://localhost:3002
  run: |
    python test_rag_playground_medical.py
```

### Manual Testing Before Deployments

Run these tests before deploying medical-domain changes:

```bash
#!/bin/bash
# Pre-deployment verification

echo "Verifying RAG Studio before deployment..."

# Test medical tenant
python /Users/mimir/Developer/Mimir/test_rag_playground_medical.py

if [ $? -eq 0 ]; then
  echo "✓ RAG Studio tests passed - safe to deploy"
  exit 0
else
  echo "✗ RAG Studio tests failed - DO NOT DEPLOY"
  exit 1
fi
```

## Additional Resources

- [Mimir README](./README.md) — Full architecture and features
- [Asgard Multi-Agent Architecture](./docs/03_implementation_plans/)
- [Medical Domain Strategy](./docs/03_implementation_plans/medical_agents_strategy_20260430.md)
- [Tenant API Tests (Rust)](./ro-ai-bridge/tests/tenant_api_tests.rs) — Integration tests reference

## Support

For issues or improvements:

1. Check the Mimir logs: `docker logs mimir-ro-ai-bridge`
2. Verify database connectivity: `mysql -u mimir -p -e "SELECT VERSION();"`
3. Check Qdrant status: `curl http://localhost:6333/health`
4. Review the test script output for specific error messages

---

**Last Updated:** 2026-05-15
**Test Suite Version:** 1.0
**Tested Against:** Mimir v1.x, Python 3.8+, Node.js 18+
