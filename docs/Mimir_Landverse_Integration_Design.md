# Design Document: AI-Native Web3 Middleware (Project Mimir x Landverse)

เอกสารฉบับนี้อธิบายการออกแบบ Database Schema และ Middleware Architecture เพื่อจำลองระบบ Ragnarok Online Landverse (Stamina, Land, Tokens) มาทำงานร่วมกับ Project Mimir บนเซิร์ฟเวอร์ส่วนตัว

## 1. Database Schema Design (Simulated Web3)

เราจะสร้างตารางใหม่ (Custom Tables) ในฐานข้อมูลของ rAthena เพื่อจำลองฟีเจอร์ Web3 โดยไม่ต้องแก้ไข Source Code หลักของ rAthena มากนัก

### 1.1 Land Ownership System (`mimir_land_ownership`)
เก็บข้อมูลกรรมสิทธิ์ที่ดิน ซึ่ง AI จะนำไปใช้ในการอนุญาตให้แก้ไข NPC หรือสร้าง Event ในพื้นที่นั้นๆ

```sql
CREATE TABLE IF NOT EXISTS `mimir_land_ownership` (
  `land_id` INT(11) UNSIGNED NOT NULL AUTO_INCREMENT,
  `map_name` VARCHAR(50) NOT NULL,          -- ชื่อแผนที่ (เช่น 'prontera', 'pay_fild04')
  `owner_account_id` INT(11) UNSIGNED NOT NULL, -- เจ้าของที่ดิน (FK -> login.account_id)
  `purchase_price` DECIMAL(20, 4) DEFAULT 0, -- ราคาที่ซื้อมา (หน่วยเป็น Simulated Token)
  `rent_price_daily` DECIMAL(20, 4) DEFAULT 0, -- ราคาเช่าต่อวัน (ถ้าปล่อยเช่า)
  `tax_expire_date` DATETIME,               -- วันที่หมดอายุภาษีที่ดิน
  `ai_settings` JSON,                       -- การตั้งค่า AI (เช่น "Style: Cyberpunk", "NPCs: [Guard, Healer]")
  PRIMARY KEY (`land_id`),
  UNIQUE KEY `map_unique` (`map_name`)
) ENGINE=InnoDB;
```

### 1.2 Token Economy (`mimir_token_ledger`)
ใช้จำลอง Transaction ของเหรียญ `$ADAM` และ `$ION` เพื่อให้ AI สามารถตรวจสอบประวัติและสร้าง Quest ที่ให้รางวัลเป็น Token ได้

```sql
CREATE TABLE IF NOT EXISTS `mimir_token_ledger` (
  `transaction_id` INT(11) UNSIGNED NOT NULL AUTO_INCREMENT,
  `account_id` INT(11) UNSIGNED NOT NULL,
  `token_type` ENUM('ADAM', 'ION') NOT NULL,
  `amount` DECIMAL(20, 4) NOT NULL,         -- จำนวน (ลบ = จ่าย, บวก = รับ)
  `transaction_type` ENUM('MINT', 'BURN', 'TRANSFER', 'REWARD', 'FEE') NOT NULL,
  `related_source` VARCHAR(50),             -- ที่มา (เช่น 'AI_QUEST_123', 'LAND_TAX')
  `created_at` DATETIME DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (`transaction_id`),
  INDEX `account_idx` (`account_id`)
) ENGINE=InnoDB;
```

### 1.3 Stamina System
ใช้ตาราง `global_reg_value` ที่มีอยู่แล้วของ rAthena เพื่อความง่ายในการเข้าถึงผ่าน NPC Script

*   **Key: `#ACC_STAMINA`** (Account Variable)
    *   **Value:** Integer (จำนวน Stamina คงเหลือ)
    *   **Logic:** Middleware จะทำการ Reset ค่านี้ทุกเที่ยงคืน หรือเพิ่มให้เมื่อผู้เล่นนอนบนเตียงในบ้าน (NFT Bed)

## 2. AI Middleware Architecture

Project Mimir จะทำหน้าที่เป็น "The Bridge" ระหว่าง Database นี้กับตัวเกม rAthena

### 2.1 Modules
1.  **Stamina Manager (Background Service):**
    *   ทำงานทุกเที่ยงคืน (Cron Job) เพื่อ Reset Stamina
    *   ทำงานเมื่อผู้เล่นมี Interaction กับเตียง (ผ่าน API Call จาก NPC Script)
2.  **Landlord AI (Tier 2 Agent):**
    *   เมื่อผู้เล่นเข้าสู่ Map ที่มีเจ้าของ AI จะตรวจสอบ `mimir_land_ownership`
    *   โหลด `ai_settings` เพื่อ Spawn NPC หรือเปลี่ยนบรรยากาศ (Weather/Effect) ให้ตรงกับเจ้าของที่ตั้งไว้
3.  **Economy Watchdog (Tier 3 Agent):**
    *   วิเคราะห์ `mimir_token_ledger` เพื่อดู Inflation/Deflation
    *   ปรับ Rate การดร็อปของ `$ADAM` ใน Global Variables โดยอัตโนมัติ

## 3. Integration Points

### 3.1 NPC Script to Middleware
การสื่อสารจากเกมมายัง AI (เช่น ผู้เล่นคุยกับ NPC เพื่อซื้อที่ดิน)

```c
// Example rAthena Script
prontera,150,150,4	script	Land Manager	123,{
    mes "[Land Manager]";
    mes "ต้องการซื้อที่ดินผืนนี้ในราคา 100 ION หรือไม่?";
    if(select("ซื้อเลย", "ยกเลิก") == 2) close;
    
    // เรียกใช้ AI Bridge เพื่อตรวจสอบเงินและโอนกรรมสิทธิ์
    // สมมติใช้ Plugin หรือ HTTP Request ผ่าน bindatcmd
    @result = mimir_buy_land(getcharid(3), strcharinfo(3));
    
    if (@result == 1) {
        mes "ยินดีด้วย! คุณเป็นเจ้าของที่ดินแล้ว";
    } else {
        mes "เกิดข้อผิดพลาด หรือเงินไม่พอ";
    }
    close;
}
```

### 3.2 Middleware to Database
AI จะเขียนข้อมูลลง Database โดยตรง (Direct SQL Connection) เพื่อความเร็วในการอัปเดต Token และ Land Ownership

---

# Design Document: RO Landverse RAG Pipeline

เอกสารส่วนนี้ระบุการออกแบบระบบ RAG (Retrieval-Augmented Generation) เพื่อดึงข้อมูลจากเว็บไซต์ [rolth.maxion.gg](https://rolth.maxion.gg/) มาใช้ตอบคำถามผู้เล่น

## 1. RAG Architecture

เนื่องจากเว็บไซต์เป้าหมายเป็น Single Page Application (SPA) ที่โหลดข้อมูลด้วย JavaScript การดึงข้อมูลแบบ Static HTML ทั่วไปจะไม่เพียงพอ

### 1.1 Ingestion Pipeline (การนำเข้าข้อมูล)
แบ่งออกเป็น 3 แหล่งข้อมูลหลัก:

1.  **Dynamic Content (News/Events):**
    *   **Method:** Headless Browser (Puppeteer/Playwright)
    *   **Frequency:** ทุก 6 ชั่วโมง
    *   **Target:** หน้าหลัก, หน้าข่าวสาร, Patch Notes
2.  **Static/Wiki Data (Game Info):**
    *   **Method:** Simulated API Calls / Puppeteer
    *   **Frequency:** รายสัปดาห์ (หรือเมื่อมี Patch ใหม่)
    *   **Target:** ข้อมูล Class, Monster, Item (หากมีหน้า Wiki ย่อย)
3.  **Community Updates (Social):**
    *   **Method:** Graph API (Facebook) หรือ Official Discord Scraper
    *   **Target:** ประกาศปิดปรับปรุงเซิร์ฟเวอร์, กิจกรรมเร่งด่วน

### 1.2 Processing Pipeline (การประมวลผล)
ข้อมูลดิบ (Raw HTML/JSON) จะถูกแปลงให้อยู่ในรูปแบบที่ LLM เข้าใจง่าย (Markdown)

1.  **Content Extraction:** ใช้ Library เช่น `cheerio` หรือ `readability.js` เพื่อตัด HTML Tags ที่ไม่จำเป็น (Menu, Footer) ออก
2.  **Table Handling (สำคัญสำหรับ RO):** ตาราง Stat มอนสเตอร์หรือ Drop Rate จะต้องถูกแปลงเป็น Markdown Table หรือ JSON string เพื่อให้ Vector Database เก็บความสัมพันธ์ของข้อมูลได้ถูกต้อง
3.  **Chunking Strategy:**
    *   **Fixed-size Chunking:** สำหรับเนื้อหาทั่่วไ (News)
    *   **Semantic Chunking:** สำหรับข้อมูล Item/Monster (1 Item = 1 Chunk เพื่อไม่ให้ Stat ขาดตอน)

### 1.3 Vector Database (Qdrant)
*   **Collection Name:** `landverse_th_knowledge`
*   **Payload Fields:** `source_url`, `title`, `category` (News, Guide, Item), `updated_at`

## 2. Additional Pipelines (ระบบเสริมที่ควรมี)

เพื่อให้ RAG ทำงานได้อย่างสมบูรณ์และมีความฉลาด ควรมี Pipeline เสริมดังนี้:

### 2.1 Update Monitoring Pipeline (ระบบตรวจสอบการอัปเดต)
*   **Change Detection:** เก็บ Hash ของหน้าเว็บเป้าหมาย หาก Hash เปลี่ยน ให้ทำการ Re-crawl เฉพาะหน้านั้น
*   **Patch Note Analyzer:** เมื่อเจอ Patch Note ใหม่ ให้ LLM สรุป "สิ่งที่เปลี่ยนแปลง" (Changes) และนำไป update หรือ flag ข้อมูลเก่าว่าเป็น out-dated

### 2.2 Query Enhancement Pipeline (ระบบขยายคำถาม)
ผู้เล่นมักใช้คำสั้นๆ เช่น "หมวกงู"
*   **Term Expansion:** แปลง "หมวกงู" -> "Snake Head Hat คุณสมบัติ วิธีทำ" ก่อนส่งไปค้นหาใน Vector DB
*   **Synonym Dictionary:** สร้าง Dictionary คำศัพท์เฉพาะของ RO (Slang) ให้ AI เข้าใจ (เช่น "AB" = Angelus + Blessing)

### 2.3 Feedback Loop (ระบบเรียนรู้จากผู้ใช้)
*   **Action:** ปุ่ม Thumbs up/down หลังคำตอบ
*   **Process:** หากผู้ใช้กด Thumbs down ให้บันทึกคำถามและคำตอบลง Log เพื่อให้ Human Reviewer มาตรวจสอบและปรับปรุง Prompt หรือเพิ่มข้อมูลลง Knowledge Base

## 3. Technology Stack Recommendation
*   **Crawler:** Crawlee (Node.js) หรือ Scrapy + Splash (Python)
*   **Parser:** LangChain (มี Text Splitters ที่ดี)
*   **Orchestrator:** Apache Airflow หรือ n8n (สำหรับตั้งเวลา Crawl)
