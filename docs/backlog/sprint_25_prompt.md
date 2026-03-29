# Sprint 25 Prompt: New Capabilities

**Theme:** 🔵 เพิ่ม features ที่ทำให้ Agentic RAG pipeline สมบูรณ์
**Sprint Goal:** ผู้ใช้ทำ end-to-end RAG pipeline ได้ด้วย clicks น้อยที่สุด

---

## B-12: Auto-Pipeline (One-Click RAG Setup) [P1, Size: L]

### Concept
ปุ่ม "Full Pipeline" หลัง add source ที่ทำทุก step อัตโนมัติ:
```
Add Source → Sync → Chunk → Generate QA → Vector Index
```
ลด manual steps จาก 4 → 1

### Technical Design
- Backend: New endpoint `POST /api/v1/sources/:id/full-pipeline`
- Orchestrate: sync → wait complete → trigger QA → wait → trigger vector index
- Frontend: Progress modal showing pipeline stages
- WebSocket or polling for real-time status

### Files to Create/Modify
- `ro-ai-bridge/src/routes/sources/` — add `pipeline.rs` (or extend sync)
- `ro-ai-dashboard/src/app/sources/page.tsx` — "Full Pipeline" button
- `ro-ai-dashboard/src/components/pipeline-progress-modal.tsx` — new

### Acceptance Criteria
- [ ] One button triggers full pipeline
- [ ] Progress UI shows current stage
- [ ] Handles errors at each stage (retry or skip)
- [ ] Can be used on existing sources (re-process)

---

## B-13: Agent Evaluation from Playground [P2, Size: M]

### Concept
ปุ่ม "Evaluate" ใน Playground ที่ trigger evaluation run โดยตรง:
- ใช้ QA pairs จาก knowledge base เป็น test set
- แสดงผลลัพธ์ inline ใน Playground (accuracy, relevance, completeness)

### Files to Modify
- `ro-ai-dashboard/src/app/playground/page.tsx` — add Evaluate button + results panel
- `ro-ai-dashboard/src/lib/api.ts` — trigger evaluation API

### Acceptance Criteria
- [ ] "Evaluate" button visible when agent selected
- [ ] Shows evaluation progress
- [ ] Results displayed inline (scores + sample Q&A)
- [ ] Link to full evaluation page for details

---

## B-14: Coverage Gap Detection [P2, Size: M]

### Concept
เทียบ knowledge coverage กับ QA pairs และ evaluation results:
- Chunks ที่ยังไม่มี QA
- Topics ที่ agent ตอบไม่ถูก (low accuracy from eval)
- Sources without vector embeddings

### Files to Modify
- `ro-ai-bridge/src/routes/coverage.rs` — add gap detection endpoint
- `ro-ai-dashboard/src/app/coverage/page.tsx` — add gap analysis panel

### Acceptance Criteria
- [ ] Shows chunks without QA pairs
- [ ] Shows topics with low evaluation scores
- [ ] Actionable links (click → navigate to fix)

---

## B-15: One-Click Agent Publish [P3, Size: M]

### Concept
Publish agent → generate API key → copy embed snippet / MCP config

### Flow
```
Agent Studio → Publish → API Key Generated → Copy Embed Code
```

### Embed Options
- REST API endpoint + API key
- JavaScript widget snippet
- MCP server config JSON

### Files to Modify
- `ro-ai-dashboard/src/app/agents/page.tsx` — publish flow + embed modal
- `ro-ai-bridge/src/routes/agents/` — API key generation

### Acceptance Criteria
- [ ] Publish generates unique API key
- [ ] Copy-to-clipboard for API key
- [ ] Embed code snippet auto-generated
- [ ] API key can be revoked/regenerated

---

## B-16: Auto-Pipeline Graph KM & PageIndex Gaps [P1, Size: M]

### Concept
อุดช่องโหว่ (Gaps) ของ Auto-Pipeline ให้รองรับ Graph KM และ PageIndex เต็มรูปแบบ:
1. **Frontend LLM Selection:** เพิ่ม UI ให้เลือก LLM Provider และ Model ก่อนสั่ง Run Full Auto-Pipeline
2. **Backend PageIndex Integration:** นำคำสั่ง `generate_tree` (PageIndex) แทรกเป็นหนึ่งในสเต็ปการรันของ Auto-Pipeline ด้าน Backend
3. **Graph History Tracking:** แก้ไข `auto_pipeline.rs` ให้บันทึก State การสร้าง Knowledge Graph ลงตาราง `kg_extraction_runs` เพื่อให้ประวัติเชื่อมโยงไปแสดงที่หน้า Graph History (`/graph`) ให้ถูกต้อง

### Files to Modify
- `ro-ai-dashboard/src/app/sources/page.tsx` — เพิ่ม Modal/Selector เลือก LLM ก่อนรัน
- `ro-ai-bridge/src/routes/auto_pipeline.rs` — เพิ่ม Step PageIndex และ Insert log ลง `kg_extraction_runs`

### Acceptance Criteria
- [ ] ผู้ใช้ปรับเปลี่ยน LLM Model สำหรับ Auto-Pipeline ในหน้า UI ได้
- [ ] Auto-Pipeline เรียกใช้งาน PageIndex Tree extraction ด้วย 
- [ ] เมื่อรัน Auto-Pipeline เสร็จ ข้อมูลประวัติ Graph ภายในต้องไปโชว์ในหน้า Dashboard Graph History 

---

## B-17: Zitadel Login UI Customization (Yggdrasil Concept) [P2, Size: S]

### Concept
หน้าเว็บ Login ปัจจุบันของ Zitadel เป็นเพียงหน้าจอ Default สีขาวล้วนและมีโลโก้แพลตฟอร์มภายนอก ซึ่งไม่เข้ากับประสบการณ์ใช้งานโดยรวมของระบบ
ต้องการปรับเปลี่ยนหน้า Login Authentication (Yggdrasil) ให้สะท้อน Branding อัตลักษณ์ของ Mimir/Asgard:
- เปลี่ยน Logo เป็นของโครงการ และปรับ Theme อิงจาก Yggdrasil Concept (โครงสร้างรากไม้แห่งข้อมูล / โทนสีหลักของระบบ)
- ปรับแต่งสีพื้นหลัง (Background), โทนสีปุ่มกด (Primary/Secondary), และ Typography
- ทำให้รองรับการแสดงผลทั้งรูปแบบ Light Mode และ Dark Mode อย่างกลมกลืน

### Implementation Approach
- ตั้งค่าผ่าน Zitadel Console (`/ui/console`) → Settings → Branding 
- อัปโหลด Custom Assets (โลโก้, ไอคอน, รูปภาพพื้นหลังธีม Yggdrasil)
- กำหนด Color palette Code ให้ตรงกับ Design System ที่ใช้บน Dashboard
- (Optional) แทรก Custom CSS เพื่อเกลาเส้นสายและ Layout หาก Zitadel เปิดให้ override เพิ่มเติม

### Acceptance Criteria
- [ ] หน้า Auth Login ของระบบมีหน้าตาตรงตาม Yggdrasil concept
- [ ] โทนสีและโลโก้กลมกลืนเป็นเนื้อเดียวกับ Mimir Dashboard ทั้งก่อนและหลัง Login
- [ ] การแสดงผลสมบูรณ์ทั้งใน Light Mode และ Dark Mode

---

## ISO Documentation
- [ ] PM-02.25 Sprint Report
- [ ] SI-04.25 Test Script
- [ ] SI-03 Traceability Matrix update
