# 🗂️ Project Documentation Index

รวบรวมเอกสารทั้งหมดในโฟลเดอร์ `docs/` โดยเรียงลำดับตามเหตุการณ์ (Timeline) และวัตถุประสงค์ของเอกสาร เพื่อใช้ในการตรวจสอบย้อนหลังและทำความเข้าใจภาพรวมของโปรเจกต์

---

## 🏗️ 1. Core Foundation & Research (รากฐานและการวิจัย)
เอกสารกลุ่มนี้ถูกสร้างขึ้นในช่วงเริ่มต้นเพื่อกำหนดทิศทางและมาตรฐานของโปรเจกต์

| ไฟล์                                                                                                                                                           | วัตถุประสงค์                                                                                  | ลำดับเหตุการณ์                        |
| :------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------- | :-------------------------------- |
| [RO_Landverse_Research_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/RO_Landverse_Research_TH.md)                 | วิเคราะห์เจาะลึกระบบ Ragnarok Online Landverse (Stamina, Land, NFT) เพื่อนำมาจำลองเป็น Logic ใน AI | การวิจัยหลัก (Initial Research)      |
| [BRD_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/BRD_Project-Mimir_TH.md)                         | Business Requirement Document: เป้าหมายทางธุรกิจ, ปัญหาที่แก้ และรายได้                            | แผนงานระดับสูง (Business Goal)      |
| [TRD_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/TRD_Project-Mimir_TH.md)                         | Technical Requirement Document: สถาปัตยกรรม Hybrid Agent 3-Tier และ Tech Stack              | การออกแบบระบบ (High-level Design) |
| [Framework_Analysis_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Framework_Analysis_Project-Mimir.md) | เปรียบเทียบ Rust AI Frameworks                                                               | การตัดสินใจเลือก Tech Stack (Rig.rs) |
| [AI_Model_Selection.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/AI_Model_Selection.md)                             | กลยุทธ์การเลือก Model สำหรับ Embedding และ LLM (Local vs Cloud)                                 | การคัดเลือก AI Model                |

---

## 🎨 2. Detailed Infrastructure Design (การออกแบบเชิงลึก)
รายละเอียดการออกแบบ Database และระบบย่อยต่างๆ

| ไฟล์                                                                                                                                                                             | วัตถุประสงค์                                                                | ความสำคัญ                             |
| :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------- | :---------------------------------- |
| [Mimir_Landverse_Integration_Design.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Mimir_Landverse_Integration_Design.md)               | ออกแบบ Schema สำหรับจำลองระบบ Web3 (Land/Token) ใน rAthena                  | ข้อมูลอ้างอิงสำหรับ Database              |
| [Monitoring_System_Plan_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Monitoring_System_Plan_Project-Mimir.md)           | ออกแบบระบบ Monitor สำหรับ Wiki Q/A Pipeline                                | ระบบจัดการ Pipeline (Axum + MariaDB) |
| [Cloud_API_Fallback_Strategy_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Cloud_API_Fallback_Strategy_Project-Mimir.md) | แผนสำรองกรณี Local Ollama มีปัญหา (Gemini API)                               | ความเสถียรของระบบ (Reliability)      |
| [Test_Results_Design.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Test_Results_Design.md)                                             | รูปแบบการเก็บผลการทดสอบ Latency และ Accuracy                               | การวัดผล (Performance Benchmarking)  |
| [rAthena_Architecture_and_DB_Schema.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/rAthena_Architecture_and_DB_Schema.md)               | สรุปโครงสร้าง System Architecture 3-Tier และความสัมพันธ์ของ Database          | พื้นฐานระบบ (Architecture & DB)       |
| [rAthena_NPC_System.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/rAthena_NPC_System.md)                                               | อธิบายโครงสร้างภาษา Script, Variable และคำสั่งพื้นฐานสำหรับเขียน NPC               | คู่มือการ Dev (Scripting Guide)        |
| [AI_NPC_Integration_Design.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/AI_NPC_Integration_Design.md)                                 | วิเคราะห์แนวทางการเปลี่ยน NPC ธรรมดาให้เป็น AI Agent (Conversation & Decision) | แนวคิดระบบ (Concept Design)          |
| [rAthena_NPC_Types.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/rAthena_NPC_Types.md)                                                 | สรุปประเภทของ NPC (Script, Shop, Warp, Monster) และคุณสมบัติเด่น              | ข้อมูลอ้างอิง (Reference)               |
| [AI_Agent_Techniques_for_NPCs.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/AI_Agent_Techniques_for_NPCs.md)                           | รวมเทคนิคสำคัญในการสร้าง AI NPC (Persona, Structured Output, Memory, RAG)    | เทคนิคเชิงลึก (Advanced Techniques)    |

---

## 🚀 3. Implementation Plans (แผนการดำเนินงานของแต่ละ Phase)
แผนการทำงานจริงที่แบ่งเป็น Sprints และ Tasks

| ไฟล์                                                                                                                                                                         | สถานะ          | เนื้อหาหลัก                                            |
| :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------- | :-------------------------------------------------- |
| [Implementation_Plan_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Implementation_Plan_Project-Mimir.md)             | ร่างแผน 5 Phase | Road Map องค์รวมของโปรเจกต์                           |
| [Implementation_Plan_rAthena_Setup.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Implementation_Plan_rAthena_Setup.md)             | ✅ เสร็จสิ้น       | การติดตั้ง rAthena Server บน Docker                    |
| [Sprint_1.4_Cron_Integration_Plan.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Sprint_1.4_Cron_Integration_Plan.md)               | ⏸️ พักไว้         | ระบบ Cron สำหรับ Sync ข้อมูลจาก MCP                     |
| [Phase_1_Remaining_Tasks.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Phase_1_Remaining_Tasks.md)                                 | ✅ เสร็จสิ้น       | งานที่ต้องทำเพื่อปิด Phase 1 (AI Tables, Ingestion)        |
| [Implementation_Plan_AI_Tables_Ingestion.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Implementation_Plan_AI_Tables_Ingestion.md) | ✅ เสร็จสิ้น       | การสร้างตาราง AI และ Ingestion Pipeline              |
| [Implementation_Plan_Phase_1_Completion.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Implementation_Plan_Phase_1_Completion.md)   | ✅ เสร็จสิ้น       | การทำ Vector Indexing และ Hybrid Search              |
| [Implementation_Plan_Phase_2_Agent_Chat.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/Implementation_Plan_Phase_2_Agent_Chat.md)   | 🏗️ กำลังทำ         | ระบบ NPC Chat, Oracle Bot และ Play-to-Earn AI Logic |

---

*Last Updated: 2026-02-19*
