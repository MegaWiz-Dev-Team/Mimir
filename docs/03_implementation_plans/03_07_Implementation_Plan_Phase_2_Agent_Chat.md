# 🤖 Implementation Plan: Phase 2 — Core Features (NPC Chat + Oracle + Safety)

**เป้าหมาย:** สร้าง AI Agent Tier 1 & 2 พร้อม Safety Filter, Game Action Tools และ RAG Optimization ให้พร้อมใช้งาน

> [!IMPORTANT]
> Phase 2 ต้องการ Phase 1 Infrastructure ทั้งหมดพร้อมใช้งาน:
> - MariaDB (AI Tables + rAthena Schema)
> - Qdrant (`wiki_qa` + Game Data Collections)
> - Ollama (LLM + Embedding Model)

---

## Sprint 2.1 — Agent Chat + Playground (สัปดาห์ 6-8)

**เป้าหมาย:** สร้าง Agent ทั้ง 2 Tier และหน้าจอ Playground สำหรับทดสอบ

### Proposed Changes

#### [NEW] [simple_npc.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/agents/simple_npc.rs)
- Tier 1: Completion Agent ด้วย `rig-core`
- รับ System Prompt จาก Persona YAML config
- เน้นความเร็ว (≤ 2s), ไม่ใช้ RAG

#### [NEW] [oracle_rag.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/agents/oracle_rag.rs)
- Tier 2: RAG Agent ด้วย `rig-core` + Qdrant
- ดึงข้อมูลจาก `wiki_qa` + Game Data collections
- Implement Custom Tools: `QueryMobDbTool`, `QueryItemDbTool` (ดึงข้อมูลจาก rAthena DB ตรง)
- สร้าง Confidence Score + Source Citation ใน Response

#### [MODIFY] [monitor.rs](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/src/bin/monitor.rs)
- `POST /api/agents/chat`: Chat endpoint รองรับทั้ง Tier 1 & 2
  - Payload: `{ "tier": 1|2, "message": "...", "persona": "sage_ariel" }`
- **Response Streaming**: ใช้ SSE (Server-Sent Events) ส่ง token ทีละตัว

#### [NEW] [config/personas/](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-bridge/config/personas/)
- Persona YAML configs สำหรับ NPC แต่ละตัว:
  - `sage_ariel.yaml` — นักปราชญ์ ชอบอธิบายละเอียด
  - `fortune_teller.yaml` — หมอดู ชอบพูดลึกลับ
  - `blacksmith.yaml` — ช่างเหล็ก พูดตรงๆ
- Format: `name`, `system_prompt`, `tier`, `allowed_actions[]`, `greeting`

#### [NEW] [playground/page.tsx](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/ro-ai-dashboard/src/app/playground/page.tsx)
- Interactive Chat UI พร้อม:
  - Agent Tier selector (Tier 1 / Tier 2)
  - Persona dropdown
  - Streaming response display
  - Source citation panel (สำหรับ Tier 2)

### Checklist 2.1
- [ ] Implement Tier 1 Agent (`simple_npc.rs`)
- [ ] Implement Tier 2 Agent (`oracle_rag.rs`)
- [ ] Update Monitor Service SSE Streaming
- [ ] Create Persona YAML files
- [ ] Build Playground UI

---

## Sprint 2.2 — Safety Filter + Cloud Fallback (สัปดาห์ 8-10)

**เป้าหมาย:** ป้องกัน Prompt Injection และเตรียม Cloud Fallback

#### [NEW] `src/middleware/safety_filter.rs`
- **Pre-Filter**: คำหยาบ, URL/Phone pattern, Toxicity score
- **Post-Filter**: ตรวจ Action validity, ความยาว, หลุดบทบาท

#### [NEW] `src/services/provider_chain.rs`
- Provider Chain: Local (Primary) → Cloud (Fallback)
- Auto-switch เมื่อ Ollama ล่มหรือตอบช้าเกิน threshold

#### [NEW] `src/services/privacy_guard.rs`
- Scrub PII (Player ID, IP, ชื่อจริง) ก่อนส่งไป Cloud API

#### [NEW] `src/services/cost_tracker.rs` + `config/cloud_limits.yaml`
- Daily spending limit ($5/day) + Kill switch
- Track usage per provider per day

### Checklist 2.2
- [ ] Implement `safety_filter.rs`
- [ ] Implement `provider_chain.rs` (Local + Cloud)
- [ ] Implement `privacy_guard.rs` (PII Scrubbing)
- [ ] Implement `cost_tracker.rs` with $5 limit
- [ ] Red-team testing (Injection Patterns)

---

## Sprint 2.3 — Game Action Tools (สัปดาห์ 10-12)

**เป้าหมาย:** ให้ AI NPC ทำ Action จริงในเกมได้ โดยมี Limit ควบคุม

#### [NEW] `src/tools/heal_tool.rs`
- Rig Tool trait + Daily limit check + Audit log (`ai_action_log`)

#### [NEW] `src/tools/buff_tool.rs`
- ระยะเวลา buff สูงสุด 5 นาที

#### [NEW] `src/tools/give_item_tool.rs`
- Whitelist items per NPC, block MVP cards/rare items

#### [NEW] `src/services/economy_limiter.rs`
- ตรวจสอบ `ai_economy_daily` + `ai_player_daily_limits` ก่อนทำ Action
- Integrate Tools เข้ากับ Agent ทั้ง Tier 1 และ Tier 2

### Checklist 2.3
- [ ] Implement `heal_tool.rs`
- [ ] Implement `buff_tool.rs`
- [ ] Implement `give_item_tool.rs`
- [ ] Implement `economy_limiter.rs`
- [ ] Integrate Tools with Agents

---

## Sprint 2.4 — Advanced RAG Optimization (สัปดาห์ 12-13)

> [!NOTE]
> Sprint นี้อาจเริ่มบางส่วนตั้งแต่ Phase 1 หากมีเวลา (Hybrid Search Schema พร้อมแล้ว)

#### [MODIFY] `src/services/qdrant.rs`
- **Hybrid Search**: ส่ง Dense + Sparse Vector พร้อมกัน
- Score Formula: `Score = (Dense * 0.7) + (Sparse * 0.3)`

#### [NEW] `src/services/reranker.rs`
- Setup `bge-reranker-v2-m3` บน Ollama
- รับ Candidates → Rerank → Return Top-K
- Integrate เข้ากับ Oracle Agent (Tier 2)

### Checklist 2.4
- [ ] Optimize `qdrant.rs` for Hybrid Search
- [ ] Setup Reranker on Ollama
- [ ] Integrate Reranker with Oracle Agent

---

## 🚦 Phase 2 Gate Criteria

> **ต้องผ่านก่อนไป Phase 3:**
- [ ] **NPC Chat Latency**: ตอบ ≤ 2 วินาที (P95)
- [ ] **Oracle Accuracy**: ตอบถูกต้อง > 85% จาก test set
- [ ] **Safety Filter**: บล็อก Prompt Injection ได้ > 95%
- [ ] **Economy Logic**: ทำงานถูกต้อง 100%
- [ ] **Audit Trail**: บันทึก Action ครบถ้วน

> [!WARNING]
> **Latency Risk**: ปัจจุบัน Ollama บน M3 ยังตอบ ~8s ต้องทดลอง Model ที่เล็กลง (เช่น Llama 3.2-1B/3B) หรือใช้ Speculative Decoding เพื่อให้ผ่าน Gate ข้อแรก

---

## Verification Plan

### Automated Tests
- Unit tests: `SimpleAgent` system prompt injection
- Mocked RAG tests: `OracleAgent` retrieval accuracy
- Safety Filter: 20+ Prompt Injection patterns
- Economy Limiter: ทดสอบ daily limits

### Manual Verification
- Dashboard Playground: Chat กับ NPC Tier 1 ทดสอบ Persona
- Dashboard Playground: ถามข้อมูลเกมผ่าน Tier 2 ตรวจ Source Citation
- Fallback Test: `docker stop ollama` → ตรวจว่าสลับไป Cloud ภายใน 0.5s
- Economy Test: สั่ง action จนถึง daily limit → ตรวจว่า block ถูกต้อง
