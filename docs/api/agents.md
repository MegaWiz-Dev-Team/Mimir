# Mimir API — Agent Capabilities (read endpoints)

Read-only endpoints for inspecting agent configuration. Companion to the existing admin CRUD that Mimir Studio uses.

- `GET /api/v1/agents` — list agents for the calling tenant
- `GET /api/v1/agents/{agent_id_or_name}` — detail view

Sibling doc for Bifrost: [Bifrost/docs/api/agents.md](../../../Bifrost/docs/api/agents.md). Both services expose the same response shape; choose whichever fits your client's role:
- **Bifrost** = agent runtime (call `/v1/agents/{id}/run` after discovering capabilities)
- **Mimir** = CRUD layer (authoritative `agent_configs` writes happen here)

> [!IMPORTANT]
> These endpoints expose **persona IP** (`system_prompt`, `personality_traits`, `greeting`) and **attack-surface inventory** (`tools`, `mcp_servers`). Treat them like authenticated admin reads, not a public directory.

## Authentication

Both endpoints require a tenant context resolved by one of:

1. **JWT** — `Authorization: Bearer <token>`. Two algorithms accepted:
   - **RS256** — Yggdrasil Zitadel-issued JWT, tenant from `urn:zitadel:iam:org:id` claim. Requires `YGGDRASIL_ISSUER` env at startup.
   - **HS256** — legacy internal JWT issued by `IamService::generate_jwt` (Mimir Studio login). Tenant from `tenant_id` claim.
2. **`X-Tenant-Id` header** (fallback). Used when no JWT is presented OR the JWT failed validation. Each fallback request emits a `WARN`-level log so operators can spot header-only callers.

Neither path supplied → **`401 Unauthorized`**.

JWT wins over header when both are present (header silently ignored).

### Rate limit
60 requests per minute per source IP (`X-Forwarded-For` → `X-Real-IP` → `Forwarded` → peer IP). Excess returns **`429 Too Many Requests`**.

### Audit
Each successful detail call emits a structured `tracing` event:
```
event=agent.detail.read tenant_id=<...> agent_id=<...> agent_name=<...>
auth_mode=jwt|header_fallback|legacy_header
```
The event flows to Tyr via the existing log/OTLP pipeline. List endpoint reads are not audited (too noisy).

### Scope of the public-read layer
Only `GET /api/v1/agents` and `GET /api/v1/agents/{id_or_name}` are mounted on the JWT + rate-limit + audit stack. The admin endpoints (`POST`/`PUT`/`DELETE` `/api/v1/agents`, `/publish`, `/chat`, `/conversations`, `/generate`, `/route`) keep the legacy `X-Tenant-Id`-trust contract because they're called from Mimir Studio's authenticated session — they were intentionally left out of scope for this change. Extending those requires a separate refactor.

---

## `GET /api/v1/agents`

### Query parameters
| Name | Type | Default | Description |
|------|------|---------|-------------|
| `page` | `int` | `1` | 1-indexed pagination |
| `per_page` | `int` | `20` (max `100`) | page size |

### Response — `200 OK`
```json
{
  "tenant_id": "asgard_medical",
  "agents": [
    {
      "id": 1,
      "name": "eir-cardio",
      "display_name": "Eir Cardiology",
      "description": "Specialty agent for cardiology questions",
      "avatar_url": "https://...",
      "is_published": true,
      "model_id": "gemma-4-26b",
      "capabilities": {
        "model_id": "gemma-4-26b",
        "provider": "mlx",
        "temperature": 0.7,
        "max_tokens": 2048,
        "top_k": 5,
        "use_rag": true,
        "use_knowledge_graph": false,
        "use_pageindex": false,
        "tools": ["vector_search", "ocr_extract"],
        "mcp_servers": []
      }
    }
  ],
  "total": 12,
  "page": 1,
  "per_page": 20
}
```

### Field rules
- Top-level fields (`id`, `name`, `display_name`, `description`, `avatar_url`, `is_published`, `model_id`) are preserved for backwards compatibility with existing Mimir Studio code.
- `capabilities.tools` and `capabilities.mcp_servers` are always arrays (NULL DB columns serialize as `[]`).
- **Never present** on the list response: `system_prompt`, `personality_traits`, `greeting`, `rag_params`, `api_key`, `template_id`.

---

## `GET /api/v1/agents/{agent_id_or_name}`

### Path resolution
- If `{agent_id_or_name}` parses as `i64` → looked up by `agent_configs.id`.
- Otherwise → looked up by `agent_configs.name`.
- Both branches are filtered on the caller's tenant.

> **Caveat**: an agent whose `name` is a string of digits (e.g. `"42"`) is unreachable by name — the path always resolves to ID first. Conventional names (`eir`, `eir-cardio`, …) are unaffected.

### Response — `200 OK`
```json
{
  "id": 1,
  "name": "eir-cardio",
  "display_name": "Eir Cardiology",
  "description": "...",
  "avatar_url": "https://...",
  "greeting": "Hello, I'm Eir's cardiology specialist.",
  "is_published": true,
  "model_id": "gemma-4-26b",
  "system_prompt": "You are a cardiology specialist...",
  "personality_traits": ["warm", "precise", "evidence-based"],
  "created_at": "2026-05-10T12:34:56+00:00",
  "updated_at": "2026-05-19T08:21:00+00:00",
  "capabilities": {
    "model_id": "gemma-4-26b",
    "provider": "mlx",
    "temperature": 0.7,
    "max_tokens": 2048,
    "top_k": 5,
    "use_rag": true,
    "use_knowledge_graph": true,
    "use_pageindex": false,
    "tools": ["vector_search", "graph_search", "primekg_search"],
    "mcp_servers": ["hermodr-mimir"]
  },
  "rag_params": {
    "limit": 10,
    "alpha": 0.7,
    "output_format": "json"
  }
}
```

### Detail-only fields
- `system_prompt`, `personality_traits`, `greeting`, `created_at`, `updated_at`.
- `rag_params` is a **whitelisted projection** — only `limit`, `alpha`, `output_format` are returned. Any other keys stashed in the DB column (operator notes, legacy tuning, future config) are dropped before serialization.

### Never returned
- `api_key` — hard-excluded via `#[serde(skip_serializing)]` on the struct field. To rotate, use the dedicated `POST /api/v1/agents/{id}/publish` admin endpoint.
- `template_id` — internal implementation detail.

---

## Error responses

| Status | Body | When |
|--------|------|------|
| `401 Unauthorized` | (no body) | No JWT and no `X-Tenant-Id`; or JWT failed validation AND no header fallback |
| `404 Not Found` | `{"error":"agent_not_found"}` | Agent does not exist for this tenant. **Same body and same code path** whether the agent is missing entirely or belongs to another tenant — no cross-tenant existence oracle |
| `429 Too Many Requests` | (governor default body) | Rate limit exceeded for the source IP |
| `500 Internal Server Error` | `{"error":"internal_error"}` | DB connection failure or unexpected error. Server logs the real cause; the response never echoes it |

---

## Examples

### List agents (legacy header path)
```bash
curl -H 'X-Tenant-Id: asgard_medical' \
     http://localhost:3002/api/v1/agents | jq
```

### List with JWT (Yggdrasil RS256)
```bash
curl -H "Authorization: Bearer $YGGDRASIL_JWT" \
     'http://localhost:3002/api/v1/agents?page=1&per_page=50' | jq
```

### Detail by ID
```bash
curl -H "Authorization: Bearer $JWT" \
     http://localhost:3002/api/v1/agents/1 | jq
```

### Detail by name
```bash
curl -H "Authorization: Bearer $JWT" \
     http://localhost:3002/api/v1/agents/eir-cardio | jq
```

### Cross-tenant probe (must 404, never leak)
```bash
curl -i -H 'X-Tenant-Id: nonexistent' \
     http://localhost:3002/api/v1/agents/eir-cardio
# HTTP/1.1 404 Not Found
# {"error":"agent_not_found"}
```

### Field-leak grep (run after any handler change)
```bash
curl -s -H 'X-Tenant-Id: asgard_medical' \
     http://localhost:3002/api/v1/agents | \
  grep -E 'api_key|system_prompt|hunter2|rag_params'
# expect no output
```

---

## Implementation reference
- Handlers (list/detail): [src/routes/agents/crud.rs](../../ro-ai-bridge/src/routes/agents/crud.rs)
- Router split (public read + admin merge): [src/routes/agents/mod.rs](../../ro-ai-bridge/src/routes/agents/mod.rs)
- Flexible auth middleware: [mimir-core-ai/src/middleware/flexible_tenant.rs](../../ro-ai-bridge/mimir-core-ai/src/middleware/flexible_tenant.rs)
- JWT validator (Yggdrasil RS256): [mimir-core-ai/src/services/iam_jwt.rs](../../ro-ai-bridge/mimir-core-ai/src/services/iam_jwt.rs)
- Integration tests (11 tests, all passing): [tests/agents_endpoint.rs](../../ro-ai-bridge/tests/agents_endpoint.rs)
