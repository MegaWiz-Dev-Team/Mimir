# SI-04.17: Sprint 17 Test Script (Knowledge Graph & GraphRAG)

**Project Name:** Project Mimir
**Sprint:** Sprint 17
**Tester:** AI Assistant
**Date:** 2026-03-04
**Status:** ✅ All Tests Passed

---

## 1. Unit Tests — Backend

### 1.1 Build Verification
| ID         | Scenario                 | Steps                                             | Expected                   | Result | Issue/PR | หมายเหตุ                                 |
| ---------- | ------------------------ | ------------------------------------------------- | -------------------------- | ------ | -------- | --------------------------------------- |
| TC_SP17_U1 | cargo check — 0 errors   | 1. รัน `cargo check 2>&1 \| grep -cE "^error"`     | Output: 0                  | ✅ Pass | PR #187  | Clean build                             |
| TC_SP17_U2 | mimir-core-ai full tests | 1. รัน `cargo test --lib -p mimir-core-ai`         | 27 tests pass, exit code 0 | ✅ Pass | PR #187  | Includes neo4j + entity_extractor tests |
| TC_SP17_U3 | ro-ai-bridge graph tests | 1. รัน `cargo test --lib -p ro-ai-bridge -- graph` | 5 tests pass, exit code 0  | ✅ Pass | PR #187  | route assembly + deserialization tests  |

### 1.2 Neo4j Service Tests (14 tests)
| ID          | Scenario                  | Steps                                                 | Expected                                     | Result | Issue/PR | หมายเหตุ                        |
| ----------- | ------------------------- | ----------------------------------------------------- | -------------------------------------------- | ------ | -------- | ------------------------------ |
| TC_SP17_N1  | Config จาก environment    | 1. ตั้ง NEO4J_URI/USER/PASSWORD<br>2. สร้าง Neo4jConfig  | Config อ่านค่าจาก env var ถูกต้อง                | ✅ Pass | PR #187  | URI, user, password match      |
| TC_SP17_N2  | Config defaults           | 1. ไม่ตั้ง env vars<br>2. สร้าง Neo4jConfig               | Default: bolt://localhost:7687, neo4j, neo4j | ✅ Pass | PR #187  | Requires unsafe for remove_var |
| TC_SP17_N3  | Cypher MERGE node         | 1. build_upsert_entity_cypher()                       | MERGE query มี tenant_id parameter            | ✅ Pass | PR #187  | Tenant isolation verified      |
| TC_SP17_N4  | Cypher MERGE relation     | 1. build_upsert_relation_cypher()                     | MERGE query เชื่อม 2 nodes ด้วย relation type   | ✅ Pass | PR #187  |                                |
| TC_SP17_N5  | Cypher match nodes        | 1. build_match_nodes_cypher("test")                   | CONTAINS query มี tenant_id + LIMIT           | ✅ Pass | PR #187  |                                |
| TC_SP17_N6  | Cypher neighbors          | 1. build_neighbors_cypher()                           | Variable-length path query [:*1..depth]      | ✅ Pass | PR #187  |                                |
| TC_SP17_N7  | Cypher path               | 1. build_path_cypher()                                | shortestPath query with max depth            | ✅ Pass | PR #187  |                                |
| TC_SP17_N8  | Stats query               | 1. build_stats_cypher()                               | COUNT query per entity type                  | ✅ Pass | PR #187  |                                |
| TC_SP17_N9  | Delete by source          | 1. build_delete_by_source_cypher()                    | DETACH DELETE with source_id filter          | ✅ Pass | PR #187  |                                |
| TC_SP17_N10 | Tenant isolation          | 1. สร้าง query สำหรับ tenant "A"<br>2. ตรวจ query string | ทุก query มี WHERE tenant_id = $tenant_id      | ✅ Pass | PR #187  |                                |
| TC_SP17_N11 | Search with type filter   | 1. build_match_nodes_cypher + type                    | Query มี AND n.entity_type                    | ✅ Pass | PR #187  |                                |
| TC_SP17_N12 | Visualization query       | 1. build_visualization_cypher()                       | MATCH nodes + relationships for graph viz    | ✅ Pass | PR #187  |                                |
| TC_SP17_N13 | Depth parameter neighbors | 1. Depth = 3                                          | Path length = [:*1..3]                       | ✅ Pass | PR #187  |                                |
| TC_SP17_N14 | Default depth path        | 1. No depth specified                                 | Default max_depth = 5                        | ✅ Pass | PR #187  |                                |

### 1.3 Entity Extractor Tests (12 tests)
| ID          | Scenario                        | Steps                                               | Expected                                             | Result | Issue/PR | หมายเหตุ |
| ----------- | ------------------------------- | --------------------------------------------------- | ---------------------------------------------------- | ------ | -------- | ------- |
| TC_SP17_E1  | Prompt มี text ที่ส่งมา             | 1. build_extraction_prompt("test text")             | Prompt contains "test text"                          | ✅ Pass | PR #187  |         |
| TC_SP17_E2  | Prompt มี JSON schema            | 1. build_extraction_prompt()                        | Prompt contains "entities" + "relations" JSON schema | ✅ Pass | PR #187  |         |
| TC_SP17_E3  | Parse valid JSON                | 1. parse_extraction_response(valid_json)            | Parsed entities + relations ถูกต้อง                    | ✅ Pass | PR #187  |         |
| TC_SP17_E4  | Parse empty JSON                | 1. parse_extraction_response("{}")                  | Empty vectors, no error                              | ✅ Pass | PR #187  |         |
| TC_SP17_E5  | Parse JSON in code block        | 1. parse_extraction_response("```json\n{...}\n```") | Extracted JSON parsed correctly                      | ✅ Pass | PR #187  |         |
| TC_SP17_E6  | Parse invalid JSON              | 1. parse_extraction_response("not json")            | Returns empty result, no panic                       | ✅ Pass | PR #187  |         |
| TC_SP17_E7  | Dedup entities (same name+type) | 1. dedup 2 entities same name+type                  | 1 entity returned                                    | ✅ Pass | PR #187  |         |
| TC_SP17_E8  | Dedup keeps first occurrence    | 1. dedup entities                                   | Properties from first entity preserved               | ✅ Pass | PR #187  |         |
| TC_SP17_E9  | No dedup different types        | 1. dedup entities different types                   | Both entities kept                                   | ✅ Pass | PR #187  |         |
| TC_SP17_E10 | Case-insensitive dedup          | 1. dedup "Aspirin" + "aspirin" same type            | 1 entity returned                                    | ✅ Pass | PR #187  |         |
| TC_SP17_E11 | ExtractionResult creation       | 1. Create ExtractionResult struct                   | Fields populated correctly                           | ✅ Pass | PR #187  |         |
| TC_SP17_E12 | Entity type enum                | 1. Check all EntityType variants                    | All types present (Person, Org, Location, etc.)      | ✅ Pass | PR #187  |         |

### 1.4 Graph Route Tests (5 tests)
| ID         | Scenario                       | Steps                       | Expected                               | Result | Issue/PR | หมายเหตุ |
| ---------- | ------------------------------ | --------------------------- | -------------------------------------- | ------ | -------- | ------- |
| TC_SP17_R1 | Routes assembly                | 1. graph_routes() → Router  | Router created without panic           | ✅ Pass | PR #187  |         |
| TC_SP17_R2 | EntitySearchQuery defaults     | 1. Deserialize "{}"         | q=None, type=None, limit=20, page=1    | ✅ Pass | PR #187  |         |
| TC_SP17_R3 | ExtractRequest defaults        | 1. Deserialize "{}"         | source_ids=[]                          | ✅ Pass | PR #187  |         |
| TC_SP17_R4 | VisualizationQuery deserialize | 1. Deserialize "{}"         | limit=100, type=None                   | ✅ Pass | PR #187  |         |
| TC_SP17_R5 | PathQuery required fields      | 1. Deserialize with from/to | from + to populated, max_depth default | ✅ Pass | PR #187  |         |

---

## 2. Frontend Tests

### 2.1 Feature Verification
| ID         | Scenario                | Steps                                 | Expected                                                    | Result | Issue/PR | หมายเหตุ                   |
| ---------- | ----------------------- | ------------------------------------- | ----------------------------------------------------------- | ------ | -------- | ------------------------- |
| TC_SP17_F1 | Graph page renders      | 1. Navigate to /graph                 | Page shows with KPI cards, search, filters, canvas          | ✅ Pass | PR #187  | shadcn/ui Card components |
| TC_SP17_F2 | Graph navigation link   | 1. Check navbar                       | "Graph" link with Share2 icon present                       | ✅ Pass | PR #187  |                           |
| TC_SP17_F3 | KG Settings tab         | 1. Go to Settings → Knowledge Graph   | Shows active status + link to Graph page (not Coming Soon)  | ✅ Pass | PR #187  |                           |
| TC_SP17_F4 | Graph theme consistency | 1. Compare /graph with /analytics/llm | Same theme (bg-background, shadcn Cards, no hardcoded dark) | ✅ Pass | PR #187  | Fixed theme mismatch      |

**Grand Total: 35/35 (100%)**

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด SI-04)*
