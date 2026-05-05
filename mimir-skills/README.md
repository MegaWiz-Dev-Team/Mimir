# Mimir Skills Registry

Each directory here is one **medical skill** available to the Hermodr MCP Server.

## Convention

```
mimir-skills/
  <skill-name>/
    SKILL.md      — Goal, SOP, Input/Output, Error handling (required)
    schema.json   — JSON Schema draft-07 for input + ToolResult output (required)
    examples/
      example_01.json   — real input → expected output (min 3 before ship)
      example_02.json
      example_03.json
```

## ToolResult Envelope (all skills must return this)

```json
{
  "tool_name": "drug_drug_interaction",
  "status": "ok",
  "data": { ... },
  "sources": [{ "id": "DB00331", "type": "drugbank" }],
  "latency_ms": 85
}
```

`status` values: `ok` | `partial` | `error`

## Adding a New Skill

1. Copy `_template/` to `<new-skill-name>/`
2. Fill in `SKILL.md` completely (all sections required)
3. Update `schema.json` with skill-specific `data` payload shape
4. Add 3+ real examples in `examples/`
5. Register in Hermodr MCP manifest (Sprint 2)

## Sprint Status

| Skill | Sprint | Status |
|---|---|---|
| `pubmed-search` | S2 | pending |
| `drug-drug-interaction` | S2 | pending |
| `clinical-trial-matching` | S2 | pending |
| `differential-diagnosis` | S2 | pending |
| `cpic-pharmacogenomics` | S2 | pending |
