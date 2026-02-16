# 🏺 Project-Mimir
## Ragnarok Online: AI-Native Evolution

> **Project-Mimir (มิเมียร์):** ในตำนานนอร์ส Mimir คือเทพแห่งความรู้และสติปัญญา และเป็นผู้รักษาบ่อน้ำศักดิ์สิทธิ์ (Mímisbrunnr) โครงการนี้จึงมีเป้าหมายเพื่อสร้าง "บ่อน้ำแห่งความรู้" ให้กับโลกของ Ragnarok Online ผ่านระบบ AI Agentic Architecture

Project-Mimir คือระบบ AI Middleware ที่ออกแบบมาเพื่อยกระดับประสบการณ์ผู้เล่นใน Ragnarok Online โดยการเปลี่ยน NPC แบบดั้งเดิมให้มีความฉลาด ตอบโต้ได้คล่องแคล่ว และช่วยดูแลเซิร์ฟเวอร์แบบอัตโนมัติ

---

## 🚀 Key Modules

- **🏰 Generative NPC:** การเปลี่ยน NPC ให้มีชีวิตด้วยบุคลิก (Persona) เฉพาะตัว ตอบคำถามและทำ Action ในเกมได้จริง
- **🔮 Oracle RAG Bot:** ระบบถาม-ตอบข้อมูลเกมอัจฉริยะ (Retrieval-Augmented Generation) ที่ดึงข้อมูลจาก Database จริงของเซิร์ฟเวอร์
- **🤖 AI Game Master:** ผู้ดูแลเซิร์ฟเวอร์ 24/7 ทำหน้าที่ตรวจจับพฤติกรรมผิดปกติ (Bot Detection) และสร้าง Event อัตโนมัติ
- **💰 AI Economy:** ระบบหารายได้ผ่านฟีเจอร์ AI เช่น Smart Homunculus และ Fortune Teller โดยมีระบบควบคุมสมดุลเศรษฐกิจ (Hard Limits)

## 🛠 Technology Stack

- **Language:** Rust 🦀
- **Web Framework:** Axum
- **AI Agent Framework:** [Rig (rig.rs)](https://rig.rs)
- **Local LLM Server:** Ollama (Qwen 2.5 32B / Meditron)
- **Vector Database:** Qdrant (สำหรับ RAG)
- **Primary Database:** MariaDB
- **Caching/Session:** Redis
- **Hardware Target:** Mac mini M4 Pro (64GB RAM)

## 🏗 System Architecture

โครงการนี้ใช้ **Hybrid Agent Architecture** แบ่งการทำงานเป็น 3 ระดับ (Tiers):

1.  **Tier 1: Simple Agent (≤2s):** สำหรับ NPC Chat ทั่วไป เน้นความเร็ว
2.  **Tier 2: RAG Agent (≤5s):** สำหรับ Oracle และ Homunculus ที่ต้องค้นหาข้อมูลหรือเรียก Tools
3.  **Tier 3: Background Agent:** สำหรับ AI GM ที่ทำงานวิเคราะห์ข้อมูลเบื้องหลัง

## 📂 Project Structure

```
Project-Mimir/
├── docs/                      # เอกสารประกอบโครงการ (BRD, TRD, Plans)
│   ├── BRD_Project-Mimir_TH.md
│   ├── TRD_Project-Mimir_TH.md
│   ├── Implementation_Plan_Project-Mimir.md
│   └── Cloud_API_Fallback_Strategy_Project-Mimir.md
├── ro-ai-bridge/             # Rust Middleware (Coming Soon)
├── scripts/                   # Migration และ Ingestion scripts
└── docker-compose.yml         # Infrastructure setup
```

## 📖 Documentation

- [**Business Requirement Document (BRD)**](docs/BRD_Project-Mimir_TH.md)
- [**Technical Requirement Document (TRD)**](docs/TRD_Project-Mimir_TH.md)
- [**Implementation Plan**](docs/Implementation_Plan_Project-Mimir.md)
- [**Framework Analysis**](docs/Framework_Analysis_Project-Mimir.md)

---

## 🧑‍💻 Get Started

*โครงการอยู่ในช่วง Phase 1: Infrastructure & Foundation*

1.  Clone repository:
    ```bash
    git clone https://github.com/megacare-dev/Project-Mimir.git
    ```
2.  ศึกษา [Implementation Plan](docs/Implementation_Plan_Project-Mimir.md) เพื่อทำตามขั้นตอนการติดตั้ง

---
*Created with ❤️ for the Ragnarok Online community.*
