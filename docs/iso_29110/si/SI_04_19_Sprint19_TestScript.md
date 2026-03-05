# SI-04.19: Sprint 19 Test Script (Playground → Agent Templates)

**Project Name:** Project Mimir
**Sprint:** Sprint 19
**Tester:** AI Assistant
**Date:** 2026-03-05
**Status:** ✅ All Tests Passed

---

## 1. Unit Tests — Backend

### 1.1 Agent Module Tests (10 tests)
| ID          | Scenario                       | Steps                                                                    | Expected                         | Result | Issue/PR | หมายเหตุ              |
| ----------- | ------------------------------ | ------------------------------------------------------------------------ | -------------------------------- | ------ | -------- | -------------------- |
| TC_SP19_U1  | Templates are valid            | 1. รัน `cargo test --lib routes::agents::tests::test_templates_are_valid` | 4 templates, correct IDs         | ✅ Pass | #193     | npc_game_agent, etc. |
| TC_SP19_U2  | Templates have required fields | 1. รัน `test_templates_have_required_fields`                              | All fields populated             | ✅ Pass | #193     |                      |
| TC_SP19_U3  | Template tiers                 | 1. รัน `test_template_tiers`                                              | All 4 = Tier 2                   | ✅ Pass | #193     |                      |
| TC_SP19_U4  | All templates use RAG          | 1. รัน `test_all_templates_use_rag`                                       | use_rag = true for all           | ✅ Pass | #193     |                      |
| TC_SP19_U5  | NPC has action tools           | 1. รัน `test_npc_agent_has_action_tools`                                  | heal, buff, warp, QueryMobDb     | ✅ Pass | #193     |                      |
| TC_SP19_U6  | Medical Doctor config          | 1. รัน `test_medical_doctor_config`                                       | medgemma, KG, temp ≤ 0.4         | ✅ Pass | #193     |                      |
| TC_SP19_U7  | NPC use Heimdall               | 1. รัน `test_npc_templates_use_heimdall`                                  | provider = "heimdall"            | ✅ Pass | #193     |                      |
| TC_SP19_U8  | API key format                 | 1. รัน `test_api_key_format`                                              | starts with "ak_", len=35        | ✅ Pass | #193     |                      |
| TC_SP19_U9  | AgentConfig new fields         | 1. รัน `test_agent_config_has_new_fields`                                 | tier=Some(1), response_mode=Some | ✅ Pass | #193     |                      |
| TC_SP19_U10 | CreateRequest defaults         | 1. รัน `test_create_request_defaults`                                     | tier=None, response_mode=None    | ✅ Pass | #193     |                      |

**Command:** `cd ro-ai-bridge && cargo test --lib routes::agents::tests -- --nocapture`

---

## 2. Frontend Tests

### 2.1 Build Verification
| ID         | Scenario         | Steps                                     | Expected                                          | Result | Issue/PR | หมายเหตุ     |
| ---------- | ---------------- | ----------------------------------------- | ------------------------------------------------- | ------ | -------- | ----------- |
| TC_SP19_F1 | npm build passes | 1. `cd ro-ai-dashboard && npx next build` | ✓ Compiled, /playground route listed, exit code 0 | ✅ Pass | #193     | Static page |

### 2.2 PERSONAS Removal
| ID         | Scenario                     | Steps                                                  | Expected       | Result | Issue/PR | หมายเหตุ           |
| ---------- | ---------------------------- | ------------------------------------------------------ | -------------- | ------ | -------- | ----------------- |
| TC_SP19_F2 | PERSONAS removed from api.ts | 1. `grep -c "PERSONAS" ro-ai-dashboard/src/lib/api.ts` | 0 occurrences  | ✅ Pass | #193     | Hardcoded removed |
| TC_SP19_F3 | fetchPlaygroundAgents exists | 1. `grep -c "fetchPlaygroundAgents" api.ts`            | ≥ 1 occurrence | ✅ Pass | #193     | DB-backed         |

### 2.3 Playground Agent-First Flow
| ID         | Scenario                      | Steps                                     | Expected                                           | Result | Issue/PR | หมายเหตุ         |
| ---------- | ----------------------------- | ----------------------------------------- | -------------------------------------------------- | ------ | -------- | --------------- |
| TC_SP19_I1 | Agent Studio → Playground btn | 1. Open /agents 2. Check action bar       | "Playground" button visible with ExternalLink icon | ✅ Pass | #193     | Deep-link       |
| TC_SP19_I2 | Tier badge on agent cards     | 1. Open /agents 2. Check card headers     | T1/T2 badge next to Live/Draft badge               | ✅ Pass | #193     | Tier display    |
| TC_SP19_I3 | Deep-link ?agent=mimir        | 1. Open /playground?agent=mimir           | Mimir auto-selected as persona                     | ✅ Pass | #193     | useSearchParams |
| TC_SP19_I4 | Empty state no agents         | 1. Open /playground with empty DB         | "No Agents Available" card with link to Studio     | ✅ Pass | #193     | Empty state     |
| TC_SP19_I5 | Suspense boundary             | 1. npx next build (useSearchParams check) | Build passes without CSR bailout error             | ✅ Pass | #193     | Next.js 16      |

---

## 3. Summary

| Category           | Total  | Pass   | Fail  |
| ------------------ | ------ | ------ | ----- |
| Backend Unit Tests | 10     | 10     | 0     |
| Frontend Build     | 1      | 1      | 0     |
| Frontend Features  | 2      | 2      | 0     |
| Integration        | 5      | 5      | 0     |
| **Total**          | **18** | **18** | **0** |
