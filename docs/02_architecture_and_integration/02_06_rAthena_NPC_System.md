# 🤖 rAthena NPC Scripting System

เอกสารฉบับนี้อธิบายโครงสร้างและการทำงานของระบบ NPC (Non-Player Character) ใน rAthena ซึ่งเขียนด้วยภาษา Script เฉพาะของ Athena (Athena Scripting Language)

---

## 📂 1. Structure & Location
ไฟล์ NPC ทั้งหมดจะถูกเก็บอยู่ในโฟลเดอร์ `npc/` โดยแบ่งเป็นหมวดหมู่ย่อย:
-   `npc/re/`: สคริปต์สำหรับระบบ Renewal (แพทช์ปัจจุบัน)
-   `npc/pre-re/`: สคริปต์สำหรับ Classic/Pre-Renewal
-   `npc/custom/`: **แนะนำให้เขียน NPC ใหม่ที่นี่** เพื่อไม่ให้ปนกับไฟล์หลักของระบบ

การเปิดใช้งาน NPC จะทำผ่านไฟล์ `.conf` (เช่น `npc/re/scripts_main.conf`) ซึ่งจะ `import` ไฟล์ .txt เข้ามาในระบบ

---

## 📝 2. Script Syntax (โครงสร้างภาษา)

### NPC Header
บรรทัดแรกของ NPC จะกำหนด **ตำแหน่ง** และ **หน้าตา** ของ NPC
```c
// Map,X,Y,Facing  Type  Name  SpriteID, { Code }
prontera,150,150,4	script	Healer	4_F_KAFRA,{
    // ... code ...
}
```
-   **Map, X, Y**: พิกัดที่ NPC ยืน (ใช้ `/where` ในเกมดูได้)
-   **Facing**: ทิศที่หันหน้า (0-7, โดย 0=ทิศเหนือ, 4=ทิศใต้)
-   **Type**: ประเภท (ส่วนใหญ่ใช้ `script`, หรือ `shop` สำหรับร้านค้า)
-   **Name**: ชื่อ NPC (แสดงผลในเกม)
    -   ใช้ `#` เพื่อซ่อนชื่อจริง (เช่น `Healer#prt` จะแสดงแค่ `Healer`)
-   **SpriteID**: รูปร่าง NPC (เลข ID Monster หรือชื่อ Constant เช่น `4_F_KAFRA`)

---

### 📦 3. Variables (ตัวแปร)
ตัวแปรใน rAthena แบ่งตาม **Scope** (ขอบเขต) และ **Lifetime** (อายุ) โดยดูจาก Prefix:

| Prefix    | Scope     | Description                          | Reset เมื่อ                      |
| :-------- | :-------- | :----------------------------------- | :----------------------------- |
| **(ไม่มี)** | Character | เก็บติดตัวละครถาวร (Save ลง DB)         | ไม่ Reset (จนกว่าจะลบตัว)         |
| **@**     | Character | ตัวแปรชั่วคราวของตัวละครนั้นๆ              | Logout                         |
| **$**     | Global    | ตัวแปรของ Server (ทุกตัวละครเห็นเหมือนกัน) | ไม่ Reset (เก็บใน SQL)           |
| **$@**    | Global    | ตัวแปรของ Server ชั่วคราว               | Restart Server                 |
| **.**     | NPC       | ตัวแปรเฉพาะของ NPC ตัวนั้น               | Restart Server / Reload Script |
| **#**     | Account   | เก็บติด ID (แชร์ทุกตัวละครใน ID เดียวกัน)   | ไม่ Reset                       |

**Suffix (ชนิดข้อมูล):**
-   ไม่มี (เช่น `@zeny`): เก็บตัวเลข (Integer)
-   `$` (เช่น `@name$`): เก็บข้อความ (String)

---

### 🛠️ 4. Common Commands (คำสั่งพื้นฐาน)

#### การสนทนา
-   `mes "Hello World";`: แสดงข้อความในกล่องสนทนา
-   `next;`:  ปุ่ม "Next" ให้กดไปหน้าถัดไป
-   `close;`: ปิดกล่องสนทนา (จบการทำงาน)
-   `select("Option A:Option B");`: สร้างเมนูให้เลือก

#### การจัดการผู้เล่น
-   `heal <hp>, <sp>;`: เติมเลือด/มานา (เช่น `heal 1000, 1000;`)
-   `warp "mapname", <x>, <y>;`: วาร์ปผู้เล่น
-   `getitem <id>, <amount>;`: ให้ไอเทม
-   `delitem <id>, <amount>;`: ลบไอเทม
-   `Zeny`: เป็นตัวแปรพิเศษ ใช้งานได้เลย (เช่น `if (Zeny < 100) ...`)

---

## 💡 5. Example: Healer NPC
ตัวอย่างโค้ด NPC ฮีลเลอร์แบบง่าย พร้อมคำอธิบาย

```c
prontera,156,180,4	script	MyHealer	4_F_KAFRA,{
    // 1. ทักทาย
    mes "[Healer]";
    mes "สวัสดีจ้ะ ต้องการฮีลไหม?";
    mes "ราคา 100 Zeny นะ";
    next;

    // 2. สร้างเมนูเลือก
    // switch จะตรวจสอบค่าที่ผู้เล่นเลือก (1=Yes, 2=No)
    switch(select("Yes, please.:No, thanks.")) {
        case 1:
            // 3. ตรวจสอบเงิน
            if (Zeny < 100) {
                mes "[Healer]";
                mes "เงินไม่พอนะ!";
                close;
            }
            // 4. หักเงิน และ ฮีล
            Zeny = Zeny - 100;
            percentheal 100, 100; // ฮีล 100% HP/SP
            mes "[Healer]";
            mes "เรียบร้อยจ้า!";
            close;
        case 2:
            // 5. ปฏิเสธ
            mes "[Healer]";
            mes "ไว้มาใหม่นะ";
            close;
    }
}
```

## 🔗 Reference
-   ไฟล์เอกสารฉบับเต็ม: `rathena/doc/script_commands.txt` (คู่มือศักดิ์สิทธิ์ของ Developer)
