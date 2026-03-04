# SI-05: User Manual (คู่มือการใช้งาน)
**Project Name:** Project Mimir
**Sprint:** 18 (ปรับปรุงล่าสุด — ครอบคลุมทุกฟีเจอร์ถึง Sprint 18)

## 1. System Overview (ภาพรวมระบบ)
Project Mimir เป็นระบบ AI แพลตฟอร์มแบบ Multi-Tenant สำหรับจัดการ Knowledge Base, RAG Pipeline, Agent Testing และ Coverage Analytics โดยแต่ละ Tenant จะมีพื้นที่ข้อมูลแยกจากกันอย่างเป็นอิสระ (Data Isolation)

ระบบประกอบด้วย 15 หน้าหลักที่จัดกลุ่มเป็น 4 กลุ่มหลัก + 1 หน้า Overview:

| กลุ่ม           | หน้า                                        | คำอธิบาย                                 |
| ------------- | ------------------------------------------ | -------------------------------------- |
| **Overview**  | Dashboard                                  | สรุปสถิติภาพรวมทั้งระบบ                     |
| **Data**      | Sources, Knowledge, Vector, Quality        | จัดการข้อมูลเข้า + ตรวจสอบคุณภาพ            |
| **AI**        | Playground, Agents, Graph                  | ทดสอบ AI, จัดการ Agent, Knowledge Graph |
| **Analytics** | Coverage, LLM Analytics, Evaluations, Logs | วิเคราะห์ความครอบคลุม + ประสิทธิภาพ         |
| **Admin**     | Settings, Tenants, Users                   | จัดการระบบ, พื้นที่ทำงาน, ผู้ใช้                |

---

## 2. Getting Started (การเริ่มต้นใช้งาน)

### 2.1 การเข้าสู่ระบบ (Login)
1. เปิดเว็บเบราว์เซอร์ไปที่ `http://localhost:3000` (หรือ URL ที่ติดตั้ง)
2. กรอก **Email address** และ **Password** ในกล่อง Login
   - SuperAdmin: `admin@superadmin.com`
   - Tenant Admin: เช่น `admin@mimir.local`
3. คลิกปุ่มสีน้ำเงิน **"Sign In"** → เข้าสู่หน้า Dashboard อัตโนมัติ

### 2.2 การใช้งาน Navigation Bar
Navbar ถูกจัดเป็น **Grouped Dropdown Menus** 5 กลุ่ม:

```
Project Mimir | Overview | Data ▼ | AI ▼ | Analytics ▼ | Admin ▼ | [Tenant: ___] [Logout]
```

- **เปิดเมนู:** Hover หรือคลิกที่กลุ่ม → dropdown แสดงรายการย่อย
- **เปลี่ยน Tenant:** คลิก dropdown "Tenant" มุมขวาบน → เลือก Tenant → หน้าจะ reload อัตโนมัติ
- **Logout:** คลิกไอคอน ลูกศรออก (→) มุมขวาสุด

### 2.3 Pipeline Status Bar
แถบ Pipeline แสดงอยู่ใต้ Navbar ทุกหน้า แสดงสถานะข้อมูลตลอดกระบวนการ:

```
SOURCES (6) → CHUNKS (585) → DEDUP (4 done) → QA (—) → VECTOR (—)
```

- คลิกที่แต่ละขั้นตอนเพื่อไปยังหน้าที่เกี่ยวข้อง
- ตัวเลขอัปเดตอัตโนมัติทุก 10 วินาที

---

## 3. Data — จัดการข้อมูล

### 3.1 Sources (แหล่งข้อมูล)
**เมนู:** Data ▼ → Sources | **URL:** `/sources`

หน้านี้ใช้สำหรับเพิ่ม จัดการ และ sync แหล่งข้อมูลที่จะนำเข้าระบบ

**การใช้งาน:**
1. คลิกปุ่ม **"+ Add Source"** มุมขวาบน
2. เลือกประเภท Source: `File Upload`, `URL`, `Web Hierarchy` (Crawl เว็บไซต์)
3. กรอกชื่อและรายละเอียด → คลิก **"Save"**
4. คลิกปุ่ม **"▶ Sync"** ในแถวของ Source → ระบบจะเริ่ม Chunking อัตโนมัติ
5. สถานะจะเปลี่ยนจาก `IDLE` → `RUNNING` → `COMPLETED`

**Web Hierarchy Loader:**
- สำหรับ crawl เว็บไซต์ทั้งหมด ระบุ URL หลัก + จำนวนหน้าสูงสุด (ตั้งค่าได้ที่ Admin → Settings → Pipeline)
- ระบบจะสร้าง Chunks แบบ hierarchy (h1/h2/h3 aware)

### 3.2 Knowledge Base (คลังความรู้)
**เมนู:** Data ▼ → Knowledge | **URL:** `/knowledge`

แสดงรายการ Chunks ทั้งหมดในระบบพร้อม metadata

**การใช้งาน:**
1. ดูรายการ Chunks ทั้งหมด — แต่ละ chunk แสดง: เนื้อหา, Source (badge สี), chunk index, token count
2. **กรอง:** ใช้ช่อง Search ค้นหาเนื้อหา หรือกรองตาม Source dropdown
3. **ดูรายละเอียด:** คลิกที่ chunk → แสดง Side panel: เนื้อหาเต็ม, metadata (qa_status, split_level), timestamps
4. **Generate QA:** เลือก chunks → คลิก **"Generate QA"** → ระบบสร้างคู่คำถาม-คำตอบจาก chunk content

### 3.3 Vector Management (จัดการเวกเตอร์)
**เมนู:** Data ▼ → Vector | **URL:** `/vector`

หน้านี้ใช้สำหรับจัดการ Qdrant vector database — ดูสถิติ, ค้นหา, และ index ข้อมูลใหม่

**KPI Cards (4 กล่องด้านบน):**
| Card          | คำอธิบาย                        |
| ------------- | ----------------------------- |
| Qdrant Points | จำนวน vectors ใน Qdrant        |
| MariaDB Q/A   | จำนวน QA pairs ใน Database     |
| Indexed       | จำนวนที่ embed แล้ว + Sync Rate % |
| Pending Index | จำนวนที่รอ embed                 |

**Vector Search Preview:**
1. เลือก **Tenant** จาก dropdown (All / Default / Ragnarok TH / Medical Clinic A)
2. Toggle **"Show Expired Data"** ตามต้องการ
3. พิมพ์คำถามในช่อง Search (เช่น "What is Moonstone?") → คลิก **"Search"**
4. ผลลัพธ์แสดง: **Score %** (similarity), **Q/A** content, **Source** badge + Chunk #

**Index Pending Data:** คลิกปุ่ม **"⚡ Index Pending Data"** เพื่อ embed QA ที่ยังไม่ได้ index

### 3.4 Quality Control (ควบคุมคุณภาพ)
**เมนู:** Data ▼ → Quality | **URL:** `/quality_control`

Kanban board สำหรับตรวจสอบ QA pairs ที่ AI สร้างขึ้น

**การใช้งาน:**
1. Board แบ่ง 2 คอลัมน์: **Pending Review** (รอ) | **Resolved** (เสร็จ)
2. แต่ละการ์ดแสดง: คำถาม, คำตอบ, ป้ายกำกับ (`CONFLICT` สีแดง, `DUPLICATE` สีเหลือง)
3. **Duplicate:** คลิก **"Merge"** → ระบบรวมเนื้อหาอัตโนมัติ
4. **Conflict:** คลิก **"Resolve Conflict"** → เลือก **"Keep A"** / **"Keep B"** หรือกรอก Custom Resolution

---

## 4. AI — ทดสอบ AI

### 4.1 Playground (ห้องทดลอง)
**เมนู:** AI ▼ → Playground | **URL:** `/playground`

จำลองการสนทนากับ AI Agent เหมือน Chat UI

**การใช้งาน:**
1. เลือก **Persona** จาก dropdown มุมขวาบน (เช่น `System Default`, `Mimir Game Guide`)
2. พิมพ์คำถามในช่อง **"Type a message..."** ด้านล่าง
3. คลิก **"Send"** (หรือกด Enter) → รอคำตอบจาก AI
4. คำตอบจะแสดง: ข้อความ, Action badges (ถ้ามี), Source citations พร้อม Similarity Score
5. คลิก Citation link เพื่อดูต้นฉบับ

### 4.2 Agents (จัดการ Agent)
**เมนู:** AI ▼ → Agents | **URL:** `/agents`

หน้านี้แสดงรายการ Agent ที่มีในระบบ พร้อม Tool Registry ของแต่ละ Agent

**การใช้งาน:**
1. ดูรายการ Agent ทั้งหมด — แต่ละ Agent แสดง: ชื่อ, ประเภท (Tier 1/2), สถานะ
2. คลิกที่ Agent เพื่อดูรายละเอียด: Prompt, Tools ที่ใช้ได้, Provider/Model ที่กำหนด

### 4.3 Knowledge Graph (กราฟความรู้)
**เมนู:** AI ▼ → Graph | **URL:** `/graph`

แสดง Entity-Relationship Graph จากข้อมูลในระบบ (ใช้ Neo4j)

**การใช้งาน:**
1. ดูสถิติ: จำนวน Nodes, Edges, ประเภท Entity
2. **Search Entities:** พิมพ์ชื่อ entity ในช่อง "Search by name..." → คลิก 🔍
3. **Filter ประเภท:** เลือก Entity Type จาก dropdown (Person, Organization, Location, Concept, Event, Product, Drug, Symptom, Item, Monster)
4. **Visualization:** คลิกที่ node ในกราฟ → แสดง details + connections ในแผงด้านขวา
5. **Trigger Extraction:** กดปุ่ม "Refresh" เพื่อดึง entity ใหม่จาก source data

---

## 5. Analytics — วิเคราะห์

### 5.1 Coverage Analytics (วิเคราะห์ความครอบคลุม)
**เมนู:** Analytics ▼ → Coverage | **URL:** `/coverage`

Dashboard แสดงความครอบคลุมของข้อมูลในทุกมิติ (Source → Chunk → QA → Vector → KG)

**KPI Cards (5 กล่อง):**
| Card              | คำอธิบาย                         |
| ----------------- | ------------------------------ |
| Total Sources     | จำนวนแหล่งข้อมูลทั้งหมด              |
| QA Coverage       | สัดส่วน Sources ที่มี QA pairs      |
| Vector Coverage   | สัดส่วน QA ที่ embed แล้ว           |
| Avg Chunks/Source | จำนวน chunk เฉลี่ยต่อ source       |
| KG Entities       | จำนวน entity ใน Knowledge Graph |

**Pipeline Flow:** แสดงความปกติ/ผิดปกติของการไหลข้อมูล Source→Chunk→QA→Vector→KG

**Gap Analysis Panel:**
- แสดงรายการ Sources ที่ยังไม่ครบกระบวนการ
- สามารถ Sort ตาม: Source Name, Chunks, QA Pairs, QA Coverage %, Gaps

### 5.2 LLM Analytics (วิเคราะห์ LLM)
**เมนู:** Analytics ▼ → LLM Analytics | **URL:** `/analytics/llm`

แสดงสถิติการใช้งาน LLM: จำนวน requests, token usage, latency, costs (ถ้ามี)

### 5.3 Evaluations (ประเมิน Agent)
**เมนู:** Analytics ▼ → Evaluations | **URL:** `/evaluations`

ทดสอบ AI Agent ด้วยชุดข้อสอบอัตโนมัติ (Batch Evaluation)

**การใช้งาน:**
1. คลิก **"✨ New Evaluation Wizard"** มุมขวาบน
2. **Step 1:** เลือก Agents ที่จะทดสอบ → **Next**
3. **Step 2:** เลือก AI Models (เลือกได้หลายตัว) → **Next**
4. **Step 3:** กำหนด Question Limit → **Start Batch Run**
5. ดูผลใน **Heatmap Grid**: แถว = Models, คอลัมน์ = Agents, สี = คะแนน (เขียว/เหลือง/แดง)
6. คลิกช่องคะแนน → ดูรายละเอียด + **"Edit Score"** เพื่อ override คะแนน

### 5.4 Conversation Logs (บันทึกสนทนา)
**เมนู:** Analytics ▼ → Logs | **URL:** `/conversations`

ดูประวัติการสนทนาทั้งหมดที่เกิดขึ้นในระบบ

**การใช้งาน:**
1. ดูรายการ conversations — แสดง: timestamp, user, agent, สรุปเนื้อหา
2. คลิกที่ conversation เพื่อดู full chat history
3. ดู Steps ของแต่ละ turn: RAG retrieval, tool calls, LLM responses

---

## 6. Admin — จัดการระบบ

### 6.1 Settings (ตั้งค่า)
**เมนู:** Admin ▼ → Settings | **URL:** `/settings`

หน้าตั้งค่าแบ่งเป็น **8 แท็บ** ด้านซ้าย:

| แท็บ                 | คำอธิบาย                                                                                                        |
| ------------------- | ------------------------------------------------------------------------------------------------------------- |
| **General**         | เปลี่ยนชื่อ Tenant, ดู Tenant ID                                                                                   |
| **AI Models**       | กำหนด Provider/Model สำหรับแต่ละ slot: Chat, RAG, Pipeline Generator, Judge, Embedding + Heimdall Gateway URL/Key |
| **Pipeline**        | Max Crawl Pages, Chunk Strategy/Size/Overlap, Dedup Threshold                                                 |
| **Knowledge Graph** | Neo4j connection, link ไป Graph Explorer                                                                      |
| **Search**          | Embedding Model, Top-K, Similarity Threshold, Search Mode (Semantic/Hybrid/Keyword)                           |
| **Security**        | (Coming Soon)                                                                                                 |
| **Tenants**         | สร้าง/ลบ Tenant (SuperAdmin)                                                                                   |
| **Users**           | สร้าง/ลบ User, กำหนด Role                                                                                       |

**การเปลี่ยน AI Model:**
1. ไปแท็บ **AI Models**
2. กรอก Provider (Ollama/Heimdall/Gemini) + Model ในแต่ละ slot
3. กรอก Heimdall URL + API Key (ถ้าใช้)
4. คลิก **"Save Changes"**

### 6.2 Tenant Management (จัดการ Tenant)
**เมนู:** Admin ▼ → Tenants | **URL:** `/tenants`

*หรือผ่าน: Admin ▼ → Settings → แท็บ Tenants*

**การสร้าง Tenant ใหม่:**
1. คลิก **"+ Create Tenant"**
2. กรอก: Tenant Name, Admin Email, Admin Password
3. ติ๊ก "Create Dedicated Vector DB" (ถ้าต้องการแยก vector DB)
4. คลิก **"Create"** → ระบบสร้าง schema + กลับหน้ารายการ

### 6.3 User Management (จัดการผู้ใช้)
**เมนู:** Admin ▼ → Users | **URL:** `/users`

*หรือผ่าน: Admin ▼ → Settings → แท็บ Users*

**การเพิ่มผู้ใช้:**
1. คลิก **"+ Create User"**
2. กรอก: Username, Password, เลือก Tenant, กำหนด Role (admin/viewer)
3. คลิก **"Create"**

**การลบผู้ใช้:** คลิกไอคอนถังขยะ 🗑️ → ยืนยัน

---

## 7. Troubleshooting (แก้ปัญหาเบื้องต้น)

| ปัญหา                        | สาเหตุ                   | วิธีแก้                                          |
| --------------------------- | ----------------------- | --------------------------------------------- |
| Login ไม่ได้                  | Email/Password ผิด       | ตรวจสอบข้อมูล, ลอง reset password               |
| หน้าจอว่างเปล่า                | Backend ไม่ทำงาน          | ตรวจสอบ Rust backend (`cargo run`)            |
| Vector Search ไม่มีผลลัพธ์      | ยังไม่ได้ index            | ไปหน้า Vector → คลิก "Index Pending Data"       |
| QA = 0 ในทุกหน้า              | ยังไม่ได้ generate         | ไปหน้า Knowledge → เลือก chunks → "Generate QA" |
| Knowledge Graph ว่าง         | ยังไม่ได้ extract entities | รอ Sprint ถัดไป หรือ trigger extraction         |
| เปลี่ยน Tenant แล้วข้อมูลไม่เปลี่ยน | Cache เบราว์เซอร์         | กด Cmd+Shift+R (Hard Refresh)                 |

---
*บันทึกโดย: AI Assistant — ปรับปรุงล่าสุดวันที่ 2026-03-04 (Sprint 18)*
