# SI-05: User Manual (คู่มือการใช้งาน)
**Project Name:** Project Mimir
**Sprint:** 3 (Tenant Configuration & Provisioning)

## 1. System Overview (ภาพรวมระบบ)
Project Mimir เป็นระบบ AI แพลตฟอร์มแบบ Multi-Tenant ที่ให้ผู้บริหารระบบ (SuperAdmin) สามารถแบ่งแยกพื้นที่ทำงาน (Workspace) ให้กับแต่ละ Tenant (หรือโปรเจกต์) ได้อย่างเป็นอิสระ โดยแต่ละ Tenant จะสามารถจัดการโมเดล AI (LLM), Vector Database (RAG) และทดสอบระบบผ่าน AI Playground ของตนเองได้โดยไม่ข้องแวะกับข้อมูลของ Tenant อื่น

## 2. Getting Started (การเริ่มต้นใช้งาน)
### การเข้าสู่ระบบ
1. เปิดเบราว์เซอร์และเข้าไปที่ `http://localhost:3000` (หรือ URL ของเซิร์ฟเวอร์ที่ติดตั้ง)
2. หากยังไม่ได้ล็อกอิน ระบบจะพากลับมาที่หน้า `/login`
3. กรอก **Email** และ **Password** ที่ได้รับมอบหมาย
   - *สิทธิ์ SuperAdmin สำหรับจัดการทุก Tenant (เช่น `admin@superadmin.com`)*
   - *สิทธิ์ Admin ของ Tenant เฉพาะเจาะจง (เช่น `admin@mimir.local`)*
4. กด **Sign In** เพื่อเข้าสู่แดชบอร์ด

## 3. Features & Usage (ฟีเจอร์และการใช้งาน)

### 3.1 การจัดการ Tenant (SuperAdmin Only)
ฟีเจอร์นี้สงวนไว้สำหรับผู้ดูแลระบบระดับสูงสุด ใช้ในการสร้างพื้นที่ทำงานใหม่ให้กับลูกค้าหรือโปรเจกต์ใหม่:
1. ล็อกอินด้วยบัญชีระดับ SuperAdmin
2. ที่แถบเมนูด้านซ้ายมือให้คลิก **Tenants**
3. ระบบจะแสดงตารางพื้นที่ทำงานทั้งหมดที่มีในหน้าต่าง
4. **การสร้าง Tenant ใหม่ (Provisioning):**
   - ใส่ชื่อ Tenant เช่น "Sword & Magic Server"
   - ใส่อีเมลสำหรับผู้ดูแล Tenant นี้
   - เลือกตัวเลือก **Provision Dedicated Vector DB Collection** หากต้องการแยกระบบฐานข้อมูลจำเพาะ
   - กดปุ่มเพื่อสร้าง 
   - ระบบจะดำเนินการสร้าง Tenant, สร้างบัญชีผู้ใช้เริ่มต้น, แจกจ่ายตั้งค่าฐาน และสร้างฐานข้อมูลเวกเตอร์ให้ทันที
5. **การลบ Tenant (Deprovisioning):**
   - หากต้องการลบ กดที่ไอคอน "ถังขยะ" หลังชื่อ Tenant
   - ระบบจะทำการลบข้อมูลที่เกี่ยวข้อง (รวมถึงบัญชีผู้ใช้, Config, และข้อมูลใน Vector DB) อย่างหมดจด

![ภาพตัวอย่างหน้า Tenant Management](/Users/paripolt/.gemini/antigravity/brain/7a76097f-90af-4d4c-a196-5a86716b23de/sprint3_e2e_correct_credentials_1771941079220.webp)

### 3.2 กานตั้งค่า Tenant Configuration (Tenant Admin)
ในแต่ละ Workspace ผู้ดูแลระบบของพื้นที่สามารถปรับแต่ง AI ของตัวเองได้:
1. ที่แถบเมนูด้านซ้ายคลิก **Settings**
2. เลือก Default Provider ว่าต้องการเชื่อมต่อผ่าน Engine ใด (เช่น `ollama`, `google`, `openai`)
3. ระบุ Default Model (เช่น `gemini-2.5-flash` หรือ `llama3.2`) ซึ่งจะส่งผลให้ระบบ Pipeline ทั้งหมดเรียกใช้โมเดลนี้แบบอัตโนมัติหากไม่ได้เจาะจง
4. บันทึกข้อมูลและคีย์ API (API Key) อื่นๆ
5. กด **Save Settings** มุมขวาล่าง 

### 3.3 การใช้งาน AI Playground (RAG & Actions)
ระบบจำลองแชทที่ให้ทดสอบความสามารถของ AI ในการเรียกใช้ข้อมูลใน Vector DB หรือการส่งคำสั่ง (Action) ไปยังเซิร์ฟเวอร์เกม:
1. ที่แถบเมนูด้านซ้ายคลิก **Playground**
2. ใช้งาน **Persona Selection**: ด้านบนของหน้าต่างแชทจะมีแบนเนอร์หรือ Dropdown ให้เลือก Role-play (เช่น **Mimir**, **Sage Ariel**) ซึ่งพฤติกรรมการตอบหรือข้อมูลที่เข้าถึงจะเปลี่ยนตามการตั้งค่า Persona
3. พิมพ์ข้อความสอบถามทางแชท (เช่น "มอนสเตอร์ตัวไหนให้ EXP เยอะสุด" หรือ "ช่วย heal ฉันหน่อย")
4. **การตรวจสอบผลลัพธ์ Action**: หาก AI ได้รับอนุญาตให้สั่งการในเกมได้ จะเห็นโครงสร้างระบบตีความคำสั่งสีเขียวปรากฏใต้ข้อความ (เช่น `[ACTION: heal]`)
5. **การตรวจสอบ RAG (Retrieval-Augmented Generation)**: หาก AI ตอบคำถามโดยใช้แหล่งข้อมูลจากคู่มือในฐานข้อมูล จะมีกล่อง Reference "Source Citation" ดึงพิกัดเอกสารที่ใช้อ้างอิงมาแสดงด้านล่าง

![ภาพตัวอย่างการใช้ AI Playground (Actions)](/Users/paripolt/.gemini/antigravity/brain/7a76097f-90af-4d4c-a196-5a86716b23de/sprint3_e2e_ts04_final_fix_1771948496619.webp)
