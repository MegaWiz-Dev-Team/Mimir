# Insurance agent tool seed — pre-staged for Sprint 52

**Why this file exists.** The insurance sprint plan (`Mimir/docs/03_implementation_plans/03_16_Asgard_Insurance_Sprint_Plan.md`) calls out a "B-50g+" item: when we add `ocr_extract` to Eir variants, also stage the equivalent JSON snippet for the future `asgard_insurance` agents so Sprint 52 (INS-03) doesn't re-discover the spec.

Not applied yet. **`sprint52_insurance_agents.sql` (already drafted, not run) is the canonical seed; this file is only for documentation continuity.**

## Tools the 5 insurance agents need (from sprint plan §6.2)

| Agent | Tools |
|-------|-------|
| `insurance-router` | `[]` (router only classifies; no tools needed) |
| `insurance-generic` | `["ocr_extract", "icd10_lookup"]` (catch-all) |
| `insurance-medical-review` | `["ocr_extract", "icd10_lookup", "policy_coverage_lookup", "claim_history_search"]` |
| `insurance-coverage-lookup` | `["policy_coverage_lookup"]` |
| `insurance-pre-auth` | `["ocr_extract", "icd10_lookup", "policy_coverage_lookup", "claim_history_search"]` |

## SQL pattern (mirrors `sprint50_eir_ocr_allowlist.sql`)

```sql
-- Run AFTER sprint52_insurance_agents.sql has seeded the agents.
-- Idempotent — JSON_CONTAINS guard.
UPDATE agent_configs
   SET tools = JSON_ARRAY_APPEND(COALESCE(tools, JSON_ARRAY()), '$', 'ocr_extract'),
       updated_at = NOW()
 WHERE tenant_id = 'asgard_insurance'
   AND name IN ('insurance-generic', 'insurance-medical-review', 'insurance-pre-auth')
   AND (tools IS NULL OR NOT JSON_CONTAINS(tools, '"ocr_extract"'));
```

For new tools (`policy_coverage_lookup`, `claim_history_search`), same JSON_ARRAY_APPEND pattern. Define the tool in `Hermodr/src/services/mimir.rs` (per sprint plan INS-04/INS-05) BEFORE running the SQL — otherwise agents get an allowlist for a non-existent tool, which is harmless but weird in audit.

## Cross-references

- Insurance sprint plan: `Mimir/docs/03_implementation_plans/03_16_Asgard_Insurance_Sprint_Plan.md`
- Cross-tenant gateway: `Mimir/docs/03_implementation_plans/03_15_Cross_Tenant_A2A_Gateway.md`
- Insurance agent seed (drafted, not applied): `Mimir/ro-ai-bridge/migrations/sprint52_insurance_agents.sql`
- This pattern's predecessor: `Mimir/ro-ai-bridge/migrations/sprint50_eir_ocr_allowlist.sql`
