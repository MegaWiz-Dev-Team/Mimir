# 🏺 Project-Mimir
## Ragnarok Online: AI-Native Evolution

> **Project-Mimir (มิเมียร์):** ในตำนานนอร์ส Mimir คือเทพแห่งความรู้และสติปัญญา และเป็นผู้รักษาบ่อน้ำศักดิ์สิทธิ์ (Mímisbrunnr) โครงการนี้จึงมีเป้าหมายเพื่อสร้าง "บ่อน้ำแห่งความรู้" ให้กับโลกของ Ragnarok Online ผ่านระบบ AI Agentic Architecture

Project-Mimir คือระบบ AI Middleware ที่ออกแบบมาเพื่อยกระดับประสบการณ์ผู้เล่นใน Ragnarok Online โดยการเปลี่ยน NPC แบบดั้งเดิมให้มีความฉลาด ตอบโต้ได้คล่องแคล่ว และช่วยดูแลเซิร์ฟเวอร์แบบอัตโนมัติ

---

## 🚀 Key Modules

- **🏰 rAthena Integration:** ระบบ RO Server Emulator ที่ถูกปรับแต่งให้ทำงานร่วมกับ AI Agent แบบ End-to-End
- **🎭 Generative NPC:** การเปลี่ยน NPC ให้มีชีวิตด้วยบุคลิก (Persona) เฉพาะตัว ตอบคำถามและทำ Action ในเกมได้จริง
- **🔮 Oracle RAG Bot:** ระบบถาม-ตอบข้อมูลเกมอัจฉริยะ (Retrieval-Augmented Generation) ที่ดึงข้อมูลจาก Wiki และ Game Database
- **🤖 AI Game Master:** ผู้ดูแลเซิร์ฟเวอร์ 24/7 ทำหน้าที่ตรวจจับพฤติกรรมผิดปกติ และสร้าง Event อัตโนมัติ

## 🛠 Technology Stack

- **Server Emulator:** [rAthena](https://github.com/rathena/rathena) (C++)
- **AI Backend:** Rust 🦀 (Axum + [Rig.rs](https://rig.rs))
- **Frontend Admin:** Next.js (Tailwind CSS + Shadcn/UI)
- **Local LLM Server:** Ollama (Llama 3.2 / Nomic Embed)
- **Vector Database:** Qdrant
- **Primary Database:** MariaDB
- **Hardware Target:** Mac Air M3 / Mac mini M4 Pro

## 🏗 System Architecture

โครงการนี้ใช้ **Hybrid Agent Architecture** แบ่งการทำงานเป็น 3 ระดับ (Tiers):

1.  **Tier 1: Simple Agent (≤2s):** สำหรับ NPC Chat ทั่วไป เน้นความเร็ว
2.  **Tier 2: RAG Agent (≤5s):** สำหรับ Oracle ที่ต้องค้นหาข้อมูลจาก Qdrant
3.  **Tier 3: Background Agent:** สำหรับ AI GM วิเคราะห์พยาธิสภาพของเซิร์ฟเวอร์แบบ Real-time

## 📂 Project Structure

```
Project-Mimir/
├── rathena/                   # rAthena Server Emulator
├── ro-ai-bridge/             # Rust AI Middleware & Data Pipeline
├── ro-ai-dashboard/          # Next.js Management Web Interface
├── docs/                      # Technical Documents (BRD, TRD, Plans)
└── docker-compose.yml         # Full Stack Infrastructure
```

## 🧑‍💻 Get Started

### Infrastructure Control
- **Start All**: `docker compose up -d`
- **Stop All**: `docker compose stop`
- **Shutdown (Remove)**: `docker compose down`
- **Restart AI Backend**: `cd ro-ai-bridge && cargo run --bin monitor`

### 📊 สถานะ Infrastructure ปัจจุบัน
- **MariaDB** (Port 3306): รันอยู่, มีตาราง Monitor และ rAthena Schema แล้ว
- **Qdrant** (Port 6333): รันอยู่, มี collection `wiki_qa` แล้ว
- **rAthena**: รันอยู่ครบ 3 Server (Login, Char, Map) และเชื่อมต่อ Database สำเร็จ
- **Ollama** (Port 11434): รันอยู่บน Host (Native macOS)

### Start AI Ecosystem
1.  **Start AI Backend**:
    ```bash
    cd ro-ai-bridge
    cargo run --bin monitor
    ```

2.  **Start Dashboard**:
    ```bash
    cd ro-ai-dashboard
    npm run dev
    ```

---

## 🎮 Connecting to Game Server

Server Status: **Online** (Login: 6900, Char: 6121, Map: 5121)

### 1. Client Configuration (`data/clientinfo.xml`)
แก้ไขไฟล์ `clientinfo.xml` ในโฟลเดอร์ `data/` หรือ GRF ของตัวเข้าเกม:

```xml
<connection>
    <display>Project Mimir Local</display>
    <address>127.0.0.1</address>
    <port>6900</port>
    <version>46</version>
    <langtype>0</langtype>
</connection>
```

### 2. Test Accounts
- **ID:** `test` / **Pass:** `test`
- **Register:** เติม `_M` หรือ `_F` ท้าย ID เพื่อสมัครใหม่ (เช่น `user01_M`)

---
*Created with ❤️ for the Ragnarok Online community.*
