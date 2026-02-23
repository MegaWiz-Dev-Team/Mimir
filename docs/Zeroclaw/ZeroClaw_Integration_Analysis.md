# การวิเคราะห์ ZeroClaw และแนวทางการบูรณาการเข้ากับ Project Mimir

**ZeroClaw** เป็น AI Assistant Infrastructure (ระบบโครงสร้างพื้นฐานสำหรับรัน AI Agent) ที่ถูกพัฒนาขึ้นด้วยภาษา Rust 100% โดยมีจุดเด่นคือ **"ความเบา รวดเร็ว และเป็นอิสระ (Agnostic)"**

จากการวิเคราะห์ข้อมูลใน GitHub Repository นี่คือสรุปฟีเจอร์และความสามารถหลักของ ZeroClaw:

## 1. ฟีเจอร์และความสามารถหลัก (Key Features)

### 1.1 ประสิทธิภาพสูงและกินทรัพยากรต่ำมาก (Ultra-Lean & High Performance)
* **ใช้ RAM น้อยกว่า 5MB:** ใช้ทรัพยากรน้อยกว่าระบบบอทแบบเดิมๆ อย่าง OpenClaw ถึง 99% และสามารถนำไปดีพลอยบนฮาร์ดแวร์ต้นทุนต่ำ (ระดับราคา $10) ได้สบายๆ
* **Fast Cold Starts:** เนื่องจากเขียนด้วย Rust และคอมไพล์เป็นไฟล์ Binary เดียว ทำให้การสตาร์ทระบบหรือรันคำสั่งต่างๆ เกิดขึ้นได้แบบเสี้ยววินาที

### 1.2 สถาปัตยกรรมแบบสลับสับเปลี่ยนได้อิสระ (Pluggable & Swappable Architecture)
* **Trait-driven Architecture:** ทุกๆ องค์ประกอบในระบบ เช่น LLM Provider, ช่องทางการสื่อสาร (Channel), เครื่องมือ (Tools), และระบบความจำ (Memory) สามารถถอดเปลี่ยนหรือคอนฟิกใหม่ได้ทั้งหมดโดยไม่ต้องมีการแก้โค้ดหลัก
* **Local & Remote LLM Support:** รองรับการเชื่อมต่อกับโมเดลบน Cloud (เช่น OpenAI, Anthropic Claude) และยังรองรับการรันโมเดลแบบ Local ได้อย่างสมบูรณ์ผ่าน ปลั๊กอินอย่าง Ollama, llama.cpp, และ vLLM โดยไม่ยึดติดกับค่ายใดค่ายหนึ่ง (No Lock-in)

### 1.3 ระบบหน่วยความจำและสืบค้นในตัว (Built-in Memory System & Search Engine)
* มีระบบ **Full-Stack Vector Search** ใส่มาให้ในตัว โดยที่นักพัฒนาไม่ต้องเหนื่อยไปเชื่อมต่อกับบริการภายนอกอย่าง Pinecone หรือ Elasticsearch
* Agent สามารถจดจำ เรียนรู้ และดึงข้อมูลในอดีตมาใช้ได้อัตโนมัติ
* สามารถเลือกระบบหลังบ้านสำหรับเก็บความจำได้หลากหลาย เช่น SQLite, PostgreSQL, Lucid หรือแม้แต่ให้เก็บเป็นไฟล์ Markdown

### 1.4 ระบบจัดการตัวตนและบุคลิกของ AI แบบมีมาตรฐาน (Identity System & AIEOS)
ZeroClaw โดดเด่นเรื่องการทำ Persona ให้กับโมเดล AI โดยรองรับ 2 มาตรฐานการตั้งค่า:
* **OpenClaw Format:** กำหนดตัวตนผ่านไฟล์เอกสารง่ายๆ เช่น `IDENTITY.md` (ฉันคือใคร), `SOUL.md` (จิตวิญญาณ/อารมณ์แบบไหน), `USER.md` (ผู้ใช้คือใคร)
* **AIEOS (AI Entity Object Specification):** เป็นมาตรฐานกลางแบบ JSON ที่ใช้ล็อกบุคลิกภาพ, จิตวิทยาหลัก, และลักษณะการใช้ภาษา (Linguistics) ทำให้เราสามารถ Backup ตัวตนของ AI เครื่องนี้ ย้ายไปไว้เครื่องอื่น หรือโมเดลตัวอื่นได้โดยที่นิสัยไม่เปลี่ยน

### 1.5 ความมั่นคงปลอดภัยระดับสูง (Secure-by-Design)
* มาพร้อมกับ Workspace แบบ Sandbox กั้นขอบเขตการทำงานของ AI อย่างชัดเจน
* ใช้วิธี **Deny-by-default (Allowlists):** คือป้องกันทุกอย่างไว้ก่อน จะอนุญาตให้ AI รันคำสั่งกี่อย่างหรือคุยผ่านช่องทางไหนต้องระบุเป็นกรณีๆ ไป 
* **Subscription Auth:** รองรับการเอาบัญชีแบบ Subscription (เช่น ChatGPT Plus หรือ Claude Pro) มาเชื่อมต่อแบบปลอดภัย มีการเข้ารหัสข้อมูล Token ไว้เพื่อใช้งานแทน API Key ธรรมดา

### 1.6 มีตัวช่วยเสริมในภาษา Python (`zeroclaw-tools`)
* บางครั้ง LLM บางเกรด หรือบางค่าย ไม่เสถียรเวลาต้องเรียกใช้ฟังก์ชัน (Tool Calling) ZeroClaw เลยทำ Package ฝั่ง Python แยกออกมาชื่อ `zeroclaw-tools` ซึ่งเอา **LangGraph** มาครอบไว้ เพื่อบังคับให้ LLM มีการทำงานในส่วนของ Tool Loop ที่มีเสถียรภาพและผลลัพธ์ที่ใช้งานได้จริง

### 1.7 Gateway API และระบบพร้อมใช้งาน (Production-ready Commands)
* มี HTTP Gateway เอาไว้รับส่ง Webhook ได้เลย รวมถึงมีการเชื่อมต่อสำเร็จรูปอย่าง WhatsApp และ Discord (ส่วน Telegram อยู่ในแผนงาน)
* มีคำสั่ง CLI สำหรับจัดการคิวงานหรือตั้งเป็น Daemon หรือระบบ Cron-job สำหรับ AI มาให้เสร็จสรรพ

---

## 2. การประยุกต์ใช้กับ Project Mimir

จากการศึกษาสถาปัตยกรรมของ **Project Mimir** ในเอกสารการออกแบบ (โดยเฉพาะ `02_07_AI_NPC_Integration_Design.md` และ `02_01_Mimir_Landverse_Integration_Design.md`) พบว่า ZeroClaw สามารถทำหน้าที่เป็น **"AI Bridge / Middleware"** หลักของระบบได้อย่างลงตัว:

### 2.1 ทำหน้าที่เป็น "AI Engine / Middleware" ระหว่าง rAthena กับ LLM
โครงสร้างตามเอกสาร: `[Player Data] -> [AI Engine] -> [Decision] -> [rAthena Execute]`
* **การประยุกต์ใช้:** สามารถรัน ZeroClaw เป็น Agent Daemon ควบคู่ไปกับเซิร์ฟเวอร์ rAthena ได้
* rAthena สามารถส่ง HTTP Request จากสคริปต์ NPC พร้อมกับ Context (HP, Zeny, Message) ยิงตรงไปที่ Gateway API (`/webhook`) ของ ZeroClaw
* ZeroClaw จะทำหน้าที่จัดการการคุยกับ LLM แล้วส่ง JSON ขากลับเป็น `response_text` และ `action` เพื่อให้ rAthena

### 2.2 ระบบกำหนดตัวตน (Persona) ของ NPC ด้วยมาตรฐาน AIEOS
* **การประยุกต์ใช้:** Mimir สามารถใช้ระบบ AIEOS ของ ZeroClaw กำหนดโปรไฟล์ NPC เป็นรูปแบบมาตรฐาน JSON ได้ (เช่น แยก Identity, Psychology, Linguistics) แยกเป็น `healer.json`, `kafra.json`, `landlord.json` เวลามีการเรียกใช้ NPC ตัวไหน ZeroClaw ก็จะโหลดตัวตนนั้นขึ้นมาสวมบทบาทได้แม่นยำ ไม่หลุดคาร์แรคเตอร์

### 2.3 ระบบความจำ (Memory & Reputation) สำหรับ NPC
* **การประยุกต์ใช้:** ZeroClaw มีระบบ Full-Stack Vector Search ในตัว ทำให้ NPC สามารถจำผู้เล่นและเหตุการณ์ในอดีตได้โดยอัตโนมัติ (เช่น จำได้ว่าผู้เล่นคนนี้เคยมาขอฮีลฟรี) โดยที่ทีมงานไม่ต้องสร้าง Vector Database แยกต่างหาก

### 2.4 Oracle Bot และระบบ RAG ตอบคำถามข้อมูลเกม
* **การประยุกต์ใช้:** ระบบ Memory ของ ZeroClaw สามารถใช้เพื่อจัดการ RAG สำหรับดึงข้อมูล Wiki/News ของ RO Landverse มาตอบคำถามผู้เล่นได้อย่างแม่นยำผ่าน Tool Calling

### 2.5 เชื่อมต่อระบบ Tools เพื่อสร้าง Action กลับไปยัง rAthena
* **การประยุกต์ใช้:** เนื่องจาก ZeroClaw มีโครงสร้างแบบ Trait-driven เราสามารถสร้าง "Custom Tools" (เช่น `rathena_execute_command`) เมื่อ LLM ตัดสินใจว่า "ตกลงขายที่ดิน" LLM จะเรียกใช้ฟังก์ชัน และ ZeroClaw จะ Response คำสั่งให้ rAthena ไปตัดเงินผู้เล่นและโอนกรรมสิทธิ์ในตาราง `mimir_land_ownership`

---

## 3. สถาปัตยกรรมการนำไปใช้งาน (Deployment Architecture Strategy)

**คำแนะนำ:** ควรรัน ZeroClaw แบบ **แยกส่วนประกอบ (Standalone Microservice)** แทนที่จะคอมไพล์รวมไว้ในโปรเจกต์เดียวกัน (Monorepo / Embedded)

### ทำไมถึงควรแยกเป็น Microservice?

1. **อัปเดตง่าย (Future-Proof):** ZeroClaw มีการพัฒนาอย่างต่อเนื่อง การแยกโปรเจกต์จะช่วยให้สามารถอัปเดต ZeroClaw ได้ผ่านการดึง Docker image ใหม่ หรือ pull update โดยไม่ต้องกระทบ Source code ของ Project Mimir (หลีกเลี่ยง Merge Conflicts ในโค้ด Rust ของ Mimir Core)
2. **ขยายสเกลได้อิสระ (Independent Scaling):** งานประมวลผล AI/LLM ใช้ปริมาณ CPU/Network ต่างจากการประมวลผล Map Server ของ rAthena หากจำนวนผู้เล่นและ NPC หนาแน่น เราสามารถเพิ่มจำนวน instance หรือ Container ให้เฉพาะ ZeroClaw ได้โดยไม่กระทบโครงสร้างเกมเพลย์หลัก
3. **ลดโอกาส Game Server ล่ม (Fault Isolation):** หากระบบ AI ทำงานล้มเหลว (เช่น Memory Leak ฝั่ง LLM หรือ API Key หมดอายุ) ศูนย์ควบคุมเกม rAthena จะยังทำหน้าที่พื้นฐานต่อไปได้ ทำให้ผู้เล่นยังคงสามารถเล่นและตีมอนสเตอร์ได้ตามปกติ (แม้ตอนนั้น NPC AI จะตอบกลับช้าหรือใช้งานไม่ได้ชั่วขณะ)
4. **ความปลอดภัย (Security):** สามารถตั้ง Firewall แยกระหว่าง Zone ที่คุยกับ AI (ซึ่งอาจถูก Prompt Injection) ออกจากเครือข่ายหลักของโลกเกมและฐานข้อมูลผู้ใช้ (login / account) ที่มีความสำคัญมากกว่าได้อย่างชัดเจน

**โครงสร้างที่แนะนำ:**
* **Development:** รัน rAthena บน Terminal หน้าต่างแยก และรันไลบรารี ZeroClaw อีก Terminal เพื่อจำลองแบบ Local Services สมบูรณ์แบบ
* **Production:** ใช้โครงสร้าง Containerization แพ็ก ZeroClaw เป็น Docker Container แยก แล้วให้ Game System ส่ง Request ตรงเข้าไปยัง Internal API Gateway ของ ZeroClaw
