# 🗂️ Project Documentation Index

รวบรวมเอกสารทั้งหมดในโฟลเดอร์ `docs/` โดยเรียงลำดับตามเหตุการณ์ (Timeline) และวัตถุประสงค์ของเอกสาร เพื่อใช้ในการตรวจสอบย้อนหลังและทำความเข้าใจภาพรวมของโปรเจกต์

---

## 🏗️ 1. Core Foundation & Research (รากฐานและการวิจัย)
เอกสารกลุ่มนี้ถูกสร้างขึ้นในช่วงเริ่มต้นเพื่อกำหนดทิศทางและมาตรฐานของโปรเจกต์

| ไฟล์                                                                                                                                                                       | วัตถุประสงค์                                                                                  | ลำดับเหตุการณ์                        |
| :------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------- | :-------------------------------- |
| [01_01_RO_Landverse_Research_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_01_RO_Landverse_Research_TH.md)                 | วิเคราะห์เจาะลึกระบบ Ragnarok Online Landverse (Stamina, Land, NFT) เพื่อนำมาจำลองเป็น Logic ใน AI | การวิจัยหลัก (Initial Research)      |
| [01_02_BRD_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_02_BRD_Project-Mimir_TH.md)                         | Business Requirement Document: เป้าหมายทางธุรกิจ, ปัญหาที่แก้ และรายได้                            | แผนงานระดับสูง (Business Goal)      |
| [01_03_TRD_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_03_TRD_Project-Mimir_TH.md)                         | Technical Requirement Document: สถาปัตยกรรม Hybrid Agent 3-Tier และ Tech Stack              | การออกแบบระบบ (High-level Design) |
| [01_04_Framework_Analysis_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_04_Framework_Analysis_Project-Mimir.md) | เปรียบเทียบ Rust AI Frameworks                                                               | การตัดสินใจเลือก Tech Stack (Rig.rs) |
| [01_05_AI_Model_Selection.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_05_AI_Model_Selection.md)                             | กลยุทธ์การเลือก Model สำหรับ Embedding และ LLM (Local vs Cloud)                                 | การคัดเลือก AI Model                |
| [01_06_Gap_Analysis_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_06_Gap_Analysis_Project-Mimir_TH.md)       | วิเคราะห์ช่องว่างระหว่างโปรเจกต์กับ Business Model ใหม่                                            | การวิเคราะห์กลยุทธ์ (Gap Analysis)    |
| [01_07_BRD_Expansion_Web3_SaaS_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_07_BRD_Expansion_Web3_SaaS_TH.md)             | ส่วนขยาย BRD: รายละเอียด Module E (Web3) และ Module F (SaaS)                                 | การขยายขอบเขต (BRD Expansion)     |
| [01_08_Security_Evolution_Anti-Fraud_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_08_Security_Evolution_Anti-Fraud_TH.md) | พัฒนาระบบ Anti-Fraud ด้วยแนวคิด "Offensive-Informed Defense"                                  | วิวัฒนาการความปลอดภัย (Security)     |
| [01_09_Product_Roadmap_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_09_Product_Roadmap_Project-Mimir_TH.md) | แผนงานพัฒนาผลิตภัณฑ์ (Product Roadmap) ระยะยาว Phase 1 - 4+                                    | การวางแผนกลยุทธ์ (Roadmap)          |
| [01_10_Sales_Deck_Maxion_First_Call_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_10_Sales_Deck_Maxion_First_Call_TH.md)   | เนื้อหา Slide สำหรับนำเสนอ Maxion (First Call)                                                  | ข้อมูลการนำเสนอ (Sales Deck)         |
| [01_11_Gemini_Models_Overview.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_11_Gemini_Models_Overview.md)                     | ข้อมูลโมเดลในตระกูล Gemini รุ่นต่างๆ (Cloud API)                                                 | ข้อมูลอ้างอิง AI Model (Cloud)        |

---

## 🎨 2. Detailed Infrastructure Design (การออกแบบเชิงลึก)
รายละเอียดการออกแบบ Database และระบบย่อยต่างๆ

| ไฟล์                                                                                                                                                                                         | วัตถุประสงค์                                                                         | ความสำคัญ                             |
| :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :-------------------------------------------------------------------------------- | :---------------------------------- |
| [02_01_Mimir_Landverse_Integration_Design.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_01_Mimir_Landverse_Integration_Design.md)               | ออกแบบ Schema สำหรับจำลองระบบ Web3 (Land/Token) ใน rAthena                           | ข้อมูลอ้างอิงสำหรับ Database              |
| [02_02_Monitoring_System_Plan_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_02_Monitoring_System_Plan_Project-Mimir.md)           | ออกแบบระบบ Monitor สำหรับ Wiki Q/A Pipeline                                         | ระบบจัดการ Pipeline (Axum + MariaDB) |
| [02_03_Cloud_API_Fallback_Strategy_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_03_Cloud_API_Fallback_Strategy_Project-Mimir.md) | แผนสำรองกรณี Local Ollama มีปัญหา (Gemini API)                                        | ความเสถียรของระบบ (Reliability)      |
| [02_04_Test_Results_Design.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_04_Test_Results_Design.md)                                             | รูปแบบการเก็บผลการทดสอบ Latency และ Accuracy                                        | การวัดผล (Performance Benchmarking)  |
| [02_05_rAthena_Architecture_and_DB_Schema.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_05_rAthena_Architecture_and_DB_Schema.md)               | สรุปโครงสร้าง System Architecture 3-Tier และความสัมพันธ์ของ Database                   | พื้นฐานระบบ (Architecture & DB)       |
| [02_06_rAthena_NPC_System.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_06_rAthena_NPC_System.md)                                               | อธิบายโครงสร้างภาษา Script, Variable และคำสั่งพื้นฐานสำหรับเขียน NPC                        | คู่มือการ Dev (Scripting Guide)        |
| [02_07_AI_NPC_Integration_Design.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_07_AI_NPC_Integration_Design.md)                                 | วิเคราะห์แนวทางการเปลี่ยน NPC ธรรมดาให้เป็น AI Agent (Conversation & Decision)          | แนวคิดระบบ (Concept Design)          |
| [02_08_rAthena_NPC_Types.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_08_rAthena_NPC_Types.md)                                                 | สรุปประเภทของ NPC (Script, Shop, Warp, Monster) และคุณสมบัติเด่น                       | ข้อมูลอ้างอิง (Reference)               |
| [02_09_AI_Agent_Techniques_for_NPCs.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_09_AI_Agent_Techniques_for_NPCs.md)                           | รวมเทคนิคสำคัญในการสร้าง AI NPC (Persona, Structured Output, Memory, RAG)             | เทคนิคเชิงลึก (Advanced Techniques)    |
| [02_10_rAthena_Client_AI_Communication.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/02_10_rAthena_Client_AI_Communication.md)                     | วิธีการสื่อสารระหว่าง Game Client, rAthena และ AI Agent (SQL Bridge, HTTP, Web Server) | การเชื่อมต่อระบบ (Integration Methods) |

---

## 🚀 3. Implementation Plans (แผนการดำเนินงานของแต่ละ Phase)
แผนการทำงานจริงที่แบ่งเป็น Sprints และ Tasks

| ไฟล์                                                                                                                                                                                           | สถานะ          | เนื้อหาหลัก                                            |
| :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------- | :-------------------------------------------------- |
| [03_01_Implementation_Plan_Project-Mimir.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_01_Implementation_Plan_Project-Mimir.md)                   | ร่างแผน 5 Phase | Road Map องค์รวมของโปรเจกต์                           |
| [03_02_Implementation_Plan_rAthena_Setup.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_02_Implementation_Plan_rAthena_Setup.md)                   | ✅ เสร็จสิ้น       | การติดตั้ง rAthena Server บน Docker                    |
| [03_03_Sprint_1.4_Cron_Integration_Plan.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_03_Sprint_1.4_Cron_Integration_Plan.md)                     | ⏸️ พักไว้         | ระบบ Cron สำหรับ Sync ข้อมูลจาก MCP                     |
| [03_04_Phase_1_Remaining_Tasks.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_04_Phase_1_Remaining_Tasks.md)                                       | ✅ เสร็จสิ้น       | งานที่ต้องทำเพื่อปิด Phase 1 (AI Tables, Ingestion)        |
| [03_05_Implementation_Plan_AI_Tables_Ingestion.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_05_Implementation_Plan_AI_Tables_Ingestion.md)       | ✅ เสร็จสิ้น       | การสร้างตาราง AI และ Ingestion Pipeline              |
| [03_06_Implementation_Plan_Phase_1_Completion.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_06_Implementation_Plan_Phase_1_Completion.md)         | ✅ เสร็จสิ้น       | การทำ Vector Indexing และ Hybrid Search              |
| [03_07_Implementation_Plan_Phase_2_Agent_Chat.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_07_Implementation_Plan_Phase_2_Agent_Chat.md)         | 🏗️ กำลังทำ         | ระบบ NPC Chat, Oracle Bot และ Play-to-Earn AI Logic |
| [03_08_Implementation_Plan_Model_Configuration_DB.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/03_08_Implementation_Plan_Model_Configuration_DB.md) | 🏗️ กำลังทำ         | การย้าย Model Configuration ไปเก็บใน MariaDB          |

---

*Last Updated: 2026-02-19*
