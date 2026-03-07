# ⚡ Bifrost — Agent Runtime Engine

> *"The burning rainbow bridge that connects the realms"*
>
> Bifrost is a self-hosted Agent Runtime Engine for deploying, executing, and managing AI agents with tool-use capabilities. Designed to work with [Heimdall](https://github.com/megacare-dev/Project-Mimir) (LLM Gateway) and [Mimir](https://github.com/megacare-dev/Project-Mimir) (RAG Pipeline & Agent Builder) as part of the **Project Mimir** ecosystem.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Core Features](#core-features)
- [Tech Stack](#tech-stack)
- [Project Structure](#project-structure)
- [API Reference](#api-reference)
- [Configuration](#configuration)
- [Built-in Tools](#built-in-tools)
- [Custom Tools](#custom-tools)
- [Deployment](#deployment)
- [Roadmap](#roadmap)
- [Related Projects](#related-projects)

---

## Overview

### Problem

Mimir's Agent Builder lets users create agents (system prompt, model, temperature), but execution is limited to **single-turn LLM calls**. Agents cannot:

- Call external tools or APIs
- Execute multi-step reasoning (ReAct loop)
- Search the RAG knowledge base autonomously
- Maintain long-term memory across sessions
- Delegate to other agents

### Solution

Bifrost provides a **managed runtime** that takes agent configs from Mimir and executes them as autonomous agents with full tool-use capabilities.

```
Mimir (Agent Builder) → deploys to → Bifrost (Agent Runtime) → calls → Heimdall (LLM Gateway)
```

---

## Architecture

### System Context

```
┌─────────────────────────────────────────────────────────────────┐
│                        Project Mimir Ecosystem                  │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │   Mimir       │    │   Bifrost     │    │   Heimdall   │       │
│  │              │    │              │    │              │       │
│  │  RAG Pipeline │───▶│ Agent Runtime│───▶│ LLM Gateway  │       │
│  │  Agent Builder│◀───│ Tool Execute │    │ Multi-backend│       │
│  │  Dashboard    │    │ Session Mgmt │    │ Auth/Metrics │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│         │                   │                    │               │
│      SQLite             SQLite              MLX/llama.cpp        │
│                                              /Ollama             │
└─────────────────────────────────────────────────────────────────┘
```

### Internal Architecture

```
                         ┌─────────────────────────┐
                         │      Bifrost Server      │
                         │      (FastAPI/Uvicorn)    │
                         └────────────┬────────────┘
                                      │
              ┌───────────────────────┼───────────────────────┐
              │                       │                       │
     ┌────────▼────────┐    ┌────────▼────────┐    ┌────────▼────────┐
     │  Agent Executor  │    │  Tool Registry   │    │ Session Manager │
     │                  │    │                  │    │                 │
     │ • ReAct loop     │    │ • Built-in tools │    │ • Short-term    │
     │ • Plan-Execute   │    │ • Custom tools   │    │   (conversation)│
     │ • Max iterations │    │ • JSON Schema    │    │ • Long-term     │
     │ • Error recovery │    │ • Sandboxed exec │    │   (memory bank) │
     └─────────────────┘    └─────────────────┘    └─────────────────┘
              │                       │
     ┌────────▼────────┐    ┌────────▼────────┐
     │  Agent Router    │    │  Event Logger    │
     │                  │    │                  │
     │ • Multi-agent    │    │ • Execution trace│
     │ • Handoff/Delegate│   │ • Token usage    │
     │ • Load balancing │    │ • Tool call logs │
     └─────────────────┘    └─────────────────┘
```

---

## Core Features

### 1. Agent Executor (ReAct Loop)

The core execution engine that runs agents in a think-act-observe loop:

```
User Input
    │
    ▼
┌─────────────────────────────────────┐
│  1. Build context                   │
│     (system prompt + tools + history)│
│                                     │
│  2. Call LLM (via Heimdall)         │
│     ├─ tool_call response?          │
│     │   ├─ Execute tool             │
│     │   ├─ Append result            │
│     │   └─ Go to step 2 (loop)     │
│     │                               │
│     └─ final text response?         │
│         └─ Return to user           │
└─────────────────────────────────────┘
```

**Key behaviors:**
- Supports **OpenAI-compatible function calling** format
- Configurable `max_iterations` (default: 10) to prevent infinite loops
- Configurable `max_execution_time` (default: 120s) timeout
- Streaming support via SSE
- Graceful error recovery — if a tool fails, the error is fed back to the LLM

### 2. Tool Registry

Tools are defined as JSON Schema and passed to the LLM's `tools` parameter:

```json
{
  "type": "function",
  "function": {
    "name": "search_knowledge",
    "description": "Search the RAG knowledge base for relevant documents",
    "parameters": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "The search query"
        },
        "top_k": {
          "type": "integer",
          "description": "Number of results to return",
          "default": 5
        }
      },
      "required": ["query"]
    }
  }
}
```

### 3. Session Manager

| Type | Scope | Storage | TTL |
|:--|:--|:--|:--|
| **Short-term** | Per conversation | SQLite | Session lifetime |
| **Long-term** | Per user/agent | SQLite | Configurable (30d default) |

- Short-term: Conversation messages + tool call history
- Long-term: Extracted facts, user preferences, learned patterns

### 4. Agent Router

Enables multi-agent collaboration:

```python
# Agent A can delegate to Agent B
{
  "type": "function",
  "function": {
    "name": "delegate_to_agent",
    "description": "Hand off the conversation to a specialist agent",
    "parameters": {
      "properties": {
        "agent_id": { "type": "string" },
        "message": { "type": "string" },
        "context": { "type": "object" }
      }
    }
  }
}
```

### 5. Execution Tracing

Every agent run produces a structured trace:

```json
{
  "run_id": "run_abc123",
  "agent_id": "agent_42",
  "session_id": "sess_xyz",
  "steps": [
    { "type": "llm_call", "model": "qwen3-8b", "latency_ms": 1200, "tokens": { "in": 500, "out": 120 } },
    { "type": "tool_call", "tool": "search_knowledge", "args": {"query": "..."}, "result": "...", "latency_ms": 89 },
    { "type": "llm_call", "model": "qwen3-8b", "latency_ms": 900, "tokens": { "in": 800, "out": 200 } },
    { "type": "final_answer", "content": "..." }
  ],
  "total_latency_ms": 2189,
  "total_tokens": 1620
}
```

---

## Tech Stack

| Layer | Technology | Rationale |
|:--|:--|:--|
| **Runtime** | Python 3.11+ | Rich AI/agent ecosystem |
| **Framework** | FastAPI + Uvicorn | Async, fast, OpenAPI docs |
| **Database** | SQLite (via aiosqlite) | Consistent with Mimir, zero-ops |
| **LLM Client** | httpx (async) | Calls Heimdall's OpenAI-compatible API |
| **Serialization** | Pydantic v2 | Type safety, JSON Schema generation |
| **Task Queue** | (optional) Celery / ARQ | For long-running agent tasks |
| **Sandbox** | subprocess + resource limits | Sandboxed code execution |

---

## Project Structure

```
bifrost/
├── README.md
├── pyproject.toml              # Project metadata & dependencies
├── .env.example                # Configuration template
├── Dockerfile
├── docker-compose.yml
│
├── bifrost/                    # Main package
│   ├── __init__.py
│   ├── main.py                 # FastAPI app entry point
│   ├── config.py               # Settings (Pydantic BaseSettings)
│   │
│   ├── api/                    # API routes
│   │   ├── __init__.py
│   │   ├── agents.py           # /v1/agents/:id/run, /v1/agents/:id/stream
│   │   ├── sessions.py         # /v1/sessions CRUD
│   │   ├── tools.py            # /v1/tools registry
│   │   └── health.py           # /healthz, /readyz
│   │
│   ├── core/                   # Core execution engine
│   │   ├── __init__.py
│   │   ├── executor.py         # ReAct loop / Plan-Execute
│   │   ├── router.py           # Multi-agent routing
│   │   ├── context.py          # Context builder (system prompt + tools + history)
│   │   └── streaming.py        # SSE streaming handler
│   │
│   ├── tools/                  # Tool implementations
│   │   ├── __init__.py
│   │   ├── registry.py         # Tool registration & discovery
│   │   ├── base.py             # BaseTool abstract class
│   │   ├── rag_search.py       # Search Mimir knowledge base
│   │   ├── http_request.py     # Make HTTP calls
│   │   ├── code_exec.py        # Sandboxed Python execution
│   │   ├── sql_query.py        # Query databases
│   │   ├── datetime_tool.py    # Current time, timezone conversion
│   │   └── math_tool.py        # Calculator
│   │
│   ├── memory/                 # Session & memory management
│   │   ├── __init__.py
│   │   ├── session.py          # Short-term conversation state
│   │   ├── memory_bank.py      # Long-term fact storage
│   │   └── models.py           # DB models
│   │
│   ├── clients/                # External service clients
│   │   ├── __init__.py
│   │   ├── heimdall.py         # LLM Gateway client
│   │   └── mimir.py            # RAG API client
│   │
│   └── db/                     # Database
│       ├── __init__.py
│       ├── connection.py       # SQLite connection pool
│       └── migrations/         # Schema migrations
│
├── tests/                      # Tests
│   ├── conftest.py
│   ├── test_executor.py
│   ├── test_tools.py
│   ├── test_sessions.py
│   └── test_api.py
│
└── scripts/
    ├── setup.sh                # Dev environment setup
    └── seed_tools.py           # Seed built-in tools
```

---

## API Reference

### Agent Execution

#### `POST /v1/agents/{agent_id}/run`

Execute an agent with tool-use capabilities.

```bash
curl -X POST http://localhost:8100/v1/agents/42/run \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "สรุปเอกสารเกี่ยวกับ data migration ให้หน่อย",
    "session_id": "optional-session-id",
    "stream": false
  }'
```

**Response:**
```json
{
  "run_id": "run_abc123",
  "agent_id": 42,
  "session_id": "sess_xyz789",
  "content": "จากการค้นหาใน knowledge base พบเอกสาร 3 รายการ...",
  "model_id": "qwen3-8b",
  "steps": [
    {
      "type": "tool_call",
      "tool": "search_knowledge",
      "args": { "query": "data migration" },
      "result_preview": "Found 3 documents..."
    }
  ],
  "usage": {
    "total_tokens": 1620,
    "input_tokens": 1200,
    "output_tokens": 420,
    "tool_calls": 1,
    "llm_calls": 2,
    "latency_ms": 2189
  }
}
```

#### `POST /v1/agents/{agent_id}/stream`

Same as `/run` but returns SSE stream:

```
event: step
data: {"type": "thinking", "content": "I need to search the knowledge base..."}

event: step
data: {"type": "tool_call", "tool": "search_knowledge", "args": {"query": "data migration"}}

event: step
data: {"type": "tool_result", "tool": "search_knowledge", "result": "Found 3 documents..."}

event: step
data: {"type": "content", "content": "จากการค้นหา"}

event: step
data: {"type": "content", "content": "ใน knowledge base..."}

event: done
data: {"run_id": "run_abc123", "usage": {...}}
```

---

### Sessions

#### `GET /v1/sessions`
List active sessions.

#### `GET /v1/sessions/{session_id}`
Get session details with message history.

#### `DELETE /v1/sessions/{session_id}`
End and archive a session.

---

### Tools

#### `GET /v1/tools`
List all registered tools (built-in + custom).

#### `POST /v1/tools`
Register a custom tool.

```json
{
  "name": "check_order_status",
  "description": "Check the status of a customer order",
  "endpoint": "https://api.example.com/orders/{order_id}",
  "method": "GET",
  "parameters": {
    "type": "object",
    "properties": {
      "order_id": { "type": "string", "description": "The order ID" }
    },
    "required": ["order_id"]
  },
  "auth": {
    "type": "bearer",
    "token_env": "ORDER_API_TOKEN"
  }
}
```

#### `DELETE /v1/tools/{tool_name}`
Unregister a custom tool.

---

### Health

#### `GET /healthz`
Liveness probe.

#### `GET /readyz`
Readiness probe (checks Heimdall + DB connectivity).

---

## Configuration

### `.env` Example

```bash
# ─── Bifrost Server ──────────────────────────────────────
BIFROST_HOST=0.0.0.0
BIFROST_PORT=8100
BIFROST_LOG_LEVEL=info

# ─── Heimdall (LLM Gateway) ─────────────────────────────
HEIMDALL_BASE_URL=http://localhost:8080/v1/
HEIMDALL_API_KEY=your-heimdall-api-key

# ─── Mimir (RAG API) ────────────────────────────────────
MIMIR_BASE_URL=http://localhost:3000/api/v1
MIMIR_API_KEY=your-mimir-api-key

# ─── Database ────────────────────────────────────────────
BIFROST_DB_PATH=./data/bifrost.db

# ─── Execution Limits ───────────────────────────────────
MAX_ITERATIONS=10          # Max tool-call loops per run
MAX_EXECUTION_TIME=120     # Seconds before timeout
MAX_CONTEXT_TOKENS=8192    # Max tokens in context window

# ─── Code Execution Sandbox ─────────────────────────────
CODE_EXEC_ENABLED=false
CODE_EXEC_TIMEOUT=30       # Seconds
CODE_EXEC_MAX_MEMORY=256   # MB

# ─── Auth ────────────────────────────────────────────────
BIFROST_API_KEY=your-bifrost-api-key
```

---

## Built-in Tools

| Tool | Description | Requires |
|:--|:--|:--|
| `search_knowledge` | Search Mimir RAG knowledge base | Mimir API |
| `get_current_time` | Get current date/time with timezone | – |
| `calculate` | Evaluate mathematical expressions | – |
| `http_request` | Make HTTP GET/POST requests | URL allowlist |
| `sql_query` | Query SQLite databases (read-only) | DB path config |
| `run_python` | Execute Python code in sandbox | `CODE_EXEC_ENABLED=true` |
| `delegate_to_agent` | Hand off to another agent | Agent Router |

---

## Custom Tools

### Webhook Tool

Register an external API as a tool:

```python
# POST /v1/tools
{
    "name": "get_weather",
    "description": "Get current weather for a city",
    "type": "webhook",
    "endpoint": "https://api.weather.com/v1/current",
    "method": "GET",
    "headers": {
        "X-API-Key": "${WEATHER_API_KEY}"
    },
    "parameters": {
        "type": "object",
        "properties": {
            "city": { "type": "string" }
        },
        "required": ["city"]
    },
    "response_mapping": {
        "template": "Weather in {{city}}: {{temp}}°C, {{condition}}"
    }
}
```

### Python Function Tool

Register a local Python function:

```python
from bifrost.tools.base import BaseTool, ToolResult

class StockPriceTool(BaseTool):
    name = "get_stock_price"
    description = "Get the current stock price for a given ticker symbol"
    
    parameters = {
        "type": "object",
        "properties": {
            "ticker": {
                "type": "string",
                "description": "Stock ticker symbol (e.g., AAPL)"
            }
        },
        "required": ["ticker"]
    }
    
    async def execute(self, ticker: str) -> ToolResult:
        # Your implementation
        price = await self.fetch_price(ticker)
        return ToolResult(
            success=True,
            data={"ticker": ticker, "price": price},
            display=f"{ticker}: ${price}"
        )
```

---

## Deployment

### Local Development

```bash
# 1. Clone & setup
git clone https://github.com/megacare-dev/Bifrost.git
cd Bifrost
python -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"

# 2. Configure
cp .env.example .env
# Edit .env with your Heimdall/Mimir URLs

# 3. Run migrations
python -m bifrost.db.migrations

# 4. Start server
uvicorn bifrost.main:app --host 0.0.0.0 --port 8100 --reload
```

### Docker

```bash
docker build -t bifrost:latest .
docker run -p 8100:8100 --env-file .env bifrost:latest
```

### Docker Compose (Full Stack)

```yaml
version: "3.8"
services:
  bifrost:
    build: .
    ports:
      - "8100:8100"
    env_file: .env
    volumes:
      - ./data:/app/data
    depends_on:
      - heimdall
    
  # Heimdall and Mimir run on bare metal (Apple Silicon)
  # Configure HEIMDALL_BASE_URL and MIMIR_BASE_URL in .env
```

---

## Roadmap

### Phase 1: Foundation ✅ → 🚧
- [x] Project structure & setup
- [ ] FastAPI server with health checks
- [ ] Agent Executor (ReAct loop)
- [ ] Heimdall client (OpenAI-compatible)
- [ ] Mimir client (RAG search)
- [ ] Built-in tools: `search_knowledge`, `get_current_time`, `calculate`
- [ ] Short-term session management
- [ ] Execution tracing & logging
- [ ] Basic auth (API key)
- [ ] `/v1/agents/:id/run` endpoint
- [ ] `/v1/agents/:id/stream` SSE endpoint

### Phase 2: Advanced Tools
- [ ] Custom webhook tools
- [ ] HTTP request tool with URL allowlist
- [ ] SQL query tool (read-only)
- [ ] Sandboxed Python code execution
- [ ] Long-term memory bank
- [ ] Tool result caching

### Phase 3: Multi-Agent & Scale
- [ ] Agent-to-agent delegation
- [ ] Parallel agent execution
- [ ] Agent versioning & rollback
- [ ] Rate limiting per agent/tenant
- [ ] Prometheus metrics export
- [ ] A2A protocol compatibility

### Phase 4: Intelligence
- [ ] Automatic tool selection (based on agent capability)
- [ ] Plan-and-Execute strategy (for complex multi-step tasks)
- [ ] Self-reflection / output verification
- [ ] Learning from feedback (RLHF-style)

---

## Related Projects

| Project | Role | Repo |
|:--|:--|:--|
| **Heimdall** | LLM Gateway (Rust/Axum) | [Project-Mimir/heimdall](https://github.com/megacare-dev/Project-Mimir) |
| **Mimir** | RAG Pipeline + Agent Builder (Rust/Axum) | [Project-Mimir](https://github.com/megacare-dev/Project-Mimir) |
| **Bifrost** | Agent Runtime Engine (Python/FastAPI) | *This repo* |

---

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-tool`)
3. Commit your changes (`git commit -m 'feat: add amazing tool'`)
4. Push to the branch (`git push origin feature/amazing-tool`)
5. Open a Pull Request

---

## License

MIT License — See [LICENSE](LICENSE) for details.

---

<p align="center">
  <strong>⚡ Bifrost</strong> — Part of the <a href="https://github.com/megacare-dev/Project-Mimir">Project Mimir</a> ecosystem
  <br/>
  <em>Named after the burning rainbow bridge in Norse mythology that connects the realms of gods and humanity.</em>
</p>
