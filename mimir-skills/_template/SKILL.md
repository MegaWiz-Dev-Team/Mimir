# Skill: [SKILL_NAME]

## Goal
<!-- One sentence: what clinical question does this skill answer? -->

## Data Sources
<!-- APIs, databases, or internal services this skill queries -->
- Source: [e.g. DrugBank API, PrimeKG Neo4j, BigQuery pubmed]
- Credentials: Vault path `secret/mimir/[credential_name]`

## SOP Steps
<!-- Step-by-step execution order. Each step should be atomic and testable. -->

1. Validate input against `schema.json`
2. [Step 2]
3. [Step 3]
4. Normalize output to `ToolResult` schema
5. Return result

## Input Schema
See `schema.json` — `input` section.

Key fields:
| Field | Type | Required | Description |
|---|---|---|---|
| `query` | string | ✅ | ... |

## Output Schema
See `schema.json` — `output` section.

`ToolResult` envelope:
```json
{
  "tool_name": "skill_name",
  "status": "ok | error | partial",
  "data": { ... },
  "sources": [{ "id": "...", "type": "pmid|nct|drugbank", "url": "..." }],
  "latency_ms": 123
}
```

## Error Handling

| Error | Behavior |
|---|---|
| API timeout (>10s) | Return `status: "error"`, message: "upstream timeout" |
| No results found | Return `status: "ok"`, `data: {}`, empty sources |
| Invalid input | Return `status: "error"` with validation details |
| Rate limit (429) | Retry once after 2s, then error |

## Examples
See `examples/` directory — minimum 3 examples required before Sprint 2 ship.
