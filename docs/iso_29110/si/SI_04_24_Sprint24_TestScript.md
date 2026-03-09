# SI-04.24: Sprint 24 Test Script (Graph API Hotfix & KG Import)

**Project Name:** Project Mimir
**Sprint:** Sprint 24
**Date:** 2026-03-09
**Tester:** AI Agent (Antigravity)

---

## Test Summary

| Category                      | Total  | ✅ Pass | ❌ Fail |
| ----------------------------- | ------ | ------ | ------ |
| Graph API Integration Tests   | 5      | 5      | 0      |
| Vector Search Integration     | 2      | 2      | 0      |
| Coverage API Verification     | 1      | 1      | 0      |
| KG Data Import Validation     | 3      | 3      | 0      |
| **Total**                     | **11** | **11** | **0**  |

---

## Test Cases

### Graph API Integration Tests (I = Integration)

| ID          | Scenario                                      | Steps                                                     | Expected                        | Result  | Issue/PR | หมายเหตุ                                |
| ----------- | --------------------------------------------- | --------------------------------------------------------- | ------------------------------- | ------- | -------- | -------------------------------------- |
| TC_SP24_I01 | Graph path search OSA → CPAP                  | `GET /graph/paths?from=OSA&to=CPAP`                       | `found: true`, treats relation  | ✅ Pass | #222     | FK JOIN query rewrite works            |
| TC_SP24_I02 | Entity neighbors for CPAP                     | `GET /graph/entity/{id}/neighbors`                        | 2 edges (treats, used_for)      | ✅ Pass | #222     | from_entity_id/to_entity_id JOINs      |
| TC_SP24_I03 | Graph visualization (50 nodes)                | `GET /graph/visualization?limit=50`                       | 50 nodes, 33 edges              | ✅ Pass | #222     | FK-based visualization query works     |
| TC_SP24_I04 | Trigger extraction SQL fix                    | `POST /graph/extract` (NOW() instead of datetime('now'))  | run_id=2 returned               | ✅ Pass | #222     | MariaDB-compatible SQL syntax          |
| TC_SP24_I05 | Entity search by keyword                      | `GET /graph/entities?search=sleep`                        | 284 results                     | ✅ Pass | #222     | Text search with tenant isolation      |

### Vector Search Integration Tests (I = Integration)

| ID          | Scenario                                      | Steps                                                     | Expected                        | Result  | Issue/PR | หมายเหตุ                                |
| ----------- | --------------------------------------------- | --------------------------------------------------------- | ------------------------------- | ------- | -------- | -------------------------------------- |
| TC_SP24_I06 | Vector search via Heimdall embedding           | `POST /vector/search` with query "sleep apnea CPAP"      | HTTP 200, connects to :8001     | ✅ Pass | #224     | Heimdall bge-m3 produces 1024-dim      |
| TC_SP24_I07 | Vector search TenantContext fix                | `POST /vector/search` with X-Tenant-Id header             | No 500 (TenantContext missing)  | ✅ Pass | #222     | HeaderMap + extract_tenant_id()        |

### Coverage API Verification (I = Integration)

| ID          | Scenario                                      | Steps                                                     | Expected                        | Result  | Issue/PR | หมายเหตุ                                |
| ----------- | --------------------------------------------- | --------------------------------------------------------- | ------------------------------- | ------- | -------- | -------------------------------------- |
| TC_SP24_I08 | Coverage kg_extracted detects KG data          | `GET /coverage/overview`                                  | `kg_extracted: 5` (not 0)       | ✅ Pass | #225     | Uses sources_with_kg instead of runs   |

### KG Data Import Validation (U = Unit)

| ID          | Scenario                                      | Steps                                                     | Expected                        | Result  | Issue/PR | หมายเหตุ                                |
| ----------- | --------------------------------------------- | --------------------------------------------------------- | ------------------------------- | ------- | -------- | -------------------------------------- |
| TC_SP24_U01 | Bulk entity import (1,341 unique)              | `POST /graph/entities/bulk` × 5 sources                   | 1,341 inserted, 0 skipped       | ✅ Pass | #223     | After dedup from 2,682                 |
| TC_SP24_U02 | Bulk relation import (685 with FK lookup)      | `POST /graph/relations/bulk` × 5 sources                  | 685 inserted, 8 skipped (orphan)| ✅ Pass | #222     | Entity name→ID lookup before INSERT    |
| TC_SP24_U03 | Vault secrets verification (7/7)               | `GET /vault/secrets`                                      | 7 secrets including GEMINI_API_KEY | ✅ Pass | —        | Vault + .env fallback chain works      |
