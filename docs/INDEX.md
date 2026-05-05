# 🗂️ Project Documentation Index (Version 2.3)

รวบรวมเอกสารทั้งหมดในโฟลเดอร์ `docs/` โดยได้ทำการจัดหมวดหมู่ใหม่เพื่อให้ง่ายต่อการค้นหาและอ้างอิง ระบบปัจจุบันถูกพัฒนาเสร็จสิ้นใน **Phase 1 (Version 2.3)** และพร้อมสำหรับการพัฒนาต่อยอดใน Phase ถัดไป

---

## 🏗️ 01. Requirements & Design (ความต้องการและการออกแบบ)
เอกสารกลุ่มนี้กำหนดทิศทาง มาตรฐาน ความต้องการทางธุรกิจและเทคนิคของโปรเจกต์
- `docs/01_requirements_and_design/01_01_RO_Landverse_Research_TH.md`: วิเคราะห์ระบบ Ragnarok Online Landverse
- `docs/01_requirements_and_design/01_02_BRD_Project-Mimir_TH.md`: Business Requirement Document
- `docs/01_requirements_and_design/01_03_TRD_Project-Mimir_TH_v2.3.md`: Technical Requirement Document v2.3
- `docs/01_requirements_and_design/01_03_TRD_Project-Mimir_TH_v2.2.md`: Technical Requirement Document v2.2 (Legacy)
- `docs/01_requirements_and_design/01_03_TRD_Project-Mimir_TH_v2.1.md`: Technical Requirement Document v2.1
- `docs/01_requirements_and_design/01_03_TRD_Project-Mimir_TH.md`: TRD (Legacy)
- `docs/01_requirements_and_design/01_04_Framework_Analysis_Project-Mimir.md`: วิเคราะห์ AI Framework
- `docs/01_requirements_and_design/01_05_AI_Model_Selection.md`: กลยุทธ์การเลือก Model
- `docs/01_requirements_and_design/01_06_Gap_Analysis_Project-Mimir_TH.md`: วิเคราะห์ช่องว่าง (Gap Analysis)
- `docs/01_requirements_and_design/01_07_BRD_Expansion_Web3_SaaS_TH.md`: ขยายขอบเขต BRD สู่ Web3/SaaS
- `docs/01_requirements_and_design/01_08_Security_Evolution_Anti-Fraud_TH.md`: พัฒนาระบบ Anti-Fraud
- `docs/01_requirements_and_design/01_09_Product_Roadmap_Project-Mimir_TH.md`: แผนก้าวหน้า (Product Roadmap)
- `docs/01_requirements_and_design/01_10_Sales_Deck_Maxion_First_Call_TH.md`: Sales Deck สำหรับ Maxion
- `docs/01_requirements_and_design/01_11_Gemini_Models_Overview.md`: ข้อมูล Gemini Models

## 🎨 02. Architecture & Integration (สถาปัตยกรรมและการเชื่อมต่อ)
รายละเอียดการออกแบบโครงสร้างเชิงลึก Database และระบบย่อยต่างๆ ร่วมกับ rAthena
- `docs/02_architecture_and_integration/02_01_Mimir_Landverse_Integration_Design.md`: การออกแบบผสมผสาน Landverse
- `docs/02_architecture_and_integration/02_02_Monitoring_System_Plan_Project-Mimir.md`: ระบบ Monitoring Pipeline
- `docs/02_architecture_and_integration/02_03_Cloud_API_Fallback_Strategy_Project-Mimir.md`: แผนสำรอง Cloud API
- `docs/02_architecture_and_integration/02_04_Test_Results_Design.md`: การเก็บผลทดสอบ Latency/Accuracy
- `docs/02_architecture_and_integration/02_05_rAthena_Architecture_and_DB_Schema.md`: rAthena DB Schema
- `docs/02_architecture_and_integration/02_06_rAthena_NPC_System.md`: ระบบ rAthena NPC
- `docs/02_architecture_and_integration/02_07_AI_NPC_Integration_Design.md`: การรวม AI NPC
- `docs/02_architecture_and_integration/02_08_rAthena_NPC_Types.md`: ประเภทของ rAthena NPC
- `docs/02_architecture_and_integration/02_09_AI_Agent_Techniques_for_NPCs.md`: เทคนิค AI Agent สำหรับ NPC
- `docs/02_architecture_and_integration/02_10_rAthena_Client_AI_Communication.md`: การสื่อสารระหว่างระบบต่างๆ
- `docs/02_architecture_and_integration/02_11_Ragnarok_client_on_mac.md`: การพัฒนา Client บน Mac
- `docs/02_architecture_and_integration/02_12_Heimdall_Self_Host_LLM_Server.md`: เอกสาร Heimdall Self-Hosted LLM Gateway สำหรับ Mimir

## 🚀 03. Implementation Plans (แผนการดำเนินงาน)
แผนงานจริงที่แบ่งเป็น Phase, Sprints, และ Tasks สำหรับ Version 2.3
- `docs/03_implementation_plans/03_01_Implementation_Plan_Project-Mimir.md`: แผนประยุกต์ใช้ในภาพรวม
- `docs/03_implementation_plans/03_02_Implementation_Plan_rAthena_Setup.md`: แผนจัดตั้ง rAthena Server
- `docs/03_implementation_plans/03_03_Sprint_1.4_Cron_Integration_Plan.md`: ระบบการเชื่อมต่อรหัส Cron
- `docs/03_implementation_plans/03_04_Phase_1_Remaining_Tasks.md`: สิ่งที่ค้างใน Phase 1
- `docs/03_implementation_plans/03_05_Implementation_Plan_AI_Tables_Ingestion.md`: แผนจัดการตาราง AI
- `docs/03_implementation_plans/03_06_Implementation_Plan_Phase_1_Completion.md`: งานปิด Phase 1
- `docs/03_implementation_plans/03_07_Implementation_Plan_Phase_2_Agent_Chat.md`: งานเริ่ม Phase 2 Agent Chat
- `docs/03_implementation_plans/03_08_Implementation_Plan_Model_Configuration_DB.md`: การย้าย Model Config ลง DB
- `docs/03_implementation_plans/03_09_Plan_Model_Selection.md`: ระบบ Model Selection
- `docs/03_implementation_plans/03_09_Implementation_Plan_Iterative_QA_Generation.md`: แผนพัฒนาระบบ Q/A Iterative
- `docs/03_implementation_plans/03_10_Implementation_Plan_Playground_UI.md`: แผนจัดการ Playground UI
- `docs/03_implementation_plans/03_11_Test_Plan_E2E_Phase_1_to_9.md`: แผนทดสอบระบบ E2E ของเก่า
- `docs/03_implementation_plans/03_12_Testing_and_PR_Workflow.md`: กระบวนการทดสอบและ PR Workflow
- `docs/03_implementation_plans/03_13_Implementation_Plan_v2.3_Sprints_Project-Mimir.md`: สรุปรวบยอดแผนงาน Sprints ทั้งหมดเพื่อการพัฒนา Multi-Tenant ตามสเปก v2.3
- `docs/03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md`: แผน Sprint 36-39 — Local LLM optimization (rerank/CoT/self-consistency/specialty router/LoRA) เป้าหมาย Eir HBp% 40.6% → 60%+

## 🛡️ 04. Evaluation & Testing (การทดสอบและการประเมินผล)
- `docs/04_evaluation_and_testing/04_01_Implementation_Plan_Agent_Evaluation.md`: แผนทดสอบ Agent Evaluation
- `docs/04_evaluation_and_testing/04_02_Implementation_Plan_Multi_Tenant_Modular.md`: ทดสอบ Modular Multi-Tenant
- `docs/04_evaluation_and_testing/04_02_Implementation_Plan_Multi_Tenant_Modular_v2.4.md`: อัปเดตแผนทดสอบ Multi-Tenant
- `docs/04_evaluation_and_testing/04_03_HealthBench_Pro_Baseline_2026-05-04.md`: HealthBench-Pro baseline scoreboard (Eir, n=20) + reference: arXiv:2505.08775
- `docs/04_evaluation_and_testing/04_04_Medical_Benchmarks_Catalog.md`: Catalog of 5 downloaded medical benchmarks (MedQA/MedMCQA/PubMedQA/HealthBench/MedXpertQA) — schemas, sizes, mapping to Asgard use cases

## ⚙️ 05. Security & Management (ความปลอดภัยและการจัดการสิทธิ์)
ความปลอดภัย การจัดการผู้ใช้ สิทธิ์ต่างๆ และแผนปรับปรุงโมดูลต่างๆ (รวมไฟล์หัวข้อ 05, 06, 07 เดิมบางส่วน)
- `docs/05_security_and_management/05_01_UI_UX_Spec_Multi_Tenant.md`: ข้อกำหนด UI/UX ของระบบ Multi-Tenant
- `docs/05_security_and_management/06_01_Security_Spec_Multi_Tenant.md`: สเปกด้านความปลอดภัยของ Multi-Tenant
- `docs/05_security_and_management/07_01_User_Management_RBAC_Spec.md`: ระบบจัดการผู้ใช้งานแบบ Role-Based Access Control

## 🧩 06. Misc Resources (ข้อมูลและเอกสารอื่นๆ)
- `docs/06_misc_resources/.*_on_mac.md`: ข้อมูลการพัฒนาไคลเอนต์บน Mac
- `docs/06_misc_resources/01_05_Sources_Implementation_Plan_Project-Mimir.md`: แผนสำหรับหน้า Sources
- `docs/06_misc_resources/01_06_Quality_Control_Implementation_Plan_Project-Mimir.md`: แผน Quality Control
- `docs/06_misc_resources/01_07_Vector_Management_Implementation_Plan_Project-Mimir.md`: แผน Vector Management
- `docs/06_misc_resources/01_08_Evaluations_Implementation_Plan_Project-Mimir.md`: แผนหน้า Evaluations
- `docs/06_misc_resources/01_09_User_Management_Implementation_Plan_Project-Mimir.md`: แผน User Management
- `docs/06_misc_resources/Zeroclaw/`: 

## 📋 ISO 29110 Documents (เอกสารอ้างอิงและสอบกลับ Phase 1 - Version 2.3 Completion)
เอกสารสำหรับการส่งมอบและรับรองตามมาตรฐาน
- `docs/iso_29110/pm/`: รูปแบบการจัดการโครงการประจำ Sprint และ Status Reports
- `docs/iso_29110/si/`: แบบบันทึกความต้องการ, โครงสร้างโค้ด, ตารางแมปการทดสอบ (Traceability Matrix) และคู่มือผู้ใช้งาน

---
*Status: Version 2.3 Completed Phase 1*
