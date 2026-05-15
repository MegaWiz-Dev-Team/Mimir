# Mimir Testing Guide

Complete testing suite for verifying RAG Studio and Agent Studio functionality on the `asgard-medical` tenant.

---

## 📦 Test Suites Overview

This guide includes four comprehensive test suites:

### 1. **RAG Playground Tests** 🔍
Tests RAG (Retrieval-Augmented Generation) functionality including document ingestion, vector search, and hybrid search.

- **Files**: `test_rag_playground_medical.{js,py}`
- **Tests**: 7 test cases covering health checks, document management, and RAG queries
- **Use Case**: Verify document indexing, semantic search, and answer generation

### 2. **Agent Studio API Tests** 🤖
Tests agent management, chat functionality, and agent routing.

- **Files**: `test_agents_api_medical.{js,py}`
- **Tests**: 9 test cases covering agent discovery, chat, and routing
- **Use Case**: Verify agent functionality, specialty routing, and conversation management

### 3. **Unified Test Runner** 🏃
Shell script that auto-detects available runtime and runs either test suite.

- **File**: `run_rag_tests.sh`
- **Features**: Auto-detection of Node.js/Python, colored output, error handling

### 4. **Test Documentation** 📚
- `RAG_PLAYGROUND_TEST_GUIDE.md` — RAG playground detailed guide
- `AGENTS_API_TEST_GUIDE.md` — Agent Studio API detailed guide
- `TESTING_GUIDE.md` — This comprehensive guide

---

## 🚀 Quick Start

### Option 1: Unified Runner (Recommended)

```bash
cd /Users/mimir/Developer/Mimir

# Run all RAG tests (default)
./run_rag_tests.sh

# Run agent tests
./run_rag_tests.sh agents

# Specify runtime explicitly
./run_rag_tests.sh rag python
./run_rag_tests.sh agents node
```

### Option 2: Direct Execution

**RAG Playground:**
```bash
node test_rag_playground_medical.js
python test_rag_playground_medical.py
```

**Agent Studio API:**
```bash
node test_agents_api_medical.js
python test_agents_api_medical.py
```

### Option 3: With Custom URL

```bash
MIMIR_URL=https://mimir.asgard.internal ./run_rag_tests.sh
MIMIR_URL=http://localhost:3002 node test_agents_api_medical.js
```

---

## 📋 Test Matrix

| Test Suite | RAG Playground | Agent Studio |
|-----------|---|---|
| Health Check | ✓ | - |
| Tenant Verification | ✓ | - |
| Document Listing | ✓ | - |
| Document Ingestion | ✓ | - |
| Vector Search | ✓ | - |
| Hybrid Search | ✓ | - |
| General Query | ✓ | - |
| Agent Discovery | - | ✓ |
| Agent Templates | - | ✓ |
| Agent Chat | - | ✓ |
| Conversations | - | ✓ |
| Agent Routing | - | ✓ |
| Specialty Filtering | - | ✓ |

---

## 🔧 System Requirements

### Node.js Tests
- **Runtime**: Node.js 18+ (or 16+ with --experimental-fetch)
- **No dependencies**: Uses built-in fetch API
- **Install**: `brew install node` or download from nodejs.org

### Python Tests
- **Runtime**: Python 3.8+
- **Dependencies**: `requests` library
- **Install**: `pip install requests`

### Supported Platforms
- macOS (Intel & Apple Silicon)
- Linux (Ubuntu, Debian, etc.)
- Windows (WSL2 recommended)

---

## 📊 Test Output Examples

### RAG Playground Success
```
[2026-05-15T08:35:42.123456] === RAG Studio Test Suite for asgard-medical ===
[2026-05-15T08:35:42.234567] ✓ Health check passed
[2026-05-15T08:35:42.345678] ✓ Tenant 'asgard-medical' exists (Domain: medical.megacare.com)
[2026-05-15T08:35:42.456789] ✓ Documents listed: 42 documents found
[2026-05-15T08:35:43.567890] ✓ Vector search successful (Mode: vector, Sources: 3)
[2026-05-15T08:35:43.678901] === Test Summary ===

Passed:  7/7
Failed:  0/7
Success: 100.0%
```

### Agent Studio Success
```
[2026-05-15T08:45:30.123456] === Agent Studio API Test Suite ===
[2026-05-15T08:45:30.567890] ✓ Found 6 agents
[2026-05-15T08:45:30.678901] ✓ Found 8 agent templates
[2026-05-15T08:45:32.234567] ✓ Chat successful (Conv: conv-123456)
[2026-05-15T08:45:32.345678] ✓ Routed to specialist: 'Eir — Cardiology Specialist'
[2026-05-15T08:45:32.456789] === Test Summary ===

Passed:  9/9
Failed:  0/9
Success: 100.0%
```

---

## 🐛 Troubleshooting

### "Command not found: ./run_rag_tests.sh"
```bash
chmod +x /Users/mimir/Developer/Mimir/run_rag_tests.sh
./run_rag_tests.sh
```

### "MIMIR_URL connection refused"
Ensure the Mimir service is running:
```bash
# Local K8s deployment
kubectl port-forward -n asgard-infra svc/mimir-ro-ai-bridge 3002:8080

# Docker Compose
docker-compose up -d

# Verify service is running
curl http://localhost:3002/health
```

### "Python requests library not found"
```bash
pip install requests
# or
pip3 install requests
```

### "No agents found"
Check if the medical tenant was properly initialized:
```bash
mysql < scripts/recover-asgard-tenant.sql
# Restart Bifrost to reload agent configs
```

### Tests timeout
Increase timeout and retry:
```bash
TEST_TIMEOUT=60000 node test_agents_api_medical.js
MIMIR_URL=http://localhost:3002 python test_rag_playground_medical.py
```

---

## 🔄 CI/CD Integration

### GitHub Actions Workflow

```yaml
name: Mimir Test Suite
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      mimir:
        image: ghcr.io/megawiz-dev-team/mimir:latest
        ports:
          - 3002:8080

    steps:
      - uses: actions/checkout@v3
      
      - name: Test RAG Studio
        env:
          MIMIR_URL: http://localhost:3002
        run: python test_rag_playground_medical.py
        
      - name: Test Agent Studio
        env:
          MIMIR_URL: http://localhost:3002
        run: python test_agents_api_medical.py
```

### Pre-deployment Verification

Create a script to run before deployments:

```bash
#!/bin/bash
set -e

echo "🧪 Running pre-deployment verification..."

# Test RAG functionality
python test_rag_playground_medical.py || exit 1

# Test Agent functionality
python test_agents_api_medical.py || exit 1

echo "✓ All tests passed - safe to deploy"
```

---

## 📈 Performance Monitoring

### Expected Response Times

| Operation | Expected | Acceptable |
|-----------|----------|-----------|
| Health check | <50ms | <200ms |
| List agents | <100ms | <500ms |
| Get agent config | <50ms | <200ms |
| Document list | <100ms | <500ms |
| Vector search | 1-3s | <10s |
| Agent chat | 2-8s | <30s |
| Agent routing | 1-5s | <15s |

### Monitoring Test Performance

Add performance tracking to tests:

```bash
# Use 'time' command to measure execution
time python test_rag_playground_medical.py
time node test_agents_api_medical.js

# Example output:
# real    0m12.345s
# user    0m1.234s
# sys     0m0.567s
```

---

## 🔐 Security Considerations

### Authentication
- Tests use JWT tokens internally
- No credentials stored in test files
- Use environment variables for sensitive data

### Data Isolation
- Tests run within `asgard-medical` tenant scope
- No data leakage between tenants
- Conversations are scoped to agents

### Privacy
- Test data is isolated and temporary
- Documents ingested for testing can be deleted
- Conversations don't persist sensitive information

---

## 📚 Advanced Usage

### Custom Query Testing

Edit test files to add custom medical queries:

**JavaScript** (test_rag_playground_medical.js):
```javascript
const query = {
  question: "What are contraindications for ACE inhibitors?",
  mode: "vector"
};
```

**Python** (test_rag_playground_medical.py):
```python
query = {
    "question": "What are contraindications for ACE inhibitors?",
    "mode": "vector"
}
```

### Batch Testing

Run all test suites and generate report:

```bash
#!/bin/bash
REPORT="test_report_$(date +%Y%m%d_%H%M%S).txt"

echo "=== Mimir Test Report ===" > $REPORT
echo "Date: $(date)" >> $REPORT
echo "" >> $REPORT

echo "RAG Playground Tests:" >> $REPORT
python test_rag_playground_medical.py >> $REPORT 2>&1

echo -e "\n\nAgent Studio Tests:" >> $REPORT
python test_agents_api_medical.py >> $REPORT 2>&1

echo "Report saved to: $REPORT"
cat $REPORT
```

### Load Testing

Extend tests for load testing:

```bash
#!/bin/bash
for i in {1..10}; do
  echo "Run $i..."
  python test_rag_playground_medical.py
  sleep 2
done
```

---

## 🎯 Test Maintenance

### When to Run Tests

- **Before deployment**: Verify system is healthy
- **After configuration changes**: Ensure nothing broke
- **During development**: Validate new features
- **Scheduled**: Daily/weekly health checks
- **On-demand**: When troubleshooting issues

### Updating Tests

Tests should be updated when:
- New endpoints are added
- API response formats change
- New agent specialties are introduced
- Test coverage needs expansion

### Adding New Tests

To add a new test to an existing suite:

1. Open the relevant test file
2. Add a new `async function test*()` method
3. Call the function in `runAllTests()`
4. Update documentation with new test

---

## 📞 Support & Issues

### Checking Logs

View Mimir service logs for debugging:

```bash
# Docker Compose
docker logs mimir-ro-ai-bridge

# Kubernetes
kubectl logs -n asgard-infra deploy/mimir-ro-ai-bridge

# System logs
tail -f /var/log/mimir.log
```

### Common Issues & Solutions

See detailed guides:
- [RAG Playground Troubleshooting](./RAG_PLAYGROUND_TEST_GUIDE.md#troubleshooting)
- [Agent Studio Troubleshooting](./AGENTS_API_TEST_GUIDE.md#troubleshooting)

### Reporting Issues

When reporting test failures, include:
1. Test output (copy entire output)
2. MIMIR_URL being tested
3. System information (OS, Node/Python version)
4. Recent changes to the system
5. Relevant logs from Mimir service

---

## 📖 Related Documentation

- [RAG Playground Test Guide](./RAG_PLAYGROUND_TEST_GUIDE.md) — Detailed RAG testing
- [Agent Studio API Guide](./AGENTS_API_TEST_GUIDE.md) — Agent API endpoints
- [Mimir README](./README.md) — Architecture and features
- [Medical Agents Strategy](./docs/03_implementation_plans/medical_agents_strategy_20260430.md)
- [Tenant API Tests (Rust)](./ro-ai-bridge/tests/tenant_api_tests.rs)

---

**Last Updated:** 2026-05-15  
**Version:** 1.0  
**Tested Environments:** 
- macOS 12+, Python 3.8+, Node.js 18+
- Ubuntu 20.04+, Python 3.8+, Node.js 18+
- K3s in OrbStack, Mimir v1.x
