# Prompt สำหรับ Project Mimir: Benchmark v2 Implementation

*คุณสามารถ Copy & Paste ก้อน Prompt ในแต่ละหัวข้อไปสร้าง Session / Task ใหม่ทีละ Sprint ตามลำดับได้เลยครับ*

---

## 🚀 Sprint 1: Foundation & Telemetry Prompt
**(สำหรับเริ่ม Session Sprint 1)**

```markdown
# Context
เรากำลังพัฒนาระบบ Agentic RAG Benchmark ให้กับแพลตฟอร์ม Mimir โดยทำตามแผน "Benchmark v2" ใน Sprint ที่ 1 (Foundation & Telemetry) ซึ่งมีเป้าหมายหลักในการวางรากฐาน Schema สำหรับ Dataset Versioning และระบบติดตาม Token Usage

การเปลี่ยนแปลงจะต้องครอบคลุมทั้งส่วน Backend (Axum/SQLx) และ Frontend (Next.js Dashboard)

---

# Tasks to Implement
กรุณาทำตามรายการต่อไปนี้ทีละขั้นตอน

## 1. Backend Tasks
*   **T1.1 (Dataset Version Schema):** แก้ไข `src/routes/rag_eval_dataset.rs` เพิ่ม fields ในตาราง `rag_eval_datasets`: `version INT DEFAULT 1`, `difficulty VARCHAR(10)`, `question_type VARCHAR(20)` พร้อมอัปเดต Query ต่างๆ หากสร้าง dataset ชื่อซ้ำ ให้ auto-increment version 
*   **T1.2 (Extend EvalItem):** แก้ไข `RagEvalItem` ใน `rag_eval.rs` ให้มี field (Optional ทั้งหมด): `required_tools`, `required_routing`, `question_type`, `difficulty`
*   **T1.3 (Token Usage Extraction):** เข้าไปแก้ UniversalClient ใน `mimir-core-ai/src/services/llm_router.rs` ให้สามารถดึง token_usage จาก provider ต่างๆ ได้ (Prompt, Completion, Thinking)
*   **T1.4 (Store Token in Results):** แก้ `rag_eval.rs` ให้บันทึก `total_prompt_tokens`, `total_completion_tokens`, `total_thinking_tokens` สำหรับการรัน 1 รอบ (ในตาราง `rag_eval_runs`) และเก็บ breakdown ระดับ query (ใน struct)
*   **T1.5 (Latency Breakdown):** เพิ่มการวัด `ttft_ms` เข้าไปในผลลัพธ์ของ `RagEvalQueryResult` ระหว่างที่ทำการ streaming generation ของคำตอบ

## 2. Frontend Tasks (`ro-ai-dashboard`)
*   **T1.6 (Token Analytics):** เพิ่มตารางเปรียบเทียบในหน้า `/evaluations` เพื่อแสดงผล Prompt/Completion/Thinking tokens
*   **T1.7 (Latency Waterfall):** ในส่วนของ per-query drilldown เพิ่ม bar แสดง breakdown ของ latency (TTFT / Retrieval / Generation)
*   **T1.8 (Dataset Version Selector):** ปรับหน้า `/rag-playground` ส่วนของ Dropdown เลือก dataset ให้แสดง version กำกับ และทำให้ JSON Editor เป็น Read-only หากคลิกเลือก dataset ที่มีอยู่แล้ว พร้อมเพิ่มปุ่ม "Save as New Version"
*   **T1.9 (Ground Truth Judge):** ให้แก้ Backend ให้ส่ง `expected_content` เป็นตัวอ้างอิงให้ Judge LLM ตอนตรวจคำตอบ (ถ้ามี expected_content ให้ใช้เทียบ)
*   **T1.10 (Difficulty Badge):** เพิ่ม Difficulty badge ข้างๆ ผลลัพธ์แต่ละข้อใน UI

# Guidelines
ให้เขียน Code เปลี่ยนแปลง หรือ SQL Migration Scripts และบอกให้ฉันเอาผลลัพธ์ไปทดสอบ (อย่าลืมยึดรูปแบบ multi-tenant ที่เก็บ tenant_id ควบคู่ด้วย) ขอ Implementation Plan ก่อนเริ่มเขียนโค้ดด้วย
```

---

## 🚀 Sprint 2: Intelligence & Comparison Prompt
**(สำหรับเริ่ม Session Sprint 2 — สามารถรันต่อได้เมื่อ S1 จบแล้ว)**

```markdown
# Context
โครงการ Project Mimir (Benchmark v2) เดินทางมาถึง Sprint 2 (Intelligence & Comparison) เป้าหมายของ Sprint นี้คือ ทำให้กระบวนการ Auto-Tune แข็งแกร่งขึ้น (ปลอดภัย ไม่กินงบบานปลาย) และยกระดับขีดความสามารถการเปรียบเทียบผลลัพธ์ (Comparison) พร้อมระบบแจ้งเตือนก่ารตกฮวบของประสิทธิภาพ (Regression)

---

# Tasks to Implement

## 1. Auto-Tuning Safety (Backend)
*   **T2.1 (Budget Cap):** ใน `rag_eval_tuner.rs` เพิ่มการจำกัดงบ `max_token_budget: Option<u64>` ในคำขอและสร้าง accumulator เพื่อนับ token ใน tuning loop หากครบงบให้ตัดจบและบันทึกสถานะว่า `budget_exhausted`
*   **T2.2 (Triggers/Constraints):** เพิ่ม constraints ใน AutoTune Request (`min_accuracy`, `max_latency`, `max_tokens`) กรองไม่ให้ Tuner เลือกตัวที่ตกมาตรฐานมาเป็น "Best Run" 

## 2. Baseline & Regression (Backend)
*   **T2.3 (Baseline Pinning):** ปรับตาราง `rag_eval_runs` เพิ่ม `is_baseline BOOLEAN DEFAULT FALSE` และสร้าง API `POST /runs/:id/set-baseline` สำหรับล็อกผลลัพธ์
*   **T2.4 (Regression Detection):** เพิ่ม logic หลังจากการประเมินเสร็จสิ้น ให้ไปเช็คคะแนนเทียบกับผล Baseline หาก Hit Rate (หรือเป้าหมายหลัก) ลดลงเกิน 5% ให้แนบ flag `regression_detected` เป็น Response กลับมา
*   **T2.5 (Per-query Diff Endpoint):** สร้าง API `GET /runs/compare?ids=A,B` ไว้หาความต่างของสอง Run ให้ return ข้อที่มีการเปลี่ยนผลลัพธ์จากถูกเป็นผิด (Regressions) หรือจากผิดเป็นถูก (Improvements)

## 3. Dashboard Features (Frontend)
*   **T2.6 (Auto-Tune UI):** แก้ไข Modal Auto-Tune ให้รองรับการใส่ Budget Limit, Constraints, และคำนวณ Cost Estimation ให้ดูก่อนกดยืนยัน (Token ประมาณการ * รอบการทำงาน)
*   **T2.7 (Baseline Badge):** เพิ่มปุ่ม "Set as Baseline" และสัญลักษณ์ Baseline รวบถึงแจ้งเตือน ⚠️ "Regression Detected" หากเจอความผิดปกติ
*   **T2.8 (Per-query Diff UI):** สร้าง UI มารับ API เปรียบเทียบของ T2.5 แสดงเป็นตารางแยก Regression / Improvement ข้อต่อข้อ
*   **T2.9 (Trend Line Chart):** ใช้ตาราง metric snapshots ปัจจุบันเพื่อร่าง Area/Line Chart โดยใช้ Recharts สำหรับแสดงประสิทธิภาพย้อนหลัง

# Guidelines
ฉันต้องการ Implementation Plan ก่อนที่จะลงรหัสและคำสั่ง เริ่มจาก Backend Endpoint และทำงานร่วมกันใน Frontend Components
```

---

## 🚀 Sprint 3: Usability & Export Prompt
**(สำหรับเริ่ม Session Sprint 3)**

```markdown
# Context
ใน Sprint ที่ 3 สำหรับฟีเจอร์ Benchmark ของ Mimir, เน้นไปที่ประสบการณ์ผู้ใช้ (UX), การนำข้อมูลเข้าออกระบบ, และการยกระดับ Dataset Generator ให้สร้างคำถามได้หลากหลายรูปแบบ

---

# Tasks to Implement

## 1. Exporting & Dataset Tooling (Backend)
*   **T3.1 (Export API):** สร้าง endpoint ให้ดาวน์โหลดผลประเมิน `GET /runs/:id/export?format=csv` (พร้อม json parameter) return รายละเอียดระดับข้อออกไปเลย
*   **T3.2 (QA Gen Types):** เข้าปรับ Prompt ในหน้าที่ใช้ Generate Dataset ให้รองรับ parameter `question_types` เช่น `intra_chunk`, `cross_chunk`, `adversarial` แล้วคืนการตั้ง Tag ติดไปให้แต่ละข้อ
*   **T3.3 (QC Status):** สร้าง status ของข้อมูล dataset มี field ยอมให้ปรับสถานะเป็น "Approved" (เริ่มมาเป็น "Draft")
*   **T3.4 (Deduplication Check):** ในเส้นทางการเซพ Dataset สร้าง Semantic Similarity เช็คเพื่อป้องคำถามข้อเดียวกันและโยนแบบ Alert/Warning หากเช็คเจอ > 0.85 Jaccard หรือใดๆ

## 2. Workflow UI (Frontend)
*   **T3.5 (Export Buttons):** เพิ่มปุ่ม "Export CSV/JSON" ใน Drill down view
*   **T3.6 (Filter Toggles):** จัดพื้นที่ให้ปุ่มกดกรอง (Filter Toggle) เปลี่ยนไปมาใน Run Detail (Show All / Passed Only / Failed Only)
*   **T3.7 (Side-By-Side):** จัด Layout ใน Drill Down เป็น 2 ฝั่ง ซ้ายเป็น "Expected Answer" จาก Backend ส่วนของขวาคือ "Generated Answer" 
*   **T3.8 (Quick-Start Wizard):** สร้าง Modal Wizard ที่กระโดดเด้งมาตอนเข้ามาเปิดหน้า Evaluation ครั้งแรก โดยไล่ให้ 1. เลือกชุดข้อมูล 2. เลือก Preset และ 3. สั่งรันให้ง่ายๆ
*   **T3.9 (Presets / Tooltips):** เพิ่มปุ่ม Preset ให้เลือกจัดให้ตอนสั่งรัน (Balanced, Speed, Max Accuracy) ให้ค่าเปลี่ยนในฟอร์มอัตโนมัติ พร้อมไอคอนให้ Hover ดู Tooltips เป็นตัวช่วยอธิบายค่า

# Guidelines
ให้ส่ง Implementation Plan ให้ดูก่อน ค่อยทำงานจากหลังบ้านเชื่อมไปยังหน้า Next.js
```

---

## 🚀 Sprint 4: Agentic Evaluation Prompt
**(สำหรับเริ่ม Session Sprint 4)**

```markdown
# Context
Sprint สุดท้าย (Sprint 4) สำหรับการยกระดับ Project Mimir Benchmark คือการย้ายจากการให้คะแนน RAG Pipeline ทั่วๆ ไป ไปสู่การประเมิน "Agentic Trajectory" ของ Agent แบบ Multi-Step 

เราจะสร้างความสามารถให้ระบบจำแนก Topology และประเมินจำนวนนับขั้นตอนที่ควรจะเป็น และกรณีทดสอบรับมือข้อมูลล่อลวง 

---

# Tasks to Implement

## 1. Pipeline Extensions (Backend)
*   **T4.1 (Topology Setting):** สร้าง config Topology สำหรับการ Run (`rag_only`, `single_agent`, `agent_reflection`, `swarm`) เพื่อเตรียมจัดเส้นทางการวิ่งประเมินตามพฤติกรรมการทำงาน
*   **T4.2 (Routing Scoring):** หากวิ่งด้วย Topology ที่มี Agent ให้นับคะแนนการใช้เครื่องมือ (Tool Prediction) และความแม่นยำในการเลือก Agent ต่อไปเพื่อคำนวณ `tool_precision`
*   **T4.3 (Trajectory Storage):** ทำโครงสร้าง Table `eval_trajectories` เพื่อ Log ทุก Step ทั้ง actor, เหตุผล, เวลา, และ token แล้วเชื่อมกับ ID ของ Run
*   **T4.4 (Effort Ratio):** นำจำนวน Agent Step ที่ใช้จริงตั้ง หารด้วย จำนวนที่ Optimize ไว้ และเก็บค่าเป็น `effort_ratio` เป็น 1 ใน Metric
*   **T4.5 (Adversarial Data Type):** จัดการ Scoring Rule ใหม่ หากเจอ Tag ปัญหาล่อลวง ไม่ตอบหรือแจ้งเตือนถือเป็น "คะแนนเต็ม" ตอบถูกหลอกจะ "ผิด" 

## 2. Deep Visualization (Frontend)
*   **T4.6 (Topology Selectbox):** เพิ่มกล่องให้เลือก Topology ตอนสั่ง Run 
*   **T4.7 (Timeline Trace View):** ใน Drill Down หากเป็นข้อที่มี Trajectory ให้ขยายผล Waterfall ที่เป็นกระบวนคิดของ Agent ออกมาทีละขั้น

# Guidelines
เรากำลังลงลึกให้ Backend ไปต่อยอด Agent framework ที่คุณใช้ ให้คุณประเมินความซับซ้อนและเขียน Implementation Plan ที่จะครอบคลุมโครงสร้างทั้งหมดแล้วให้ฉันช่วย Review ก่อนลุยเขียน
```
