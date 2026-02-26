# 📖 BRD Expansion: Web3 Integration & Gaming SaaS Platform
## โปรเจกต์ Project-Mimir (Phase 2 Expansion)

> [!NOTE]
> เอกสารฉบับนี้เป็นส่วนขยายของ [01_02_BRD_Project-Mimir_TH.md](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docs/01_02_BRD_Project-Mimir_TH.md) โดยมุ่งเน้นการปิดช่องว่าง (Gap) ที่พบจากการวิเคราะห์ Business Model ในมิติ B2B (Enterprise) และ B2C (SaaS)

---

## 1. บทสรุปความต้องการใหม่ (New Requirements Summary)

เพื่อเปลี่ยนจาก "AI In-game Assistant" ไปเป็น "Full-scale Gaming Tech Provider" โปรเจกต์ Mimir จะเพิ่มขอบเขตใน 2 โมดูลหลัก:
1.  **Module E: Web3 Data Pipeline & Analytics (B2B/B2C)** - เชื่อมต่อข้อมูลเศรษฐกิจภายนอกเกม
2.  **Module F: External Gaming SaaS Dashboard (B2C)** - แพลตฟอร์มวิเคราะห์สำหรับผู้เล่นระดับสูงและกิลด์

---

## 2. รายละเอียดโมดูลเพิ่มเติม (Module Details)

### 📊 Module E: Web3 Data Pipeline ("ความรอบรู้ด้านเศรษฐกิจ Token")

**แนวคิดหลัก:** ขยายฐานความรู้ของ AI Oracle ให้ครอบคลุมข้อมูล Real-time จาก Blockchain และ Marketplace เพื่อวิเคราะห์ความคุ้มค่าในการลงทุน (ROI)

**ฟีเจอร์สำคัญ:**
-   **DEX & NFT Tracker:** ดึงราคาเหรียญ ADAM, ION และราคา Floor Price ของ NFT อุปกรณ์สวมใส่จาก Maxion Marketplace
-   **Tokenomics Oracle:** AI สามารถตอบคำถามเกี่ยวกับ "กราฟเงินเฟ้อ", "สภาพคล่องใน Pool" และ "แนวโน้มราคาไอเทม"
-   **Stamina Efficiency Calculator:** คำนวณความคุ้มค่าของการใช้ค่า Stamina เทียบกับรายได้ที่จะได้รับในหน่วยเงินจริง (THB/USD)

### 🏛️ Module F: Guild Management & Analytics SaaS ("ERP สำหรับวาฬ")

**แนวคิดหลัก:** สร้าง Web Application แยกจากตัวเกม เพื่อให้บริการเครื่องมือบริหารจัดการกิลด์และวิเคราะห์ตลาดเชิงลึก

**ฟีเจอร์สำคัญ:**
-   **Guild Treasury ERP:** ระบบลงบัญชีกลางสำหรับกิลด์ (Income/Expense Tracker) รองรับการปันผล ION/Zeny จากภาษีปราสาท (GvG)
-   **Member KPI Dashboard:** วิเคราะห์ประสิทธิภาพลูกกิลด์ (Contribution) เพื่อใช้ประกอบการจ่ายเงินเดือนหรือรางวัล
-   **Market Arbitrage Alerts:** ระบบแจ้งเตือนเมื่อพบช่องว่างราคาระหว่างกิลด์ หรือเมื่อมีไอเทมราคาถูกผิดปกติ (Sniper Preview)

---

## 3. โมเดลรายได้ใหม่ (Expanded Revenue Model)

เพิ่มช่องทางรายได้จากบริการระดับ Enterprise และ SaaS:

| บริการ                           | กลุ่มเป้าหมาย             | รูปแบบรายได้                   |
| :------------------------------ | :--------------------- | :--------------------------- |
| **Enterprise Anti-Fraud Tool**  | Maxion / Server Owners | B2B Licence (รายปี)           |
| **Guild ERP Support**           | กิลด์ระดับวาฬ (Whales)    | Subscription (ตามจำนวนสมาชิก)  |
| **Premium Analytics Dashboard** | นักลงทุน / สายฟาร์ม       | Subscription (Tiered Access) |

---

## 4. โครงสร้างพื้นฐานทางเทคนิคที่ต้องการเพิ่ม (Technical Requirements)

-   **Web3 Connectors:** API Integration กับ Polygon Scan / ImmutableX หรือ Marketplace API ของ Maxion
-   **TimescaleDB/InfluxDB:** สำหรับเก็บข้อมูลราคาไอเทมและเหรียญแบบ Time-series เพื่อวาดกราฟ
-   **Web Frontend (Next.js/React):** สำหรับตัว Dashboard แยกต่างหากจาก rAthena Client

---

## 5. แผนการปล่อยเวอร์ชันส่วนขยาย (Expansion Timeline)

```
Phase 2.1 (4 สัปดาห์) → พัฒนา Web3 Data Scraper & Vectorize Tokenomics Data
Phase 2.2 (6 สัปดาห์) → พัฒนา MVP Web Dashboard (Next.js) + Guild Treasury Basics
Phase 2.3 (4 สัปดาห์) → เชื่อมต่อ AI Agent เข้ากับ Web Dashboard (Chat-to-Analytics)
```

---

*สร้างโดย: Antigravity AI*
*วันที่: 2026-02-19*
