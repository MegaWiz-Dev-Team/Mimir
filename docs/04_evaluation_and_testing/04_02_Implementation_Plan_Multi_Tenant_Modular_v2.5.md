# Benchmark v2: Cross-Reference Analysis 🔄
เทียบ **เอกสารเดิม** (`04_01`, `04_02`) กับ **Requirements ใหม่** (`benchmark_v2_requirements.md`)

---

## ✅ กลุ่ม A: สิ่งที่ทำแล้วและยังใช้ได้ — นำมาต่อยอด

| Feature | เอกสารเดิม (04_01) | สถานะปัจจุบัน | ความเห็น |
|---------|-------------------|---------------|----------|
| **Agent × Model Matrix** | Phase 2: Evaluator CLI วิ่ง Combo `(agent, model) × question` | ✅ `run_eval.rs` มีแล้ว | โครงสร้างยังดี ใช้เป็นฐานสำหรับ Single Agent Eval ได้เลย |
| **LLM-as-Judge** | Phase 2: Gemini ให้คะแนน 4 มิติ (Accuracy, Completeness, Relevance, Speed) | ✅ `rag_eval.rs` มี judge_model/judge_provider | **ต่อยอดได้:** เพิ่มมิติ Tool Precision, Route Accuracy, Hallucination ตาม Req §2.3 |
| **Human Review Override** | Phase 3: PATCH `/api/eval/scores/:id/review` + human_*_score columns | ✅ Schema มีแล้ว | ใช้ได้ตรง ๆ กับ §1 Dataset QC Loop (Human Review Queue) |
| **eval_runs + eval_scores Tables** | Phase 1: Schema ครบ run → scores → summary | ✅ ทั้ง `eval_runs` + `rag_eval_runs` มีอยู่ | ต้องเพิ่ม Column สำหรับ Token Usage (in/out/thinking) ตาม §2.2 |
| **Heatmap Dashboard** | Phase 4: Agent×Model heatmap + Detail Table | ✅ `/evaluations` page มี | ใช้เป็นฐานสำหรับ Comparison Dashboard ตาม §5 |
| **Multi-Tenant Isolation** | 04_02: `tenant_id` ทุก Table + Qdrant filter | ✅ เสร็จสมบูรณ์ (Sprint 1-2) | Benchmark v2 ต้อง scope ภายใต้ tenant — โครงสร้างพร้อมแล้ว |

---

## 🔶 กลุ่ม B: สิ่งที่ทำไว้แล้วแต่ต้องอัปเกรด

### B1. Rubric Scoring: 4 มิติ → 6+ มิติ
- **เดิม (04_01):** `accuracy_score`, `completeness_score`, `relevance_score`, `latency_ms`
- **ใหม่ต้องการ (§2.3 + §6):** เพิ่ม `tool_precision_score`, `route_accuracy_score`, `hallucination_penalty`, `effort_ratio`
- **Action:** เพิ่ม Column ใน `eval_scores` หรือเปลี่ยนเป็น JSON column `rubric_scores` แบบยืดหยุ่น

### B2. QA Dataset: Static JSON → Versioned + Typed
- **เดิม (04_01):** ไฟล์ `qa_dataset.json` วางใน `data/` — ไม่มี Version, ไม่มี Type
- **ใหม่ต้องการ (§1):** Immutable versioned datasets + 6 ชนิดโจทย์ (Single-turn, Multi-turn, Intra-chunk, Cross-chunk, Cross-source, Ground Truth)
- **Action:** Migrate จาก file-based เป็น DB-based (`eval_datasets` table) + เพิ่ม `type` field + `version` field + `difficulty` tag

### B3. Evaluator CLI → API-driven + Background Job
- **เดิม (04_01):** `run_eval.rs` เป็น standalone CLI binary ที่ต้องรันจาก terminal
- **ใหม่ต้องการ (§4, §7):** ต้อง trigger ได้จาก Dashboard (HTTP API), รันเป็น background job, มี progress tracking, มี Scheduled/CI mode
- **Action:** ยกระดับเป็น API endpoint (ซึ่ง `rag_eval.rs` ทำไปแล้วครึ่งหนึ่ง) — ต้องเพิ่ม scheduling + webhook notification

### B4. Latency Tracking: Single Number → Waterfall Breakdown
- **เดิม (04_01):** เก็บแค่ `latency_ms` ค่าเดียวต่อ question
- **ใหม่ต้องการ (§2.1):** แยก TTFT, Tool Execution Time, Agent E2E Latency
- **Action:** เปลี่ยน `latency_ms INT` เป็น `latency_breakdown JSON` ที่เก็บ `{"ttft": 400, "tool_exec": 800, "total": 1200}`

### B5. Agent Architecture: Fixed List → Topology Selection
- **เดิม (04_01):** ฮาร์ดโค้ด Agent list เป็น `["simple_npc", "oracle_rag"]`
- **ใหม่ต้องการ (§3):** ผู้ใช้เลือก Topology ได้ (Single Agent, Agent+Reflection, Swarm Multi-Agent)
- **Action:** เปลี่ยนจาก hardcoded list เป็น config-driven agent topology ที่ frontend ส่งมา

---

## 🔴 กลุ่ม C: สิ่งที่ยังไม่มีเลย — ต้องสร้างใหม่

| Requirement (§) | รายละเอียด | ความซับซ้อน |
|-----------------|------------|-------------|
| **§2.2 Token Economics (IN/OUT/Thinking)** | ไม่มีการเก็บ Token usage ใดๆ ทั้งในเอกสารเดิมและ Codebase | 🟡 กลาง — ต้องแก้ `UniversalClient` ให้ extract usage metadata จาก provider response |
| **§4 Auto-Tuning Budget Safety** | มี `rag_eval_tuner.rs` แต่ไม่มี token budget cap, early stopping, cost preview | 🟡 กลาง — เพิ่ม logic ใน tuner loop |
| **§7 Export & Sharing** | ไม่มีปุ่ม Export หรือ Shareable Link ใดๆ | 🟡 กลาง — Frontend + API endpoint ใหม่ |
| **§8 Per-query Drill-down & Source Audit** | ไม่มี Side-by-side view, ไม่มี Source Attribution check | 🔴 หนัก — ต้องเก็บ retrieved chunks per query + สร้าง UI ใหม่ |
| **§9 Regression Detection & Baseline** | ไม่มี concept ของ "Baseline Run" หรือ trend graph | 🟡 กลาง — เพิ่ม `is_baseline` flag + comparison logic |
| **§10 Onboarding Wizard & Presets** | ไม่มี Wizard หรือ Preset templates | 🟢 เบา — Pure frontend |
| **§6.1 Error Recovery Testing** | ไม่มี fault injection ใดๆ | 🔴 หนัก — ต้องสร้าง mock layer สำหรับ Tool failure simulation |
| **§6.3 Adversarial Robustness** | ไม่มี adversarial test set | 🟡 กลาง — สร้าง dataset ชนิดใหม่ + ปรับ scoring rubric |
| **§5 Query-level Comparison Diff** | มี Compare ระดับ Run แต่ไม่มีระดับข้อ | 🟡 กลาง — Backend query + Frontend diff table |

---

## 🎯 Reconciliation Actions: สิ่งที่ต้องทำเพื่อรวม 04_01 เข้ากับ Benchmark v2

### ขั้นที่ 1: Schema Evolution (ปรับฐานข้อมูล)
```
eval_scores: เพิ่ม token_usage JSON, latency_breakdown JSON, rubric_scores JSON
eval_datasets: สร้างตารางใหม่ (name, version, type, difficulty, eval_set JSON, created_at)
rag_eval_runs: เพิ่ม is_baseline BOOLEAN, dataset_version VARCHAR
```

### ขั้นที่ 2: Backend Telemetry (เก็บข้อมูล Token)
```
UniversalClient: extract prompt_tokens, completion_tokens, reasoning_tokens จาก API response
Trace Payload: บังคับ format {"tokens": {...}, "latency": {...}, "evaluation": {...}}
```

### ขั้นที่ 3: Dashboard Upgrade (ปรับหน้าเว็บ)
```
Tab: Manual / Auto-Tuning mode switcher
Table: Per-query drill-down with pass/fail badges
Modal: Side-by-side Ground Truth vs Agent Answer
Chart: Trend Line (Accuracy over time), Radar (multi-metric compare)
Button: Export CSV/PDF, Set as Baseline, Compare Selected
```

### ขั้นที่ 4: Safety & UX
```
Auto-Tuning: token budget cap, early stopping, cost estimation preview
Onboarding: Quick Start wizard, Preset templates, Inline tooltips
```
