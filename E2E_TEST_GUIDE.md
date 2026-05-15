# End-to-End (E2E) Integration Test Guide

Comprehensive integration tests that verify complete workflows across RAG and Agent Studio systems.

---

## Overview

E2E tests simulate real-world medical scenarios, combining document management, semantic search, agent conversations, and intelligent routing.

### Test Scenarios Covered

| Workflow | Description | Systems Tested |
|----------|-------------|-----------------|
| **WF1** | Document Ingestion → RAG Query | RAG, Vector DB, Document Storage |
| **WF2** | Multi-turn Agent Chat | Agents, Conversations, Context |
| **WF3** | Question Routing | Routing Agent, Specialist Selection |
| **WF4** | Cross-system Integration | RAG + Agents, Augmented Responses |

---

## Quick Start

### Run E2E Tests

```bash
# Using unified test runner
./run_rag_tests.sh e2e

# Or run directly
python test_e2e_medical_workflow.py
node test_e2e_medical_workflow.js

# Custom URL
MIMIR_URL=https://mimir.asgard.internal python test_e2e_medical_workflow.py
```

---

## Workflow Details

### Workflow 1: Document Ingestion → RAG Query

**Scenario:** A hospital needs to index new hypertension guidelines and query them.

**Steps:**
1. **Ingest Document** — Upload "Hypertension Management Guidelines 2026"
   - Title, content, source tracking
   - Automatic chunking and embedding
   - Storage in vector database

2. **RAG Query** — Ask: "What are the main treatment options for hypertension?"
   - Vector similarity search
   - Source document retrieval
   - Answer generation with citations

3. **Verify Listing** — Confirm document appears in inventory
   - Document metadata validation
   - Count verification

4. **Hybrid Search** — Test combined vector + graph search
   - Query: "What are ACE inhibitors and their side effects?"
   - Multiple retrieval modes
   - Mode selection verification

**Expected Output:**
```
✓ Document ingested (ID: 1234, Status: indexed)
✓ RAG query successful (3 sources found)
  - Hypertension Management Guidelines 2026 (Relevance: 0.95)
  Answer: "Main treatment options include ACE inhibitors, beta-blockers..."
✓ Document found in listing (Total: 42)
✓ Hybrid search successful (Mode: hybrid)
```

---

### Workflow 2: Multi-turn Agent Chat

**Scenario:** Patient has multiple questions about hypertension treatment.

**Steps:**
1. **Initialize Agent** — Select generic medical agent
2. **Turn 1** — "What are the main symptoms of hypertension?"
3. **Turn 2** — "What medications are commonly used?"
   - Maintains conversation context
   - Builds on previous responses
4. **Turn 3** — "What are the side effects I should watch for?"
   - Continued conversation
   - Contextual awareness
5. **Verify History** — Confirm conversation persisted

**Expected Output:**
```
✓ Using agent: Eir — Generic Medical Agent
✓ Turn 1 successful (Conv: conv-2026051508453012345)
  Agent: "Hypertension symptoms often include headaches, fatigue..."
✓ Turn 2 successful (same conversation)
  Agent: "Common medications include ACE inhibitors like Lisinopril..."
✓ Turn 3 successful (conversation continued)
  Agent: "Side effects vary by medication class..."
✓ Conversation verified in history (7 total)
```

---

### Workflow 3: Intelligent Agent Routing

**Scenario:** Patients with different conditions need appropriate specialists.

**Test Cases:**

**Case 1: Cardiology**
- Question: "Chest pain, shortness of breath, irregular heartbeat"
- Expected Route: Cardiology specialist (eir-cardio)
- Confidence: High (>0.9)

**Case 2: Sleep Medicine**
- Question: "Insomnia, snoring loudly, gasping for air during sleep"
- Expected Route: Sleep specialist (eir-sleep)
- Confidence: High (>0.85)

**Case 3: Pediatrics**
- Question: "Child with high fever, cough, ear pain"
- Expected Route: Pediatrics specialist (eir-pediatrics)
- Confidence: Variable but valid

**Expected Output:**
```
✓ Correctly routed to cardiology (Confidence: 0.98)
✓ Correctly routed to sleep medicine (Confidence: 0.92)
✓ Routed to: pediatrics
```

---

### Workflow 4: Cross-system Integration

**Scenario:** Agent provides answers augmented by RAG knowledge base.

**Steps:**
1. **RAG Query** — Search knowledge base for specific info
   - Query: "What are contraindications for ACE inhibitors?"
   - Returns sources and context

2. **Agent Chat** — Ask agent same question
   - Uses RAG augmentation
   - Provides grounded answers
   - Includes source citations

**Expected Output:**
```
✓ RAG provided 2 sources
✓ Agent provided RAG-augmented answer (2 sources)
  Agent: "ACE inhibitors should be avoided in patients with..."
```

---

## Test Execution Timeline

Expected execution times for each workflow:

| Workflow | Typical Time | Max Expected |
|----------|--------------|--------------|
| WF1 (Document RAG) | 5-8 seconds | 15 seconds |
| WF2 (Multi-turn Chat) | 8-12 seconds | 20 seconds |
| WF3 (Agent Routing) | 4-6 seconds | 12 seconds |
| WF4 (Cross-system) | 3-5 seconds | 10 seconds |
| **Total** | **20-31 seconds** | **57 seconds** |

---

## Test Output Structure

### Success Example
```
[2026-05-15T08:50:00.123456] ╔═════════════════════════════════════════════════════╗
[2026-05-15T08:50:00.234567] ║  End-to-End Integration Tests - asgard-medical      ║
[2026-05-15T08:50:00.345678] ╚═════════════════════════════════════════════════════╝

[2026-05-15T08:50:00.456789] === E2E Workflow 1: Document Ingestion → RAG Query ===
[2026-05-15T08:50:00.567890] Step 1/4: Ingesting medical document...
[2026-05-15T08:50:01.678901] ✓ Document ingested (ID: 1234, Status: indexed)
[2026-05-15T08:50:01.789012] Step 2/4: Querying RAG system...
[2026-05-15T08:50:02.890123] ✓ RAG query successful (3 sources found)

[2026-05-15T08:50:35.012345] === End-to-End Test Summary ===

Passed:  16/16
Failed:  0/16
Success: 100.0%

⏱️  Total Time: 35.23 seconds
```

### Failure Handling

Tests are resilient and provide clear error messages:

```
[2026-05-15T08:50:00.456789] Step 1/4: Ingesting medical document...
[2026-05-15T08:50:01.567890] ✗ Document ingestion failed
  Error: Connect timeout - ensure Mimir service is running

[2026-05-15T08:50:01.678901] === End-to-End Test Summary ===

Passed:  0/16
Failed:  1/16
Success: 0.0%
```

---

## Troubleshooting

### Common Issues

#### Issue: "Document ingestion failed"
**Causes:**
- Mimir service not running
- Database connection issue
- Disk space full

**Solutions:**
```bash
# Check service status
curl http://localhost:3002/health

# Start Mimir service
docker-compose up -d

# Check MariaDB connection
mysql -u mimir -p -e "SELECT COUNT(*) FROM documents;"
```

#### Issue: "Agent chat timeout"
**Causes:**
- LLM model not loaded
- Slow network connection
- High system load

**Solutions:**
```bash
# Increase timeout
TEST_TIMEOUT=120000 python test_e2e_medical_workflow.py

# Check model status
curl http://localhost:8081/health  # Heimdall/MLX server

# Monitor system resources
top  # or Activity Monitor on macOS
```

#### Issue: "Routing confidence too low"
**Causes:**
- Ambiguous medical questions
- Underspecified symptoms
- Router agent misconfigured

**Solutions:**
- Use more specific medical terminology
- Include detailed symptom descriptions
- Check router agent configuration in database

#### Issue: "Conversation not found in history"
**Causes:**
- Eventual consistency delay
- Database replication lag
- Conversation not persisted

**Solutions:**
```bash
# Wait and retry
sleep 2 && python test_e2e_medical_workflow.py

# Check database directly
mysql -u mimir -p -e "SELECT * FROM conversations LIMIT 5;"
```

---

## Performance Optimization

### For Faster Test Execution

1. **Reduce payload sizes** in Workflow 4
2. **Use local LLM** instead of cloud models
3. **Enable caching** in vector database
4. **Pre-warm connections** before running tests

### Database Tuning

```sql
-- Index optimization for faster queries
CREATE INDEX idx_documents_tenant ON documents(tenant_id);
CREATE INDEX idx_conversations_agent ON conversations(agent_id);
CREATE INDEX idx_vectors_embedding ON vectors(embedding_id);
```

---

## CI/CD Integration

### GitHub Actions Example

```yaml
name: E2E Integration Tests
on: [push, pull_request]

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    
    services:
      mimir:
        image: ghcr.io/megawiz-dev-team/mimir:latest
        ports:
          - 3002:8080
      mariadb:
        image: mariadb:latest
        env:
          MYSQL_ROOT_PASSWORD: root
          MYSQL_DATABASE: mimir

    steps:
      - uses: actions/checkout@v3
      
      - name: Wait for services
        run: |
          for i in {1..30}; do
            curl -f http://localhost:3002/health && break
            sleep 1
          done
      
      - name: Run E2E Tests
        env:
          MIMIR_URL: http://localhost:3002
        run: python test_e2e_medical_workflow.py
```

### Pre-deployment Checklist

```bash
#!/bin/bash
set -e

echo "🧪 Pre-deployment E2E Test Suite"

# 1. RAG functionality
python test_rag_playground_medical.py || exit 1

# 2. Agent functionality
python test_agents_api_medical.py || exit 1

# 3. Full E2E workflows
python test_e2e_medical_workflow.py || exit 1

echo "✅ All tests passed - safe to deploy"
```

---

## Advanced Testing

### Custom Medical Scenarios

Edit test file to add domain-specific workflows:

```python
# Example: Heart failure case
custom_scenario = {
    "document": {
        "title": "Heart Failure Management Protocol",
        "content": "..."
    },
    "questions": [
        "What are early signs of heart failure?",
        "Which medications reduce mortality?",
        "When is transplantation indicated?"
    ],
    "expected_route": "eir-cardio"
}
```

### Load Testing E2E

```bash
#!/bin/bash
# Run E2E tests multiple times with varying delays

for i in {1..5}; do
  echo "Run $i..."
  time python test_e2e_medical_workflow.py
  sleep 10  # Allow system recovery
done
```

### Network Simulation

Test behavior with latency/packet loss:

```bash
# On macOS
sudo ipfw pipe 1 config bw 1Mbit/s delay 100

# Run tests
MIMIR_URL=http://localhost:3002 python test_e2e_medical_workflow.py

# Remove simulation
sudo ipfw delete 1
```

---

## Monitoring & Metrics

### Key Metrics to Track

- **Document ingestion rate** (docs/sec)
- **Query latency** (RAG mode)
- **Agent response time** (ms)
- **Routing accuracy** (% correct)
- **Conversation persistence** (success rate)

### Database Queries for Analysis

```sql
-- Ingestion performance
SELECT COUNT(*) as doc_count, 
       MAX(created_at) as latest 
FROM documents 
WHERE created_at > DATE_SUB(NOW(), INTERVAL 1 HOUR);

-- Conversation statistics
SELECT agent_id, COUNT(*) as conversation_count 
FROM conversations 
GROUP BY agent_id;

-- Query performance
SELECT mode, AVG(response_time_ms) as avg_time 
FROM query_logs 
GROUP BY mode;
```

---

## Maintenance

### When to Update Tests

- New medical specialties added
- API response format changes
- Knowledge base schema changes
- Agent capabilities expanded
- Routing logic modified

### Test Maintenance Schedule

| Task | Frequency | Owner |
|------|-----------|-------|
| Run full suite | Daily | CI/CD |
| Review results | Weekly | QA |
| Update scenarios | Monthly | Medical team |
| Refactor tests | Quarterly | Engineering |

---

## Support & Documentation

### Related Guides

- [TESTING_GUIDE.md](./TESTING_GUIDE.md) — All test suites overview
- [RAG_PLAYGROUND_TEST_GUIDE.md](./RAG_PLAYGROUND_TEST_GUIDE.md) — RAG-specific testing
- [AGENTS_API_TEST_GUIDE.md](./AGENTS_API_TEST_GUIDE.md) — Agent API details
- [Mimir README](./README.md) — Architecture

### Getting Help

For E2E test issues:

1. **Check logs:**
   ```bash
   docker logs mimir-ro-ai-bridge
   kubectl logs -n asgard-infra deploy/mimir-ro-ai-bridge
   ```

2. **Verify connectivity:**
   ```bash
   curl http://localhost:3002/health
   curl http://localhost:6333/health  # Qdrant
   ```

3. **Run individual workflows:**
   Edit test file to comment out workflows and test one at a time

---

**Last Updated:** 2026-05-15  
**Version:** 1.0  
**Status:** Production Ready  
**Test Coverage:** 16 test cases across 4 workflows
