# SI-04 — Sprint: Tenant Multi-Agent APIs — Test Script & Results

| Field | Value |
|-------|-------|
| **Document ID** | SI-04-Tenant-API |
| **Version** | 1.0 |
| **Date** | 2026-03-18 |
| **Project** | Mimir — Knowledge Management Platform |
| **Module** | Tenant CRUD + Document Ingestion + Tenant Query APIs |
| **Tester** | Fenrir E2E Test Suite (automated) |
| **Environment** | macOS, Mimir v0.29.0 @ localhost:3002, MariaDB, Fenrir :8200 |

---

## 1. Test Scope

Verify the new multi-tenant APIs enabling Self-Aware Agents to store and query data per service:

- **Tenant CRUD** — Create, Read, List, Delete tenants
- **Document Ingestion** — Ingest markdown docs with PageIndex tree indexing
- **Tenant Query** — RAG-based querying scoped to tenant documents
- **Bug Fix** — Domain column VARCHAR(20) → VARCHAR(255) regression (#251)

---

## 2. TDD Unit/Integration Tests (15 tests)

| # | Test | Expected | Actual | Status |
|:-:|------|----------|--------|:------:|
| T01 | `GET /health` → 200 | `{"status":"ok"}` | `{"status":"ok"}` | ✅ |
| T02 | `GET /healthz` → 200 | K8s probe alias | 200 ok | ✅ |
| T10 | List tenants | `default_tenant` exists | Found | ✅ |
| T11 | Create `test_tdd_agent` | 201 Created | 201 + domain | ✅ |
| T12 | Create long-domain `heimdall_llm_gateway` | 201 (#251 fix) | 201 | ✅ |
| T13 | Get `test_tdd_agent` | service_type returned | Matches | ✅ |
| T14 | Get nonexistent → 404 | 404 | 404 | ✅ |
| T15 | Create duplicate → 409 | 409 Conflict | 409 | ✅ |
| T20 | Ingest markdown doc | tree_node_count ≥ 1 | 5 nodes | ✅ |
| T21 | List documents | title = "TDD Agent README" | Matches | ✅ |
| T30 | Query tenant | answer + sources | Returned | ✅ |
| T31 | Query nonexistent → 404 | 404 | 404 | ✅ |
| T90 | Delete tenant | 200, `{"deleted":"..."}` | Matches | ✅ |
| T91 | Delete long-domain tenant | 200 | 200 | ✅ |
| T92 | Delete nonexistent → 404 | 404 | 404 | ✅ |

**Result: 15/15 PASSED** — Duration: 1.81s

---

## 3. E2E Tests — Full API Suite (21 tests)

| # | Endpoint | Method | Expected | Actual | Status |
|:-:|----------|--------|----------|--------|:------:|
| 1 | `/health` | GET | 200 | 200 | ✅ |
| 2 | `/healthz` | GET | 200 | 200 | ✅ |
| 3 | `/api/v1/tenants` | GET | 200, list | 200 | ✅ |
| 4 | `/api/v1/tenants` | POST (bifrost) | 201 | 201 | ✅ |
| 5 | `/api/v1/tenants` | POST (heimdall) | 201 | 201 | ✅ |
| 6 | `/api/v1/tenants/bifrost` | GET | 200 | 200 | ✅ |
| 7 | `/api/v1/tenants/heimdall` | GET | 200 | 200 | ✅ |
| 8 | `/api/v1/tenants/nonexistent` | GET | 404 | 404 | ✅ |
| 9 | `/api/v1/tenants` | POST (dup) | 409 | 409 | ✅ |
| 10 | `/api/v1/tenants/bifrost/ingest` | POST | 200 | 200 | ✅ |
| 11 | `/api/v1/tenants/heimdall/ingest` | POST | 200 | 200 | ✅ |
| 12 | `/api/v1/tenants/bifrost/ingest/documents` | GET | 200 | 200 | ✅ |
| 13 | `/api/v1/tenants/heimdall/ingest/documents` | GET | 200 | 200 | ✅ |
| 14 | `/api/v1/tenants/bifrost/query` | POST | 200 | 200 | ✅ |
| 15 | `/api/v1/tenants/heimdall/query` | POST | 200 | 200 | ✅ |
| 16 | `/api/v1/tenants/muninn/query` | POST | 200 | 200 | ✅ |
| 17 | `/api/sources` | GET | 200 | 404 | ❌ |
| 18 | `/api/stats` | GET | 200 | 404 | ❌ |
| 19 | `/api/prompts` | GET | 200 | 404 | ❌ |
| 20 | `/api/v1/tenants/bifrost` | DELETE | 200 | 200 | ✅ |
| 21 | `/api/v1/tenants/heimdall` | DELETE | 200 | 200 | ✅ |

**Result: 18/21 PASSED (86%)** — Duration: 6.5s

> Tests #17-19 fail because these endpoints require `X-Tenant-Id` header and use `/api/v1/` prefix (pre-existing core APIs, not new code).

---

## 4. Bug Fix Verification

| Issue | Fix | Verified |
|-------|-----|:--------:|
| #251 — `domain VARCHAR(20)` | `ALTER TABLE MODIFY domain VARCHAR(255)` | ✅ T12 passes |
| #252 — `/healthz` alias | `.route("/healthz", get(health_check))` | ✅ T02 passes |

---

## 5. Fenrir Test Dashboard

All results automatically submitted to Fenrir Test Results API:

| Fenrir ID | Type | Suite | Result |
|:---------:|------|-------|:------:|
| #6 | Unit | TDD tenant_api_tests | ✅ 15/15 |
| #7 | E2E | mimir-e2e-full | ⚠️ 18/21 |

---

## 6. Files Modified

| File | Change |
|------|--------|
| `routes/tenant.rs` | Domain fix: `MODIFY domain VARCHAR(255)` |
| `main.rs` | Added `/healthz` route alias |
| `Cargo.toml` | Added `blocking` feature to reqwest |
| `tests/tenant_api_tests.rs` | **NEW** — 15 TDD integration tests |

---

## 7. Conclusion

| Metric | Value |
|--------|-------|
| TDD Tests | **15/15 PASSED** |
| E2E Tests | **18/21 PASSED (86%)** |
| Bug Fixes | **2/2 VERIFIED** |
| New APIs | **3 modules verified** (tenant, ingest, query) |
| Multi-Tenant Ready | **YES** — tenants create/query/delete correctly |

**Sign-off**: Mimir Tenant API module is ready for deployment and multi-agent integration.
