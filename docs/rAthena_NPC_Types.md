# 🎭 rAthena NPC Types & Properties

เอกสารฉบับนี้รวบรวมประเภทของ NPC (Non-Player Character) ที่มีใน rAthena และคุณสมบัติเด่นของแต่ละแบบ เพื่อเลือกใช้ให้เหมาะกับงาน

---

## 📋 Summary Table

| Type         | Definition | Main Interaction | Properties                        |
| :----------- | :--------- | :--------------- | :-------------------------------- |
| **Script**   | NPC ทั่วไป   | Click / Touch    | พูดคุย, ทำเควส, ฮีล, รัน Logic         |
| **Shop**     | ร้านค้า      | Click            | เปิดหน้าต่างซื้อ-ขาย (ห้ามรัน Script อื่น) |
| **Warp**     | จุดวาร์ป     | Walk into        | ย้ายผู้เล่นไป map อื่นทันที               |
| **Monster**  | มอนสเตอร์   | Attack / Die     | เกิดใหม่ได้, ดรอปของ, มี AI ต่อสู้       |
| **Floating** | NPC ล่องหน  | Timer / Auto     | ไม่มีรูปร่าง, ใช้รัน Global Event/Time  |

---

## 1. Script NPC (Interaction)
NPC หลักที่เจอบ่อยที่สุด ใช้สำหรับสร้าง Interactive Content

**Syntax:**
```c
map,x,y,face	script	Name	SpriteID,{ <code...> }
```

**คุณสมบัติเด่น:**
-   **Conversation:** ใช้คำสั่ง `mes`, `input`, `select` ได้
-   **Trigger:**
    -   **Click:** เมื่อผู้เล่นคลิก (OnTouch)
    -   **Area:** เมื่อเดินผ่าน (กำหนด `triggerX/Y`)
    -   **Timer:** สั่งให้ทำงานเมื่อเวลาผ่านไป
-   **AI Integration:** เหมาะที่สุดสำหรับการเชื่อมต่อกับ AI Agent

---

## 2. Shop NPC (Transaction)
NPC สำหรับการซื้อขายแลกเปลี่ยน (ระบบ Hardcode มาให้แล้ว)

**Syntax:**
```c
// Shop ปกติ (ใช้ Zeny)
map,x,y,face	shop	Name	SpriteID,<itemid>:<price>,...

// Cash Shop (ใช้ Cash Points)
map,x,y,face	cashshop	Name	SpriteID,<itemid>:<price>,...

// Item Shop (ใช้ไอเทมแลกไอเทม)
map,x,y,face	itemshop	Name	SpriteID,<cost_item>,<itemid>:<price>,...
```

**ข้อควรระวัง:**
-   **ไม่สามารถ** ใส่ Script Logic (`if-else`) ลงไปใน Shop ได้
-   ถ้าคลิกแล้วเปิดหน้าต่างขายของทันที

---

## 3. Warp NPC (Transportation)
จุดวาร์ปที่เห็นเป็นวงวนๆ บนพื้น

**Syntax:**
```c
// from_map, x, y, face	warp	Name	span_x, span_y, to_map, to_x, to_y
prontera,150,150,0	warp	ToMorocc	2,2,morocc,156,97
```

**คุณสมบัติเด่น:**
-   **Span:** กำหนดความกว้างของจุดวาร์ป (เช่น 2x2 ช่อง)
-   **Instant:** เหยียบปุ๊บไปปั๊บ (ยกเว้น `warp2` จะจับตัวละครหายตัวได้ด้วย)

---

## 4. Monster NPC (Combat)
การประกาศจุดเกิดมอนสเตอร์แบบถาวรบนแมพ

**Syntax:**
```c
// map,x,y	monster	Name	MobID,Amount,Delay1,Delay2,Event
prontera,0,0	monster	Poring	1002,1,0,0
```

**คุณสมบัติเด่น:**
-   **Respawn:** ตายแล้วเกิดใหม่ตาม Delay ที่ตั้ง
-   **Event:** สามารถผูก Script ให้ทำงานเมื่อตายได้ (`OnMyMobDead:`)

---

## 5. Floating / Function NPC (System)
NPC ที่ไม่มีตัวตนในเกม (ตั้งชื่อ map เป็น `-`) ใช้สำหรับระบบหลังบ้าน

**Syntax:**
```c
-	script	GlobalEvent	-1,{
OnInit:
    // ทำงานทันทีที่ Server เปิด
    end;
OnClock1200:
    // ทำงานตอนเที่ยงวัน
    announce "Lunch Time!", 0;
    end;
}
```

**Use Case:**
-   ระบบประกาศเซิร์ฟเวอร์
-   ระบบรีเซ็ตตัวแปรรายวัน
-   AI Manager ที่คอยตรวจสอบ World State
