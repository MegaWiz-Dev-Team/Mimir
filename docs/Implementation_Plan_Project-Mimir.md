# 🗺️ Implementation Plan: Project-Mimir (RO AI-Native Evolution)

แผนการพัฒนาแบ่งเป็น **5 Phases** ตามที่ BRD กำหนด โดยแต่ละ Phase มีรายละเอียด Tasks, Deliverables และ Gate Criteria ชัดเจน

> [!IMPORTANT]
> แผนนี้ออกแบบสำหรับ **Mac mini M4 Pro (64GB RAM)** เป็นเป้าเครื่อง Dev/Staging ทั้งหมด

---

## Phase 1: Infrastructure & Foundation (5 สัปดาห์)

**เป้าหมาย:** เตรียม Environment, ยืนยันว่า LLM ตอบได้ภายใน 2 วินาที, สร้างโครงสร้าง Rust Project

### Sprint 1.1 — Environment Setup (สัปดาห์ 1-2)

#### [NEW] Docker Compose Configuration
#### [NEW] `docker-compose.yml`

| Service | Image            | Port       | RAM   |
| ------- | ---------------- | ---------- | ----- |
| Ollama  | `ollama/ollama`  | 11434      | 22 GB |
| Qdrant  | `qdrant/qdrant`  | 6333, 6334 | 6 GB  |
| MariaDB | `mariadb:11`     | 3306       | 4 GB  |
| Redis   | `redis:7-alpine` | 6379       | 2 GB  |

**Tasks:**
- [x] สร้าง `docker-compose.yml` สำหรับ Data Layer (Qdrant, MariaDB, Redis)
- [x] ติดตั้ง Ollama native บน macOS (ใช้ MLX acceleration)
- [x] ดาวน์โหลด Model: **`gemma:2b`** (สำหรับ Dev บน M3) และ `all-minilm:l6-v2`
- [x] สร้าง Script ทดสอบ LLM latency (Result: **8.40s** on M3, target **1.8s** not met)
- [x] กำหนดค่า Docker network ให้ Service ข้ามหาเห็นกัน (Defined `mimir_network`)

### Sprint 1.2 — Rust Project Bootstrap (สัปดาห์ 2-3)

#### [NEW] Rust Project `ro-ai-bridge`

```
ro-ai-bridge/
├── Cargo.toml
├── src/
│   ├── main.rs              # Axum server bootstrap
│   ├── config.rs            # Environment config
│   ├── routes/
│   │   ├── mod.rs
│   │   └── health.rs        # GET /health
│   ├── agents/
│   │   └── mod.rs
│   ├── tools/
│   │   └── mod.rs
│   ├── middleware/
│   │   └── mod.rs
│   ├── services/
│   │   └── mod.rs
│   ├── models/
│   │   └── mod.rs
│   ├── db/
│   │   └── mod.rs
│   └── utils/
│       └── mod.rs
├── config/
│   ├── personas/
│   └── safety/
└── tests/
```

**Tasks:**
- [x] `cargo init ro-ai-bridge` พร้อม Dependencies ตาม Framework Analysis
- [x] สร้าง Axum server พื้นฐาน + `GET /health` endpoint
- [x] สร้าง Config module (อ่าน env: `OLLAMA_URL`, `QDRANT_URL`, `MARIADB_URL`, `REDIS_URL`)
- [x] ทดสอบ Rig + Ollama provider ด้วย Hello World agent (Using `rig-core` 0.10.0)
- [x] สร้างโครงสร้าง Module ทั้งหมด (เปล่า ๆ กำหนด public interface)

### Sprint 1.3 — Database Schema & Data Pipeline (สัปดาห์ 3-5)

#### [NEW] MariaDB Schema
#### [NEW] `migrations/001_ai_tables.sql`

**Tables (ไม่แตะ rAthena tables):**
- `ai_npc_persona` — เก็บบุคลิก NPC
- `ai_chat_session` — บทสนทนาต่อเนื่อง
- `ai_action_log` — Audit Trail ทุก Action
- `ai_economy_daily` — ลิมิตเศรษฐกิจรายวัน (server-wide)
- `ai_player_daily_limits` — ลิมิตต่อผู้เล่นต่อวัน
- `ai_gm_events` — Event ที่ AI สร้าง
- `ai_bot_detection` — Log การตรวจจับ Bot

**Tasks:**
- [ ] เขียน SQL Migration สำหรับ 7 tables
- [ ] สร้าง Qdrant collections: `ro_items`, `ro_monsters`, `ro_skills`, `ro_maps`, `ro_lore`, `ro_quests`
- [ ] สร้าง Data Ingestion Script (rAthena DB → Embedding → Qdrant)
- [ ] ทดสอบ Vector search ด้วย sample queries
- [ ] สร้าง SQLx models + query functions ใน Rust

### Sprint 1.4 — RAG Data Pipeline (สัปดาห์ 5-6)

#### [NEW] `scraper-service/` (Rust + Chromiumoxide)

**เป้าหมาย:** ดึงข้อมูลจาก `rolth.maxion.gg` (SPA) แต่ใช้ **Rust** เพื่อรวม Stack เป็นหนึ่งเดียว

**Tasks:**
- [x] Add crate `chromiumoxide` (หรือ `headless_chrome`) และ `scraper` (HTML parsing) ใน `Cargo.toml`
- [x] สร้าง Module `src/services/scraper.rs` สำหรับคุม Browser instance
- [x] Implement Scraper logic:
    - [x] Login (Placeholder created)
    - [x] Wait for selector (รอ JS โหลด)
    - [x] Scroll to bottom (Infinite scroll handling)
- [x] Implement GitBook MCP Client & Initial Scraping
- [x] **[Refactor]** `fetch_wiki.rs` to extract clean content (remove scripts/nav/footer)
    - [x] Parse DOM with `scraper`
    - [x] Extract `<main>` and sanitize (remove `<script>`, `<style>`, `<nav>`)
- [x] **[NEW]** Implement Multi-Agent Q/A Workshop (`src/agents/wiki_workshop/`)
    - [x] **Config**: `config/wiki_agents.toml` for model selection (Local/Cloud)
    - [x] **Agent 1**: `QAGeneratorAgent` (Local LLM/Gemini) -> Generates Q/A
    - [x] **Agent 2**: `ACUExtractorAgent` (Gemini 2.5 Flash) -> Extracts Atomic Content Units
    - [x] **Agent 3**: `CoverageVerifierAgent` (Gemini 2.5 Flash) -> Calculates Coverage %
    - [x] **Orchestrator**: `generate_qa` binary to run the pipeline
- [ ] Integrate เข้ากับ Axum Cron Job (ไม่ต้องรัน service แยก) (Deferred to Mac mini - See `docs/Sprint_1.4_Cron_Integration_Plan.md`)



### 🚦 Phase 1 Gate

> **ต้องผ่านก่อน** ไป Phase 2:
> - [ ] Ollama ตอบ single inference ≤ 1.8 วินาที
> - [ ] Qdrant vector search ทำงานได้ถูกต้อง (precision > 80%)
> - [ ] MariaDB schema ใช้ได้ + CRUD ทำงาน
> - [x] Rust server `/health` ทำงานได้
> - [x] RAG Pipeline (ดึงข้อมูลส่วน Wiki สำเร็จ 100%)


---

## Phase 2: Core Features — NPC Chat + Oracle + Safety (7 สัปดาห์)

**เป้าหมาย:** Module A (NPC Chat) + Module B (Oracle RAG Bot) + Safety Filter ทำงานได้ครบ

### Sprint 2.1 — Tier 1: NPC Chat Agent (สัปดาห์ 6-8)

#### [NEW] `src/agents/tier1_simple.rs`
#### [NEW] `src/agents/tier_router.rs`
#### [NEW] `src/routes/chat.rs`
#### [NEW] `src/services/persona_manager.rs`
#### [NEW] `config/personas/*.yaml`

**Tasks:**
- [ ] สร้าง Tier Router logic (NpcType → AgentTier mapping)
- [ ] สร้าง PersonaManager: โหลด YAML → สร้าง Rig Agent dynamically
- [ ] สร้าง NPC Persona YAML: `sage_ariel.yaml`, `fortune_teller.yaml`, etc.
- [ ] Implement `POST /api/v1/chat` endpoint
- [ ] สร้าง Session management (Redis: เก็บ chat history ต่อ session_id)
- [ ] ทดสอบ: NPC ตอบตามบุคลิก, ≤ 2 วินาที
- [ ] Implement Response Streaming (ส่งทีละ token)

### Sprint 2.2 — Tier 2: Oracle RAG Agent (สัปดาห์ 8-10)

#### [NEW] `src/agents/tier2_rag.rs`
#### [NEW] `src/routes/oracle.rs`
#### [NEW] `src/tools/query_mob_tool.rs`
#### [NEW] `src/tools/query_item_tool.rs`

**Tasks:**
- [ ] สร้าง Oracle Agent ด้วย Rig RAG Pipeline + rig-qdrant
- [ ] Implement Custom Tools: `QueryMobDbTool`, `QueryItemDbTool`
- [ ] Implement `POST /api/v1/oracle/query` endpoint
- [ ] สร้าง Confidence score + Source citation ใน response
- [ ] ทดสอบ: ค้นข้อมูลเกม 10 คำถาม, accuracy > 85%, ≤ 5 วินาที

### Sprint 2.3 — Safety Filter + Cloud Fallback (สัปดาห์ 10-12)

#### [NEW] `src/middleware/safety_filter.rs`
#### [NEW] `src/services/provider_chain.rs`
#### [NEW] `src/services/privacy_guard.rs`
#### [NEW] `src/services/cost_tracker.rs`
#### [NEW] `config/cloud_limits.yaml`

**Tasks:**
- [ ] Implement Pre-Filter: คำหยาบ, URL/Phone pattern, Toxicity score
- [ ] Implement Post-Filter: ตรวจ Action validity, ความยาว, หลุดบทบาท
- [ ] **[Cloud]** Implement Provider Chain: Local (Primary) → Cloud (Fallback)
- [ ] **[Cloud]** Implement Privacy Guard: Scrub PII (Player ID, IP) before sending to Cloud
- [ ] **[Cloud]** Implement Cost Tracker: Daily limit ($5/day) + Kill switch
- [ ] Implement Economy Limiter endpoint
- [ ] Red-team testing: ลอง Prompt Injection 20+ patterns

### Sprint 2.4 — Game Action Tools (สัปดาห์ 10-12, พร้อมกับ 2.3)

#### [NEW] `src/tools/heal_tool.rs`
#### [NEW] `src/tools/buff_tool.rs`
#### [NEW] `src/tools/give_item_tool.rs`

**Tasks:**
- [ ] Implement HealTool (Rig Tool trait + Daily limit check + Audit log)
- [ ] Implement BuffTool (ระยะเวลา buff สูงสุด 5 นาที)
- [ ] Implement GiveItemTool (Whitelist items per NPC, block MVP cards)
- [ ] Integrate Tools เข้ากับ Tier 1 Agent (NPC ที่มี allowed_actions)
- [ ] Integrate Tools เข้ากับ Tier 2 Agent (Oracle + Homunculus)

### 🚦 Phase 2 Gate

> **ต้องผ่านก่อน** ไป Phase 3:
> - [ ] NPC Chat ตอบ ≤ 2 วินาที (P95)
> - [ ] Oracle ตอบถูกต้อง > 85% จาก test set
> - [ ] Safety Filter บล็อก Prompt Injection ได้ > 95%
> - [ ] Economy Limits ทำงานถูกต้อง 100%
> - [ ] Action Audit Trail บันทึกครบทุก Action

---

## Phase 3: AI GM + Revenue Features (4 สัปดาห์)

**เป้าหมาย:** Module C (AI GM) + Module D (Revenue) ทำงานได้ครบ

### Sprint 3.1 — Tier 3: AI GM Background Agent (สัปดาห์ 13-15)

#### [NEW] `src/agents/tier3_background.rs`
#### [NEW] `src/routes/gm.rs`
#### [NEW] `src/tools/spawn_mob_tool.rs`

**Tasks:**
- [ ] Implement Background Agent Loop (ทำงานเป็น Cron ทุก 15 นาที)
- [ ] Implement Bot Detection: วิเคราะห์ movement pattern, speed anomaly
- [ ] Implement Event Generator: spawn boss, drop rate boost เมื่อผู้เล่นน้อย
- [ ] สร้าง Internal Endpoints: `/internal/gm/analyze-logs`, `/internal/gm/trigger-event`
- [ ] สร้าง Bot Report + Economy Audit endpoints
- [ ] Implement GM Dashboard data endpoints

### Sprint 3.2 — Revenue Features (สัปดาห์ 15-16)

**Tasks:**
- [ ] Implement Smart Homunculus agent (Tier 2 + เฉพาะ VIP 2+)
- [ ] Implement Fortune Teller NPC (Tier 1 + Gacha token system)
- [ ] Implement AI Support agent (Tier 2 + ลด GM workload)
- [ ] สร้าง VIP tier check middleware (API Key → VIP level → Feature access)
- [ ] Implement Fortune Teller buff system (buff หมดใน 30 นาที)

### 🚦 Phase 3 Gate

> - [ ] AI GM ตรวจจับ Bot pattern ด้วย Precision > 80%
> - [ ] Event Generator สร้าง Event ตาม player online count
> - [ ] VIP Feature gating ทำงานถูกต้อง
> - [ ] Fortune Teller ไม่ให้ของเกินลิมิต

---

## Phase 4: Integration + Load Test + Beta (4 สัปดาห์)

**เป้าหมาย:** รวม Rust Middleware เข้ากับ rAthena จริง, Load Test, Beta Test

### Sprint 4.1 — rAthena C++ Integration (สัปดาห์ 17-18)

#### [MODIFY] rAthena Source — Script Engine

**Tasks:**
- [ ] เพิ่ม Script Commands: `ai_chat(npc_id, msg)`, `ai_action(npc_id, json)`
- [ ] Implement libcurl HTTP client (non-blocking, thread pool 4 threads)
- [ ] สร้าง Fallback: ถ้า AI ไม่ตอบใน 3 วินาที → ใช้ `mes` Script เดิม
- [ ] เขียน NPC Script ตัวอย่าง (Sage Ariel, Oracle, Fortune Teller)
- [ ] ทดสอบ End-to-End: ผู้เล่น → rAthena → Rust → AI → กลับเกม

### Sprint 4.2 — Fallback & Circuit Breaker (สัปดาห์ 18-19)

#### [NEW] `src/services/circuit_breaker.rs`

**Tasks:**
- [ ] Implement Circuit Breaker: 5 failures → open → 30s wait → half-open → 3 success → close
- [ ] Implement 4-level Fallback: L0 Normal → L1 Degraded → L2 Cloud (Gemini) → L3 Static
- [ ] สร้าง Health check ที่ report Fallback level
- [ ] ทดสอบ: Kill Ollama → verify switch to Cloud (Latency < 2s)
- [ ] ทดสอบ: Resume Ollama → verify switch back to Local

### Sprint 4.3 — Load Testing + Beta (สัปดาห์ 19-20)

**Tasks:**
- [ ] เขียน k6 load test scripts (target: 200 concurrent players)
- [ ] ทดสอบ Tier 1: 160 req/min sustained
- [ ] ทดสอบ Tier 2: 30-50 req/min sustained
- [ ] ทดสอบ Queue overflow: > 256 req → HTTP 429
- [ ] Optimize bottlenecks (LLM inference, DB queries)
- [ ] Beta test กับ 20-50 ผู้เล่นจริง (2 สัปดาห์)
- [ ] เก็บ Feedback + แก้บัค

### 🚦 Phase 4 Gate

> - [ ] รองรับ 200 คนพร้อมกัน, P95 Latency ≤ 2.5 วินาที (Tier 1)
> - [ ] Fallback ทำงานเมื่อ Ollama ล่ม, downtime < 1 วินาที
> - [ ] Error rate < 2% (5xx responses)
> - [ ] Beta users satisfaction > 70%

---

## Phase 5: Production & Cloud Migration (4 สัปดาห์)

**เป้าหมาย:** Monitoring ครบ, เตรียม Cloud migration path

### Sprint 5.1 — Monitoring & Observability (สัปดาห์ 21-22)

#### [NEW] `src/routes/metrics.rs`

**Tasks:**
- [ ] Implement Prometheus metrics endpoint (`/metrics`)
- [ ] สร้าง Grafana Dashboard: latency, throughput, error rate, RAM, fallback count
- [ ] ตั้ง Alert rules: P95 > 2.5s, 5xx > 2%, RAM > 58GB, Fallback > 10/hr
- [ ] Implement Economy Daily Report (AI Zeny given, Items given)
- [ ] Setup Log aggregation (structured JSON logs)

### Sprint 5.2 — Production Hardening (สัปดาห์ 22-23)

**Tasks:**
- [ ] Security audit: API Key rotation, TLS 1.3 enforcement
- [ ] Model SHA-256 verification on startup
- [ ] Chat log auto-cleanup (> 30 days)
- [ ] Backup scripts (MariaDB + Qdrant daily backup)
- [ ] Runbook documentation (Ops procedures)

### Sprint 5.3 — Cloud Migration Planning (สัปดาห์ 23-24)

**Tasks:**
- [ ] Dockerize Rust middleware (multi-stage build)
- [ ] สร้าง Cloud Run deployment config
- [ ] Document Cloud SQL / Qdrant Cloud / Memorystore migration steps
- [ ] Cost estimation: Google Cloud vs self-hosted
- [ ] Create migration playbook

---

## Verification Plan

### Automated Tests

**Unit Tests — `cargo test`:**
```bash
cd ro-ai-bridge && cargo test
```
- Tier Router: ทุก NpcType map ไป Tier ที่ถูกต้อง
- Safety Filter: Pre/Post filter catch ทุก test case
- Economy Limiter: ทุก limit enforce ถูกต้อง
- Persona Manager: โหลด YAML ได้ถูกต้อง

**Integration Tests — Docker Compose test profile:**
```bash
docker compose -f docker-compose.test.yml up -d
cargo test --test integration
```
- API → DB → Qdrant round-trip
- Chat endpoint full flow (mock LLM)
- Action endpoint with limit enforcement

**Load Tests — k6:**
```bash
k6 run tests/load/tier1_chat.js --vus 200 --duration 5m
k6 run tests/load/tier2_oracle.js --vus 50 --duration 5m
```
- Target: Tier 1 P95 ≤ 2 วินาที, Tier 2 P95 ≤ 5 วินาที

**Safety Tests — Red-team Scripts:**
```bash
cargo test --test safety_redteam
```
- 20+ Prompt Injection patterns
- Economy exploitation attempts
- Role escape attempts

### Manual Verification

> [!NOTE]
> เนื่องจากยังไม่มี rAthena test server จึงต้องทดสอบ E2E ด้วย manual testing

1. **E2E NPC Chat:** เปิด RO client → คุย NPC → ตรวจสอบว่า NPC ตอบตามบุคลิก + Action ทำงาน
2. **Oracle Accuracy:** ถามคำถามเกม 20 ข้อ → ตรวจสอบ accuracy ด้วย GM ที่รู้เกมดี
3. **Fallback Test:** `docker stop ollama` → ตรวจว่า NPC สลับไป static script ภายใน 0.5 วินาที
4. **Economy Test:** ลองให้ AI สั่ง action จนถึง daily limit → ตรวจว่า block ถูกต้อง

---

## Risk Mitigations ที่ต้องสร้างใน Phase 2

| ความเสี่ยง (จาก BRD) | Mitigation ที่ Implement                          |
| ------------------ | ----------------------------------------------- |
| AI พูดคำหยาบ         | Safety Filter 2 ชั้น (Sprint 2.3)                 |
| ผู้เล่นหลอก AI ให้ของ  | Economy Limiter + Whitelist (Sprint 2.3-2.4)    |
| AI Hallucination   | RAG + Source citation (Sprint 2.2)              |
| AI ทำให้ Pay-to-Win  | VIP = convenience only, ไม่ให้ power (Sprint 3.2) |

---

## Dependencies & Prerequisites

| Dependency            | Status   | Notes                        |
| --------------------- | -------- | ---------------------------- |
| Mac mini M4 Pro 64GB  | ✅ มีแล้ว   | เครื่อง Dev/Staging            |
| Ollama                | ต้องติดตั้ง  | `brew install ollama`        |
| Docker Desktop        | ต้องติดตั้ง  | สำหรับ Data Layer              |
| Rust toolchain        | ต้องติดตั้ง  | `rustup` + stable channel    |
| rAthena source code   | ต้องเตรียม | สำหรับ Phase 4 C++ integration |
| Qwen 2.5 32B Q4 model | ต้องเตรียม | ~20GB (ใช้ gemma:2b แทนบน m3) |
| Samsung T7 Shield 2TB | ✅ มีแล้ว   | สำหรับ Models + Data           |

---

## Timeline Summary

```
สัปดาห์  1-6   Phase 1: Infrastructure & Foundation (+RAG Pipeline)
สัปดาห์  7-13  Phase 2: NPC Chat + Oracle + Safety
สัปดาห์ 14-17  Phase 3: AI GM + Revenue Features
สัปดาห์ 18-21  Phase 4: Integration + Load Test + Beta
สัปดาห์ 22-25  Phase 5: Production + Cloud Migration

```

**รวม: ~25 สัปดาห์ (~6 เดือน)**
