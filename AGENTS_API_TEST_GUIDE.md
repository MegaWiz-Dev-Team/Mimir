# Agent Studio API Test Guide

## Overview

This guide documents the Agent Studio API and provides automated test suites to verify agent management and chat functionality on the `asgard-medical` tenant.

## What is the Agent Studio API?

The Agent Studio API (`/api/v1/agents`) provides endpoints for:

- **Agent Management**: CRUD operations on agent configurations
- **Agent Discovery**: List, filter, and search available agents
- **Agent Chat**: Send messages to agents and receive responses
- **Conversation Management**: Track and retrieve conversation history
- **Agent Routing**: Automatically route questions to specialist agents
- **Templates**: Access pre-built agent templates

---

## API Endpoints Reference

### List Agents
```
GET /api/v1/agents
Query Parameters:
  - specialty: Filter by specialty (e.g., "cardiology", "sleep", "pediatrics")
  - limit: Number of results (default: 50)
  - skip: Pagination offset
```

**Example Response:**
```json
[
  {
    "id": "eir-cardio",
    "name": "Eir — Cardiology Specialist",
    "specialty": "cardiology",
    "model_id": "mlx-community/Qwen3.5-9B-MLX-4bit",
    "temperature": 0.3,
    "use_rag": true,
    "description": "Specialized in cardiovascular medicine"
  },
  ...
]
```

### Get Agent Templates
```
GET /api/v1/agents/templates
```

Returns available templates for creating new agents (e.g., "cardiology", "sleep", "pediatrics", "generic").

### Get Specific Agent
```
GET /api/v1/agents/:id

Parameters:
  - id: Agent ID (e.g., "eir-cardio", "eir-sleep")
```

### Chat with Agent
```
POST /api/v1/agents/:id/chat

Request Body:
{
  "message": "Your question here",
  "model": "auto",  // or specific model ID
  "mode": "chat"    // or "rag" for context-aware responses
}
```

**Response:**
```json
{
  "response": "Agent's response text...",
  "conversation_id": "conv-123456",
  "sources": [
    {
      "title": "Document Title",
      "page": 1,
      "relevance": 0.95
    }
  ]
}
```

### List Agent Conversations
```
GET /api/v1/agents/:id/conversations

Query Parameters:
  - limit: Number of conversations to return
  - skip: Pagination offset
```

### Route Question to Specialist
```
POST /api/v1/agents/route

Request Body:
{
  "question": "Your medical question...",
  "tenant_id": "asgard-medical"
}
```

**Response:**
```json
{
  "selected_agent": {
    "id": "eir-cardio",
    "name": "Eir — Cardiology Specialist",
    "specialty": "cardiology"
  },
  "confidence": 0.95,
  "reasoning": "Question mentions cardiovascular symptoms..."
}
```

---

## Quick Start

### Run Tests

**Option 1 — Auto-detect & Run:**
```bash
cd /Users/mimir/Developer/Mimir
./run_rag_tests.sh agents
```

**Option 2 — Run with Node.js:**
```bash
node test_agents_api_medical.js
```

**Option 3 — Run with Python:**
```bash
python test_agents_api_medical.py
```

### Custom URL

Test against a different Mimir instance:

```bash
MIMIR_URL=https://mimir.asgard.internal python test_agents_api_medical.py
```

---

## Test Suite Details

### What Gets Tested

1. **Phase 1: Agent Discovery**
   - ✓ List all agents
   - ✓ Get agent templates
   - ✓ Verify agent count and metadata

2. **Phase 2: Agent Management**
   - ✓ Get specific agent configuration
   - ✓ Verify agent properties (model, specialty, RAG settings)

3. **Phase 3: Agent Chat & Conversations**
   - ✓ Send chat message to agent
   - ✓ Receive response with context
   - ✓ List conversation history
   - ✓ Verify conversation metadata

4. **Phase 4: Advanced Features**
   - ✓ Agent routing (question → specialist)
   - ✓ Specialty filtering
   - ✓ Pagination support

### Example Test Output

```
[2026-05-15T08:45:30.123456] === Agent Studio API Test Suite ===
[2026-05-15T08:45:30.234567] Base URL: http://localhost:3002
[2026-05-15T08:45:30.345678] Tenant: asgard-medical

[2026-05-15T08:45:30.456789] Phase 1: Agent Discovery
[2026-05-15T08:45:30.567890] ✓ Found 6 agents
  Agent ID                   Name                           Specialty            Model
  eir                        Eir — Generic Medical Agent    general              mlx-community/Qwen3.5-9B-MLX-4bit
  eir-cardio                 Eir — Cardiology Specialist    cardiology           mlx-community/Qwen3.5-9B-MLX-4bit
  eir-sleep                  Eir — Sleep Medicine Spec...   sleep                mlx-community/Qwen3.5-9B-MLX-4bit

[2026-05-15T08:45:30.678901] ✓ Found 8 agent templates
  - generic: General-purpose medical AI assistant
  - cardiology: Specialized in heart and cardiovascular diseases
  - sleep: Sleep medicine and sleep disorders specialist

[2026-05-15T08:45:31.789012] Phase 2: Agent Management
[2026-05-15T08:45:31.890123] ✓ Retrieved agent 'Eir — Generic Medical Agent' (RAG: true, Temp: 0.3)

[2026-05-15T08:45:32.901234] Phase 3: Agent Chat & Conversations
[2026-05-15T08:45:32.012345] ✓ Chat successful (Conv: conv-2026051508453012345)

  Response Preview:
  "Common respiratory infection symptoms include fever (temperature >38°C), cough (dry or productive)..."

[2026-05-15T08:45:32.123456] ✓ Retrieved 3 conversations for agent

[2026-05-15T08:45:32.234567] Phase 4: Advanced Features
[2026-05-15T08:45:32.345678] ✓ Routed to specialist: 'Eir — Cardiology Specialist' (cardiology) - Confidence: 0.98
[2026-05-15T08:45:32.456789] ✓ Filtered agents by specialty: 1 results

[2026-05-15T08:45:32.567890] === Test Summary ===

Passed:  9/9
Failed:  0/9
Success: 100.0%
```

---

## Medical Agents Available

The `asgard-medical` tenant includes specialized agents for:

| Agent ID | Name | Specialty | Purpose |
|----------|------|-----------|---------|
| eir | Generic Medical AI | General | All-purpose medical questions |
| eir-cardio | Cardiology Specialist | Cardiology | Cardiovascular diseases, heart conditions |
| eir-sleep | Sleep Medicine | Sleep | Sleep disorders, CPAP management |
| eir-ent | ENT Specialist | ENT | Ear, nose, throat conditions |
| eir-pediatrics | Pediatrics Specialist | Pediatrics | Children's health, pediatric conditions |
| eir-router | Question Router | Routing | Directs questions to appropriate specialist |

---

## Common Use Cases

### 1. Get All Cardiology Agents
```bash
curl -s http://localhost:3002/api/v1/agents?specialty=cardiology | jq '.'
```

### 2. Chat with Sleep Medicine Specialist
```bash
curl -X POST http://localhost:3002/api/v1/agents/eir-sleep/chat \
  -H "Content-Type: application/json" \
  -d '{
    "message": "What are the risks of untreated sleep apnea?",
    "mode": "rag"
  }' | jq '.'
```

### 3. Route Complex Question
```bash
curl -X POST http://localhost:3002/api/v1/agents/route \
  -H "Content-Type: application/json" \
  -d '{
    "question": "My child has a persistent cough and ear pain. Which specialist should evaluate this?",
    "tenant_id": "asgard-medical"
  }' | jq '.'
```

### 4. View Conversation History
```bash
curl http://localhost:3002/api/v1/agents/eir-cardio/conversations?limit=10 | jq '.'
```

---

## Troubleshooting

### Issue: "No agents found"
**Solution:**
- Verify the tenant `asgard-medical` exists in the database
- Run the recovery script: `mysql < scripts/recover-asgard-tenant.sql`
- Check if Bifrost service is running
- Verify database connectivity

### Issue: "Agent chat timeout"
**Solution:**
- Increase test timeout in environment: `TEST_TIMEOUT=60000`
- Verify LLM model is loaded and accessible
- Check Heimdall service status (model server)
- Review available system resources (RAM, GPU memory)

### Issue: "Routing confidence too low"
**Solution:**
- Ensure the question is clearly related to medical domain
- Verify RAG knowledge base is populated
- Check router agent configuration in database

### Issue: "Conversation list is empty"
**Solution:**
- Chat with the agent first to create a conversation
- Verify conversation persistence settings
- Check MariaDB storage for conversation records

---

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Agent Studio API Tests
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
      
      - name: Test Agent Studio API
        env:
          MIMIR_URL: http://localhost:3002
        run: |
          python test_agents_api_medical.py
```

---

## Performance Considerations

### Response Times

- **List agents**: <100ms
- **Get agent config**: <50ms
- **Chat request**: 2-10s (depends on LLM model)
- **Agent routing**: 1-5s (decision-making overhead)
- **List conversations**: <200ms

### Optimization Tips

1. **Use agent routing** for complex questions instead of fixed agent selection
2. **Cache agent configurations** if listing frequently
3. **Batch conversations** retrieval with pagination
4. **Use model: "auto"** to let agent choose optimal model

---

## Security Notes

- Agent responses are scoped to tenant data
- Chat conversations are encrypted at rest
- Agent routing uses tenant-scoped knowledge base
- API requires proper authentication via JWT tokens
- Conversation history is retained per agent per tenant

---

## Related Resources

- [RAG Playground Test Guide](./RAG_PLAYGROUND_TEST_GUIDE.md)
- [Mimir README](./README.md)
- [Medical Agents Strategy](./docs/03_implementation_plans/medical_agents_strategy_20260430.md)
- [Tenant API Tests](./ro-ai-bridge/tests/tenant_api_tests.rs)

---

**Last Updated:** 2026-05-15
**API Version:** v1
**Test Suite Version:** 1.0
**Tested Against:** Mimir 1.x, Node.js 18+, Python 3.8+
