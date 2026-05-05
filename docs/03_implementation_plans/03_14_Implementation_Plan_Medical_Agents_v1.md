# 🏥 Medical Agentic Ecosystem + KG Migration — Master Implementation Plan

## 🎯 Overview

เอกสารฉบับนี้รวม 2 แผนงานที่มีความสัมพันธ์กันไว้ในที่เดียว:

- **Track A — KG Migration** (MariaDB → Neo4j): prerequisite สำหรับ PrimeKG import, ใช้เวลา ~1 สัปดาห์
- **Track B — Medical Agentic Ecosystem**: ยกระดับ Mimir จาก RAG tool → Medical Agent, 4 sprints ~3 เดือน

**อ้างอิงหลัก:** `medical_agents_strategy_20260430.md` — PrimeKG, TxAgent, Medea, MedOpenClaw, ClinicalAgents (Dual-Memory)

**ข้อกำหนดสำคัญ:** ตั้งแต่ Track B Sprint 1 เป็นต้นไป **ห้าม add table ใหม่ใน MariaDB** ทุก schema ใหม่ต้อง design เป็น PostgreSQL หรือ Neo4j เท่านั้น

---

## 🗺️ Sprint Roadmap

```
พฤษภาคม 2026                  มิถุนายน 2026   กรกฎาคม 2026   Q3 2026
──────────────────────────     ─────────────   ─────────────   ─────────────
Track A: KG Migration
  KG-S1  KG-S2  KG-S3
  (1d)   (4d)   (2d)
  ──────────────────▶ done

Track B: Medical Agents
  B-S1                         B-S2            B-S3            B-S4
  Schema & Memory               Hermodr MCP     Researcher      DB Migration
  + PrimeKG ETL                 + PubMed        Mode + ToolRAG  (MariaDB→PG)
```

Track A ต้องเสร็จ **KG-S2** ก่อนเริ่ม B-S1 PrimeKG import (Neo4j ต้องมี indexes พร้อม)

---

---

# Track A: KG Migration — MariaDB → Neo4j

**เป้าหมาย:** ย้าย Knowledge Graph (tenant KG) จาก MariaDB ไปยัง Neo4j เป็น single source of truth แล้ว drop `kg_entities` / `kg_relations` ออกจาก MariaDB

**สถานะก่อน migration:**
```
Write → MariaDB (primary) + Neo4j (dual-write)
Read  → MariaDB (SqlGraphRetriever)
```

**สถานะหลัง migration:**
```
Write → Neo4j เท่านั้น
Read  → Neo4j เท่านั้น (Neo4jGraphRetriever)
MariaDB → drop kg_entities, kg_relations
```

---

## KG-S1: Validate & Index
**เวลา:** ~1 วัน

### งาน KG-1.1 — Data Parity Check

```sql
-- MariaDB
SELECT tenant_id, COUNT(*) FROM kg_entities GROUP BY tenant_id;
SELECT tenant_id, COUNT(*) FROM kg_relations GROUP BY tenant_id;
```

```cypher
// Neo4j
MATCH (n:Entity) RETURN n.tenant_id, count(n);
MATCH ()-[r:RELATES_TO]->() RETURN r.tenant_id, count(r);
```

เกณฑ์: count ตรงกัน ≥ 95% → ผ่าน, น้อยกว่า → re-sync ก่อน

### งาน KG-1.2 — สร้าง Neo4j Indexes & Constraints

```cypher
CREATE CONSTRAINT entity_unique IF NOT EXISTS
  FOR (n:Entity) REQUIRE (n.name, n.entity_type, n.tenant_id) IS UNIQUE;

CREATE FULLTEXT INDEX entity_name_ft IF NOT EXISTS
  FOR (n:Entity) ON EACH [n.name, n.entity_type];

CREATE INDEX entity_tenant IF NOT EXISTS FOR (n:Entity) ON (n.tenant_id);
CREATE INDEX entity_source IF NOT EXISTS FOR (n:Entity) ON (n.source_id);
CREATE INDEX rel_tenant IF NOT EXISTS FOR ()-[r:RELATES_TO]-() ON (r.tenant_id);
```

### งาน KG-1.3 — Benchmark Latency

| Query | Target |
|---|---|
| Entity FTS search | < 50ms |
| 1-hop neighbors (20 results) | < 30ms |
| 2-hop expansion | < 100ms |
| Shortest path (depth 6) | < 200ms |

เกณฑ์: Neo4j latency ≤ MariaDB × 1.5

### Definition of Done — KG-S1
- [ ] Data parity ≥ 95% หรือ re-sync เสร็จ
- [ ] Indexes และ constraints สร้างแล้วทั้งหมด
- [ ] Benchmark ผ่านทุก latency target

---

## KG-S2: Switch Read Path
**เวลา:** ~3–4 วัน

### งาน KG-2.1 — เพิ่ม Cypher ที่ขาดใน `neo4j.rs`

`build_search_with_hops_cypher()` — แทน SQL UNION ALL 2-hop:
```cypher
MATCH (root:Entity {tenant_id: $tenant_id})
WHERE elementId(root) = $root_id
MATCH (root)-[r1:RELATES_TO]->(n1:Entity {tenant_id: $tenant_id})
RETURN n1.name, n1.entity_type, r1.relation_type, 1 AS hop
UNION ALL
MATCH (root:Entity {tenant_id: $tenant_id})
WHERE elementId(root) = $root_id
MATCH (root)-[r1:RELATES_TO]->(mid:Entity)-[r2:RELATES_TO]->(n2:Entity)
WHERE mid.tenant_id = $tenant_id AND n2.tenant_id = $tenant_id
RETURN n2.name, n2.entity_type,
       (r1.relation_type + ' -> ' + r2.relation_type), 2 AS hop
LIMIT $limit
```

`build_fulltext_search_cypher()` — แทน MariaDB MATCH/AGAINST:
```cypher
CALL db.index.fulltext.queryNodes('entity_name_ft', $query)
YIELD node, score
WHERE node.tenant_id = $tenant_id
RETURN node.name, node.entity_type, node.properties, score
ORDER BY score DESC LIMIT $limit
```

### งาน KG-2.2 — `Neo4jGraphRetriever` ใน `retrieval/graph.rs`

```rust
pub struct Neo4jGraphRetriever { neo4j: Arc<Neo4jService> }

#[async_trait]
impl GraphRetriever for Neo4jGraphRetriever {
    async fn search(&self, query: &str, tenant_id: &str, limit: usize)
        -> Result<Vec<GraphSearchResult>>
    {
        // extract_search_terms() — คงไว้ (pure Rust)
        // build_fulltext_search_cypher() แทน SQL FULLTEXT
        // build_search_with_hops_cypher() แทน SQL UNION ALL
        // compute_match_score() — คงไว้ (pure Rust)
    }
}
```

### งาน KG-2.3 — Feature Flag ใน `routes/search.rs`

```rust
// USE_NEO4J_GRAPH=true|false
let retriever: Box<dyn GraphRetriever> = if use_neo4j_graph {
    Box::new(Neo4jGraphRetriever::new(neo4j_svc.clone()))
} else {
    Box::new(SqlGraphRetriever::new(pool.clone()))
};
```

### งาน KG-2.4 — Port `graph_analytics.rs` → Cypher

- God Nodes: `MATCH (n)-[r]-() RETURN n, count(r) ORDER BY count(r) DESC`
- Surprising Connections: `shortestPath()` ที่มีอยู่แล้วใน `neo4j.rs`

### Definition of Done — KG-S2
- [ ] `Neo4jGraphRetriever` compile และ unit test ผ่านทั้งหมด
- [ ] `USE_NEO4J_GRAPH=true` บน staging — search result ตรงกับ MariaDB ≥ 90%
- [ ] `graph_analytics.rs` ทำงานผ่าน Neo4j
- [ ] Rollback ทำได้ใน < 1 นาที (เปลี่ยน env var เท่านั้น)

---

## KG-S3: Cutover & Cleanup
**เวลา:** ~2 วัน

### งาน KG-3.1 — ลบ Dual-Write ออกจาก `routes/graph.rs`

หลัง cutover extraction endpoint ทำแค่:
1. Upsert ลง Neo4j → ดึง `elementId` ✅
2. ลบ MariaDB insert code ทั้งหมด
3. `neo4j_svc: Option<>` → required (ไม่มี Neo4j = error ชัดๆ)

### งาน KG-3.2 — Drop MariaDB KG Tables

```sql
-- migration: 20260601000000_drop_kg_mariadb_tables.sql
DROP TABLE IF EXISTS kg_relations;
DROP TABLE IF EXISTS kg_entities;
-- kg_extraction_runs คงไว้ (ยังใช้ track pipeline jobs)
```

### งาน KG-3.3 — ลบ Dead Code

- `retrieval/graph.rs`: ลบ `SqlGraphRetriever` และ SQL queries ทั้งหมด
- `routes/graph.rs`: ลบ `SELECT ... neo4j_node_id FROM kg_entities` และ references ทั้งหมด
- `src/bin/clean_kg.rs`: rewrite ใช้ `MATCH (n:Entity) ... DETACH DELETE`

### งาน KG-3.4 — Update Settings UI

`ro-ai-dashboard/src/app/settings/page.tsx:232` hardcode Neo4j URL → query จาก `/api/v1/graph/health` แทน

### Definition of Done — KG-S3
- [ ] `USE_NEO4J_GRAPH` env var ลบออก — Neo4j เป็น default เสมอ
- [ ] Migration `drop_kg_mariadb_tables` รันผ่านบน staging
- [ ] `grep -r "kg_entities\|kg_relations" src/` ไม่เจออะไร
- [ ] E2E: search → neighbors → path ผ่านทั้งหมด
- [ ] MariaDB KG tables ถูก drop บน production

---

---

# Track B: Medical Agentic Ecosystem

---

## B-S1: Schema Foundation & Memory Architecture
**เดือน:** พฤษภาคม 2026
**เป้าหมาย:** วางรากฐาน storage ทั้งหมดสำหรับ Medical Agent และ import PrimeKG ลง Neo4j

---

### Backend: PostgreSQL Schema

**งาน 1.1 — Medical Schema บน `asgard_postgres:5432`**

สร้าง schema แยกเพื่อไม่กระทบ Zitadel ที่ใช้ `public` schema อยู่

```sql
CREATE SCHEMA IF NOT EXISTS medical;

CREATE TABLE medical.pubmed_articles (
    pmid         BIGINT PRIMARY KEY,
    title        TEXT NOT NULL,
    abstract     TEXT,
    mesh_terms   JSONB,
    pub_date     DATE,
    fetched_at   TIMESTAMPTZ DEFAULT now()
);
CREATE INDEX ON medical.pubmed_articles (pub_date DESC);
CREATE INDEX ON medical.pubmed_articles USING GIN (mesh_terms);

CREATE TABLE medical.clinical_guidelines (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title          TEXT NOT NULL,
    body           TEXT,
    source         TEXT,        -- "WHO", "AHA", "CPIC", ...
    version        TEXT,
    effective_date DATE
);

CREATE TABLE medical.agent_audit_log (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  TEXT NOT NULL,
    agent_id   TEXT,
    tool_name  TEXT NOT NULL,
    input      JSONB,
    output     JSONB,
    latency_ms INT,
    created_at TIMESTAMPTZ DEFAULT now()
);
CREATE INDEX ON medical.agent_audit_log (tenant_id, created_at DESC);
```

**ไม่มี** `primekg_entities` / `primekg_relations` ใน PostgreSQL — ข้อมูล graph ทั้งหมดอยู่ใน Neo4j

---

### Backend: PrimeKG ETL → Neo4j

**งาน 1.2 — Neo4j Heap Tuning (ต้องทำก่อน import)**

```yaml
# k8s/02-services/neo4j/deployment.yaml
- name: NEO4J_server_memory_heap_initial__size
  value: "2G"
- name: NEO4J_server_memory_heap_max__size
  value: "4G"
- name: NEO4J_server_memory_pagecache_size
  value: "2G"
```

deploy และ verify Neo4j start ได้ก่อนเริ่ม import

**งาน 1.3 — Node Label Design (Dual Label Pattern)**

```cypher
-- Tenant KG (เดิม, ไม่เปลี่ยน)
(:Entity {tenant_id: "abc", name: "Metformin", type: "drug"})

-- PrimeKG nodes (ไม่มี tenant_id — เป็น global public data)
(:PrimeKG:Disease  {entity_id: "MONDO:0005148", name: "Type 2 Diabetes", ontology_id: "...", source: "primekg"})
(:PrimeKG:Drug     {entity_id: "DrugBank:DB00331", name: "Metformin", ontology_id: "...", source: "primekg"})
(:PrimeKG:Gene     {entity_id: "NCBI:3643", name: "INSR", source: "primekg"})
(:PrimeKG:Protein  {...})
(:PrimeKG:Pathway  {...})

-- Relationship types
(drug:PrimeKG:Drug)-[:TREATS {weight: f64, evidence_count: i32, source: "primekg"}]->(disease:PrimeKG:Disease)
(drugA:PrimeKG:Drug)-[:INTERACTS_WITH {...}]->(drugB:PrimeKG:Drug)
(gene:PrimeKG:Gene)-[:ASSOCIATED_WITH {...}]->(disease:PrimeKG:Disease)
(drug:PrimeKG:Drug)-[:TARGETS {...}]->(protein:PrimeKG:Protein)
(gene:PrimeKG:Gene)-[:INTERACTS_WITH {...}]->(gene2:PrimeKG:Gene)

-- Cross-link: Tenant entity ↔ PrimeKG node (สร้างโดย ETL Phase 3)
(:Entity {tenant_id: "abc"})-[:SAME_AS {
    confidence: 0.9,
    match_strategy: "ontology_id",  -- "ontology_id" | "name" | "synonym"
    linked_at: datetime(),
    primekg_version: "2024-01"
}]->(:PrimeKG:Drug)
```

**งาน 1.4 — ETL Script (Checkpoint Pattern)**

Import ทั้งหมด 8M edges โดยแบ่งเป็น phase เพื่อ recovery กรณี fail กลางทาง:

```
Phase 1: Import Nodes (129K — เร็ว, ทำก่อน)
  - LOAD CSV kg.csv → GROUP BY node type → CREATE (:PrimeKG:<Type>)
  - สร้าง index: entity_id, name ทันทีหลัง import nodes
    CREATE INDEX FOR (n:PrimeKG) ON (n.entity_id)
    CREATE INDEX FOR (n:PrimeKG) ON (n.name)

Phase 2: Import Edges ทีละ relation_type (checkpoint ต่อ type)
  disease-drug    (~600K)  → LOAD CSV → APOC periodic.iterate → ✓ checkpoint
  drug-drug       (~400K)  → LOAD CSV → APOC periodic.iterate → ✓ checkpoint
  disease-gene    (~400K)  → LOAD CSV → APOC periodic.iterate → ✓ checkpoint
  drug-protein    (~700K)  → LOAD CSV → APOC periodic.iterate → ✓ checkpoint
  gene-gene PPI   (~3M)    → LOAD CSV → APOC periodic.iterate → ✓ checkpoint
  อื่นๆ           (~3M)    → LOAD CSV → APOC periodic.iterate → ✓ checkpoint

Phase 3: SAME_AS Cross-link ETL
  Priority 1: ontology_id exact match → confidence 1.0
  Priority 2: normalized name match   → confidence 0.9
  Priority 3: UMLS/MeSH synonym       → confidence 0.75
  Rule: confidence < 0.75 → ไม่สร้าง link (prefer miss over false positive)

Phase 4: Verify
  MATCH (n:PrimeKG) RETURN labels(n), count(n) -- ตรวจ node counts
  MATCH ()-[r]->(:PrimeKG) RETURN type(r), count(r) -- ตรวจ edge counts
```

---

### Backend: Unified Dual-Memory Schema

**งาน 1.5 — Redis Key Schema**

ออกแบบเป็น unified foundation สำหรับทั้ง Medical Agent (Sprint 1) และ Bifrost (Sprint 4)

```
session:{session_id}:patient_context
  Type:   HASH
  Fields: chief_complaint, medications, labs, differentials, ...
  EXPIRE: SET ทันทีเมื่อ session resolved
  Owner:  Medical Agent (Sprint 1)

session:{session_id}:history
  Type:   LIST (JSON strings: {role, content, ts})
  EXPIRE: 24h
  Owner:  ออกแบบสำหรับ Bifrost migration (Sprint 4)
          ปัจจุบัน Bifrost ยังใช้ MariaDB swarm_checkpoints อยู่

session:{session_id}:report_progress
  Type:   HASH
  Fields: dim_1 ... dim_10 → "pending" | "running" | "done" | "skipped"
  EXPIRE: 24h
  Owner:  Research Report SSE (Sprint 3)
```

**งาน 1.6 — Qdrant Collections**

```
"clinical-wisdom"
  Scope:   global (ไม่มี tenant_id — medical knowledge สาธารณะ)
  Vector:  embed(clinical_guidelines.body)
  Payload: {source, guideline_id, effective_date, version}
  Owner:   Medical Agent (Sprint 1)

"pubmed-abstracts"
  Scope:   global
  Vector:  embed(title + abstract)
  Payload: {pmid, pub_date, mesh_terms[]}
  Owner:   pubmed_search tool (Sprint 2) — สร้าง collection ไว้ก่อนได้เลย

"agent-memory"
  Scope:   tenant-scoped (payload: tenant_id)
  Owner:   Bifrost migration (Sprint 4) — ยังไม่ต้องสร้างตอนนี้
           แทนที่ Memvid .mv2 files ในอนาคต
```

---

### Backend: Skill Registry

**งาน 1.7 — สร้าง `mimir-skills/` Directory**

```
mimir-skills/
  _template/
    SKILL.md          # Goal, SOP Steps, Input Schema, Output Schema, Error Handling
    schema.json       # JSON Schema draft-07
    examples/
      example_01.json
  pubmed-search/           # Sprint 2
  drug-drug-interaction/   # Sprint 2
  clinical-trial-matching/ # Sprint 2
  differential-diagnosis/  # Sprint 2
  cpic-pharmacogenomics/   # Sprint 2
  README.md                # Convention ทั้งหมด
```

---

### Definition of Done — B-S1

```
Schema & Storage
[ ] PostgreSQL schema "medical" สร้างแล้ว — tables: pubmed_articles, clinical_guidelines, agent_audit_log
[ ] Neo4j heap tuned: 4G heap, 2G pagecache — deploy และ verify ก่อน import

PrimeKG ETL
[ ] Import nodes ครบ 129K — verify ด้วย count query per label
[ ] Import edges ครบ ~8M — checkpoint log บันทึกทุก relation_type
[ ] SAME_AS links: confidence ≥ 0.75 ทั้งหมด — ไม่มี link ที่ต่ำกว่านี้
[ ] Neo4j indexes สร้างหลัง import — 1-hop query latency < 100ms

Memory
[ ] Redis: set/get/expire บน session:{sid}:patient_context ทดสอบผ่าน
[ ] Qdrant: collection "clinical-wisdom" สร้างแล้ว payload schema ตรงตาม spec
[ ] Sprint 4 backlog ticket: Bifrost memory migration (swarm_checkpoints → Redis, Memvid → Qdrant)

Skill Registry
[ ] mimir-skills/_template/ ครบทุกไฟล์
[ ] SKILL.md template ครบ: Goal, SOP, Input/Output schema, Error handling
[ ] schema.json เป็น JSON Schema draft-07 valid

Constraints
[ ] ไม่มี table ใหม่ใน MariaDB
[ ] ไม่มี primekg_* table ใน PostgreSQL
[ ] SAME_AS ไม่มี link confidence < 0.75
```

---

## B-S2: Hermodr MCP Server + PubMed Integration
**เดือน:** มิถุนายน 2026
**เป้าหมาย:** สร้าง Hermodr ให้เป็น MCP Server จริง โดย `pubmed_search` ต้องทำงานครบ loop (cache → BigQuery → NCBI)

---

### งาน 2.1 — MCP Server Scaffold

- สร้าง Hermodr service ให้ implement MCP Server Protocol
- Tool registration: แต่ละ tool ใน `mimir-skills/` ลงทะเบียนผ่าน MCP tool manifest
- Credentials: ทุก API key ดึงจาก Vault เท่านั้น (ห้าม hardcode)

### งาน 2.2 — `pubmed_search` Tool (Priority #1)

```
call pubmed_search(query, filters)
    │
    ├── 1. Qdrant "pubmed-abstracts" → cache hit → return
    │
    └── 2. cache miss
            │
            ▼
        BigQuery: bigquery-public-data.pubmed
        (Service Account ผ่าน Vault)
            │
            ├── save metadata → PostgreSQL medical.pubmed_articles
            ├── embed(title + abstract) → Qdrant "pubmed-abstracts"
            └── return ToolResult

Fallback: article อายุ < 30 วัน → NCBI Entrez API
(BigQuery snapshot lag ~2 สัปดาห์)

Response: {pmid, title, abstract, mesh_terms[], pub_date, source: "cache"|"bigquery"|"ncbi"}
```

### งาน 2.3 — 4 Skills ที่เหลือ

| Tool | Source | Priority |
|---|---|---|
| `drug_drug_interaction` | DrugBank / DDInter API | สูงมาก — patient safety |
| `clinical_trial_matching` | ClinicalTrials.gov API | สูง |
| `differential_diagnosis` | MedOpenClaw SOP + PrimeKG Neo4j | สูง |
| `cpic_pharmacogenomics` | CPIC API | กลาง |

แต่ละ tool: เขียน `SKILL.md` + `schema.json` + ทดสอบ 3 examples จริง
Output ทุก tool ต้อง normalize เป็น `ToolResult` schema เดียวกัน

### Definition of Done — B-S2
- [ ] Hermodr MCP Server start ได้ — Mimir Agent connect ผ่าน MCP protocol ได้
- [ ] `pubmed_search` ทดสอบ cache hit และ BigQuery fallback ผ่านทั้งคู่
- [ ] `drug_drug_interaction` ทดสอบ major interaction ตรงกับ DrugBank reference
- [ ] 5 tools ทั้งหมดมี `SKILL.md`, `schema.json`, และ 3 examples
- [ ] ทุก API key ดึงจาก Vault — ไม่มีค่า hardcode ใน code หรือ env file
- [ ] E2E: Mimir Agent → Hermodr MCP → tool call → structured result

---

## B-S3: Researcher Mode + ToolRAG
**เดือน:** กรกฎาคม 2026
**เป้าหมาย:** เปิด "Researcher Mode" — แพทย์เห็น AI ทำงานแบบ real-time ผ่าน Report-First workflow + ToolRAG

---

### งาน 3.1 — `ResearchReport` (10 Dimensions)

```rust
// Rust implementation (ไม่ใช่ Python ตามที่ draft เดิมเขียน — อยู่ใน Mimir API)
struct ResearchReport {
    dimensions: [&'static str; 10],  // epidemiology, genetic_architecture, ...
}

impl ResearchReport {
    async fn run(&self, query: &str, report_id: &str) -> impl Stream<Item = DimensionUpdate> {
        // spawn 10 dimension tasks พร้อมกัน (parallel)
        // ใช้ Semaphore(3) จำกัด concurrent LLM calls
        // ใช้ CancellationToken ป้องกัน task leak เมื่อ client disconnect
        // per-dimension timeout: 90s — เกินให้ skip + log ไม่ crash ทั้งหมด
        // progress tracking: HINCRBY report:{id} completed_dimensions 1 (atomic)
    }
}
```

10 dimensions: `epidemiology`, `genetic_architecture`, `pathophysiology`, `diagnostic_criteria`, `standard_of_care`, `emerging_therapies`, `drug_interactions`, `comorbidity_network`, `patient_phenotypes`, `evidence_quality`

### งาน 3.2 — SSE Streaming Endpoint

```
GET /api/v1/research/stream/{report_id}
Content-Type: text/event-stream

Pattern: Axum Sse<ReceiverStream> + tokio mpsc channel (เดียวกับ chat.rs)

Keep-alive: ส่ง ":keepalive\n\n" ทุก 30s (CF drops idle HTTP ที่ ~100s)

Events:
  event: dimension_complete
  data: {"dimension": "epidemiology", "content": "...", "sources": [...], "progress": "1/10"}

  event: report_done
  data: {"summary": "...", "total_dimensions": 10}

Redis keys ที่ใช้:
  session:{report_id}:report_progress   HASH — track per-dimension state
  EXPIRE: 24h
```

### งาน 3.3 — ToolRAG (TxAgent pattern)

- Embed description ของทุก tool ใน `mimir-skills/` → Qdrant `tool-registry`
- `select_tools(query, top_k=3)` — ดึง 3–5 tools ที่ relevance สูงสุด
- Inject เฉพาะ selected tools เข้า agent context (ไม่ส่งทั้งหมด)

### งาน 3.4 — Progressive Report Panel (Frontend)

- Panel ใหม่ในหน้า Chat/Query
- 10 dimension cards: `pending → loading → complete`
- แต่ละ card expand ได้ — แสดง content + sources
- Executive Summary โชว์หลัง dimension ครบ 10

### งาน 3.5 — Source Citation Component (Frontend)

- Source badge: PubMed PMID / ClinicalTrials NCT ID / DrugBank ID
- คลิก badge → ดู abstract ย่อ (จาก `medical.pubmed_articles` cache)

### Definition of Done — B-S3
- [ ] `ResearchReport` รัน 10 dimensions สำหรับ 3 test diseases
- [ ] SSE stream ไม่ถูก Cloudflare ตัด — ครบ 10 dimensions ทุกครั้ง
- [ ] Semaphore(3) ป้องกัน rate limit — ทดสอบ 5 concurrent reports
- [ ] CancellationToken ทำงาน — disconnect client → tasks cancel ใน < 2s
- [ ] ToolRAG เลือก tool ถูกต้อง ≥ 85% บน test set 20 queries
- [ ] Frontend แสดง real-time progress ถูกต้องตาม SSE events
- [ ] Source citation แสดง PMID / NCT ID และ link ไป source จริงได้

---

## B-S4: Database Migration + Bifrost Memory Upgrade
**เดือน:** Q3 2026 (หลัง B-S3 stable)
**เป้าหมาย:** ย้าย Mimir Core AI จาก MariaDB → PostgreSQL และ upgrade Bifrost memory เป็น unified Redis+Qdrant

---

### งาน 4.1 — Syntax Audit (MariaDB → PostgreSQL)

Scan ทุก `.sql` ใน `mimir-core-ai/migrations/` และ query files:

| MySQL syntax | PostgreSQL replacement |
|---|---|
| `AUTO_INCREMENT` | `GENERATED ALWAYS AS IDENTITY` |
| `TINYINT` | `SMALLINT` หรือ `BOOLEAN` |
| `ENUM` | `VARCHAR` + CHECK constraint |
| `LIMIT` ใน `UPDATE`/`DELETE` | CTE หรือ subquery |
| `ON DUPLICATE KEY UPDATE` | `INSERT ... ON CONFLICT DO UPDATE` |
| MySQL JSON functions | PostgreSQL JSONB operators |

### งาน 4.2 — Migration Script

- ใช้ `pgloader` สำหรับ data migration (handle type conversion อัตโนมัติ)
- Validation queries: row count + checksum ทุก table หลัง migrate
- Rollback plan: MariaDB ยัง run parallel อีก 14 วันหลัง cutover

### งาน 4.3 — Bifrost Memory Migration (flagged ตั้งแต่ B-S1)

ย้าย Bifrost จากระบบ memory เดิมมาใช้ unified foundation ที่ design ไว้ใน B-S1:

```
เดิม (Bifrost):
  swarm_checkpoints (MariaDB) → turn history ไม่มี TTL
  Memvid .mv2 files (local disk) → long-term memory หาย เมื่อ pod restart

ใหม่ (Unified):
  session:{sid}:history (Redis, EXPIRE 24h) → แทน swarm_checkpoints
  Qdrant "agent-memory" (tenant-scoped) → แทน Memvid .mv2 files
```

Steps:
1. สร้าง Qdrant collection `agent-memory` (payload: `tenant_id`, `session_id`, `ts`)
2. Update `overseer.rs`: เขียน/อ่าน turn history จาก Redis แทน MariaDB
3. Update `memvid_manager.rs`: commit/search ผ่าน Qdrant แทน `.mv2` files
4. Drop `swarm_checkpoints` จาก MariaDB schema หลัง validate

### งาน 4.4 — Huginn & Muninn Evaluation

- **Huginn** (660K audit logs): ถ้าต้องการ cross-service query → migrate ลง `asgard_postgres`, ถ้าไม่ → คงไว้ SQLite
- **Muninn** (48K memory): ถ้าจะเป็น backbone Long-term Memory → merge เข้า `clinical_guidelines`, ถ้าไม่ → คงไว้ SQLite

### งาน 4.5 — Services ที่ไม่ต้อง Migrate

| Service | DB | การตัดสินใจ | เหตุผล |
|---|---|---|---|
| **Eir/OpenEMR** | MariaDB | ❌ ห้ามแตะ | Vendor app ผูกกับ MySQL syntax |
| **Forseti** | SQLite | ❌ คงไว้ | Test runner |
| **Fenrir** | SQLite | ❌ คงไว้ | Local test artifacts |
| **Mjolnir** | SQLite | ❌ คงไว้ | 12K เล็กเกินไป |
| **Mega-Care** | Firestore + Cloud SQL | ❌ คงไว้ | GCP-native stack แยก |

### Definition of Done — B-S4
- [ ] Syntax audit report ครบ
- [ ] Migration ผ่านบน staging ก่อน production
- [ ] Row count และ data validation ผ่านทุก table
- [ ] Mimir API ทดสอบบน PostgreSQL ผ่าน smoke test ทั้งหมด
- [ ] MariaDB stand-by อีก 14 วันหลัง cutover
- [ ] Bifrost: swarm_checkpoints → Redis ผ่าน — turn history ยังครบ
- [ ] Bifrost: Memvid → Qdrant "agent-memory" ผ่าน — search ยังทำงาน
- [ ] Huginn/Muninn decision document อัปเดต

---

## 📊 Summary

| Track | Sprint | งานหลัก | เวลา |
|---|---|---|---|
| A | KG-S1 | Validate + Indexes | 1 วัน |
| A | KG-S2 | Switch Read Path | 3–4 วัน |
| A | KG-S3 | Cutover + Cleanup | 2 วัน |
| B | B-S1 | Schema + PrimeKG ETL + Memory | พ.ค. 2026 |
| B | B-S2 | Hermodr MCP + PubMed | มิ.ย. 2026 |
| B | B-S3 | Researcher Mode + ToolRAG | ก.ค. 2026 |
| B | B-S4 | DB Migration + Bifrost Memory | Q3 2026 |

---

## ⚠️ Dependencies & Risks

| ความเสี่ยง | ระดับ | Mitigation |
|---|---|---|
| PrimeKG license (CC BY 4.0) | **ต่ำ** | CC BY 4.0 อนุญาต commercial use — ต้องระบุ attribution เท่านั้น |
| Neo4j OOM ระหว่าง import 8M edges | สูง | Tune heap 4G ก่อน import — checkpoint per relation type ป้องกัน restart จาก 0 |
| SAME_AS false positive → wrong medical result | สูง | confidence threshold ≥ 0.75, ontology_id priority, เก็บ match_strategy บน relationship |
| Parallel dimensions LLM rate limit | กลาง | Semaphore(3) จำกัด concurrent LLM calls |
| Parallel dimensions task leak | กลาง | CancellationToken propagated ทุก task — cancel เมื่อ SSE disconnect |
| BigQuery pubmed snapshot lag 1–2 สัปดาห์ | กลาง | NCBI Entrez fallback สำหรับ articles อายุ < 30 วัน |
| Cloudflare 100s idle timeout บน SSE | กลาง | keepalive comment ทุก 30s ตาม pattern เดิมของ Bifrost |
| Bifrost Memvid .mv2 หาย เมื่อ pod restart | กลาง | ยอมรับก่อน Sprint 4 — migration แก้ root cause |
| MariaDB → PostgreSQL syntax incompatibility | กลาง | Syntax audit ก่อน B-S4 เริ่ม |
| Python sandbox security (Medea AnalysisExecution) | สูง | เลื่อนไป post-v1 — ต้อง design isolation ก่อน implement |
| Neo4j cascade fail เมื่อ parallel queries สูง | กลาง | Load test Neo4j ก่อน enable parallel dimensions |

---

**วันที่อัปเดต:** 1 พฤษภาคม 2026
**สถานะ:** Ready for Track A KG-S1 + Track B B-S1 Kickoff
**Architect:** Antigravity AI
