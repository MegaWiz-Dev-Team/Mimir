# 📋 สรุปงานที่เหลือ: Phase 1 Completion

เอกสารนี้สรุปงานที่ยังค้างอยู่ใน Phase 1 เพื่อให้กลับมาเริ่มต่อได้ทันทีในวันพรุ่งนี้

---

## 🏗️ 1. Database & Infrastructure (AI Layer)
**เป้าหมาย:** สร้างพื้นฐานข้อมูลสำหรับระบบ Agentic AI

- [ ] **AI Tables Migration**: เขียน SQL สำหรับสร้างตาราง AI ใน MariaDB (ห้ามแตะ rAthena tables)
    - `ai_npc_persona`: เก็บค่า Setting และบุคลิกของ NPC
    - `ai_chat_session`: เก็บประวัติการคุยเพื่อให้ AI มีความจำ (Memory)
    - `ai_action_log`: บันทึกการกระทำของ AI (Heal, Buff, Warp)
    - `ai_economy_daily`: คุมมวลรวมเงินและไอเทมที่ AI จ่ายออก
    - `ai_player_daily_limits`: คุมลิมิตรายคน
- [ ] **SQLx Models**: สร้าง Struct และ Function ใน Rust (ro-ai-bridge) เพื่อเชื่อมต่อกับตารางเหล่านี้

## 🔮 2. Vector Data & Game Ingestion
**เป้าหมาย:** ทำให้ AI "รู้" ข้อมูลจริงจาก Server

- [ ] **Game Data Indexer**: เขียน Script ดึงข้อมูลจาก rAthena DB -> Embedding -> Qdrant
    - Collection: `ro_items`, `ro_monsters`, `ro_skills`, `ro_quests`
- [ ] **Hybrid Search Logic**: ปรับปรุงโค้ด Search ใน Rust ให้ส่งทั้ง Dense และ Sparse Vector (BM25) เพื่อความแม่นยำในการค้นหาชื่อไอเทม/มอนสเตอร์

## 🚀 3. Performance & RAG Hardening
**เป้าหมาย:** ทำให้ AI ฉลาดและตอบไว

- [ ] **Reranker Service**: เพิ่ม `bge-reranker-v2-m3` เข้าไปใน Pipeline เพื่อคัดกรองคำตอบที่ดีที่สุด
- [ ] **Latency Benchmark**: ทดลองปรับ Model (เช่น Llama3.2-1B หรือ 3B-INT8) เพื่อพยายามให้ได้ความเร็วใกล้เคียง 1.8s บนเครื่อง M3

---

## 🛠️ สรุปสถานะ Infrastructure ปัจจุบัน (สำหรับเริ่มงาน)
- **MariaDB**: รันอยู่ (Port 3306), มีตาราง Monitor แล้ว
- **Qdrant**: รันอยู่ (Port 6333), มี collection `wiki_qa` แล้ว
- **rAthena**: รันอยู่ครบ 3 Server (Login, Char, Map)
- **Ollama**: รันอยู่ (Port 11434)

**Next Action:** เริ่มต้นด้วยการเขียน Migration สำหรับ **AI Tables** ครับ!
