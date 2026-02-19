# Implementation Plan: AI Model Configuration in MariaDB

แผนการดำเนินงานเพื่อย้ายการตั้งค่า AI Model จากค่าคงที่ใน Code (Hardcoded) ไปเก็บไว้ใน MariaDB เพื่อรองรับการจัดการแบบ Dynamic และการทำ Value List สำหรับ Dashboard

## 1. เป้าหมาย (Goal)
- เก็บรายการ Model (Model Registry) ไว้ใน Database
- สามารถเลือก Model ให้ NPC แต่ละตัวได้ผ่าน Database (Per-NPC configuration)
- รองรับการทำ Value List ใน UI สำหรับเลือกใช้งานโมเดลต่างๆ (Ollama, Gemini, OpenAI ฯลฯ)

## 2. การเปลี่ยนแปลงฐานข้อมูล (Database Changes)

### [NEW] `03_08_add_model_configs.sql`
สร้างตาราง `ai_models` และเพิ่มฟิลด์ใน `ai_npc_persona`:

- **Table: `ai_models`**
    - `model_id` (PK): ชื่อสไลด์ของโมเดล (e.g., `qwen2.5:32b`, `gemini-2.5-flash`)
    - `provider`: ผู้ให้บริการ (e.g., `ollama`, `google`, `openai`)
    - `model_type`: ประเภท (e.g., `llm`, `embedding`, `reranker`)
    - `is_active`: สถานะการเปิดใช้งาน
    - `capabilities`: รายละเอียดความสามารถ (JSON) เช่น รองรับ Tool, Vision หรือไม่
- **Update: `ai_npc_persona`**
    - เพิ่มคอลัมน์ `model_id` (FK) เพื่อเชื่อมโยงกับตาราง `ai_models`

## 3. แผนการดำเนินงาน (Execution Plan)

### Step 1: Database Migration
- สร้างไฟล์ Migration ใหม่ใน `ro-ai-bridge/migrations/`
- รัน Migration เพื่อสร้างตารางและอัปเดตความสัมพันธ์

### Step 2: Rust Backend (ro-ai-bridge)
- **Model Definition**: สร้าง Struct `ModelConfig` ใน `src/models/`
- **Data Access**: เพิ่มฟังก์ชันใน `src/services/db.rs` เพื่อดึงข้อมูล Model List และ Per-NPC Model
- **Agent Initialization**: แก้ไข `SimpleNpcAgent` และ `OracleAgent` ให้ดึงโมเดลจาก Database แทนค่า Default

### Step 3: Documentation
- อัปเดต `01_03_TRD_Project-Mimir_TH.md` ในส่วนของ DB Schema

---

## 4. แผนการทดสอบ (Verification Plan)

### การทดสอบเชิงเทคนิค
- [ ] ตรวจสอบว่าตารางถูกสร้างขึ้นถูกต้องใน MariaDB
- [ ] ทดสอบการ Insert ข้อมูลโมเดลใหม่ผ่าน SQL แล้วตรวจสอบว่า AI Bridge ดึงไปใช้ได้จริง
- [ ] ทดสอบกรณี NPC ไม่มีการระบุโมเดล (ต้องใช้ System Default)

### ผลลัพธ์ที่คาดหวัง
- ระบบสามารถโหลดโมเดลตามที่ระบุใน DB สำหรับ NPC แต่ละตัวได้
- Dashboard สามารถดึงรายการโมเดลจาก DB มาทำ Dropdown ได้
