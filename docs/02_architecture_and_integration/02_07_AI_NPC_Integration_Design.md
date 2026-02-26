# 🧠 AI Agent Integration Analysis: Healer NPC Case Study

เอกสารฉบับนี้วิเคราะห์แนวทางการเปลี่ยน **Healer NPC** แบบเดิม (Rule-Based) ให้เป็น **AI Agent** ที่มีความคิด การตัดสินใจ และบทสนทนาที่ยืดหยุ่น

---

## 🆚 1. Comparison: Rule-Based vs AI-Based

### 🤖 แบบเดิม (Rule-Based)
NPC ทำงานตามเงื่อนไข `if-else` ที่เขียนไว้ตายตัว
-   **Conversation:** "สวัสดี ราคา 100 Zeny" (พูดเหมือนเดิมทุกครั้ง)
-   **Decision:** ถ้าเงินพอ -> ฮีล, ถ้าเงินไม่พอ -> ไม่ฮีล
-   **Logic:**
    ```c
    if (Zeny >= 100) {
        Zeny -= 100;
        percentheal 100, 100;
    } else {
        mes "เงินไม่พอ";
    }
    ```

### 🧠 แบบใหม่ (AI-Based)
NPC ใช้ LLM (AI) ในการตัดสินใจจาก "Context" (สภาพแวดล้อมและข้อมูลผู้เล่น)
-   **Conversation:** ทักทายตามอารมณ์/ความสนิท หรือแซวผู้เล่นถ้าเลือดเต็ม
-   **Decision:** อาจจะฮีลให้ฟรีถ้าชอบหน้า, หรือคิดแพงขึ้นถ้าผู้เล่นกวนประสาท
-   **Logic Flow:**
    `[Player Data] -> [AI Engine] -> [Decision: Talk/Action] -> [rAthena Execute]`

---

## 🔄 2. AI Integration Workflow

เพื่อให้ NPC "คิด" ได้ เราต้องสร้างท่อส่งข้อมูล (Pipeline) ระหว่าง rAthena และ AI Bridge

### Step 1: Input (รวบรวมข้อมูลฝั่ง rAthena)
สคริปต์ NPC ต้องเก็บข้อมูลผู้เล่นเพื่อส่งให้ AI ใช้ตัดสินใจ:
1.  **Player State:** HP, SP, BaseLevel, Job (ผู้เล่นใกล้ตายไหม? เป็นอาชีพอะไร?)
2.  **Wallet:** Zeny (รวยหรือจน?)
3.  **Identity:** Name, Reputation (เคยคุยกันมาก่อนไหม? - จำผ่าน Memory/Database)
4.  **Message:** สิ่งที่ผู้เล่นพิมพ์คุย (`input .@msg$;`)

### Step 2: Processing (ฝั่ง AI Bridge)
AI จะได้รับ JSON Context:
```json
{
  "npc_role": "Healer",
  "personality": "Kind but strict about money",
  "player": { "name": "HeroA", "hp_percent": 10, "zeny": 50 },
  "message": "ช่วยด้วย! จะตายแล้วแต่ไม่มีเงิน"
}
```
**AI Thought Process:**
> "ผู้เล่นใกล้ตายมาก (HP 10%) แต่น่าสงสารไม่มีเงิน... ฉันเป็นคนใจดี งั้นรอบนี้ช่วยฟรีละกัน แต่ต้องเตือนนะ"

**AI Output:**
```json
{
  "response_text": "ตายจริง! แผลเหวอะหวะขนาดนี้... เอ้า! รอบนี้รักษาให้ฟรีนะ แต่คราวหน้าต้องระวังตัวกว่านี้นะเจ้าหนู",
  "action": "HEAL_FREE"
}
```

### Step 3: Execution (ฝั่ง rAthena)
NPC รับค่ากลับมาแล้วทำงานตามคำสั่ง:
-   แสดงข้อความ: `mes "ตายจริง! ...";`
-   ทำตาม Action:
    -   `HEAL_FREE`: `percentheal 100, 100;`
    -   `HEAL_PAID`: ตัดเงิน `Zeny -= 100;` แล้วฮีล
    -   `REJECT`: `mes "ไปหาเงินมาก่อนไป๊!";`

---

## 📝 3. Conceptual Script Structure

ตัวอย่างโครงสร้างสคริปต์เมื่อเชื่อมต่อ AI (สมมติว่ามีคำสั่ง `call_ai_bridge`)

```c
prontera,156,180,4	script	AI_Healer	4_F_KAFRA,{
    // 1. รับข้อความจากผู้เล่น
    mes "[Healer]";
    mes "สวัสดีจ้ะ มีอะไรให้ช่วยไหม?";
    input .@player_msg$; // ผู้เล่นพิมพ์: "ขอฮีลหน่อย พี่สาวคนสวย"

    // 2. รวบรวม Context
    .@hp_rate = (Hp * 100) / MaxHp;
    .@zeny = Zeny;

    // 3. ส่งให้ AI คิด (สมมติฟังก์ชัน connection)
    // ส่ง: "User:ขอฮีลหน่อย, HP:10%, Zeny:5000"
    // รับ: Action String กลับมา
    .@ai_response$ = call_ai_bridge(.@player_msg$, .@hp_rate, .@zeny);

    // สมมติ AI ตอบกลับมาเป็น Format: "ACTION|Message"
    // ตัวอย่าง 1: "HEAL|ได้จ้ะ หน้าตาดีแบบนี้พี่ลดให้"
    // ตัวอย่าง 2: "REJECT|ปากหวานนะแต่พี่ไม่หลงกลหรอก จ่ายมาซะดีๆ"

    .@action$ = extract_action(.@ai_response$);
    .@talk$   = extract_message(.@ai_response$);

    // 4. แสดงบทสนทนา
    mes "[Healer]";
    mes .@talk$;

    // 5. ทำ Action
    if (.@action$ == "HEAL") {
        percentheal 100, 100;
        specialeffect2 EF_HEAL2;
    }
    else if (.@action$ == "CHARGE_AND_HEAL") {
        if (Zeny >= 100) {
            Zeny -= 100;
            percentheal 100, 100;
        }
    }
    
    close;
}
```

---

## 🎯 4. Advanced Actions (การเดิน/การกระทำ)

นอกจากฮีล AI สามารถสั่งให้ NPC ทำอย่างอื่นได้ผ่านคำสั่ง rAthena:

1.  **การเดิน (Walking/Navigation):**
    -   ถ้า AI ตัดสินใจ "เดินหนี" หรือ "นำทาง"
    -   ใช้คำสั่ง `unitwalk <GID>, <x>, <y>;`
    -   *Use Case:* ผู้เล่นชวน NPC ไปเดินเล่น หรือ NPC เดินไปหยิบของ

2.  **ท่าทาง (Emotes):**
    -   AI ส่งอารมณ์ (Happy, Angry)
    -   ใช้คำสั่ง `emotion <id>;` (เช่น `/omg`, `/thx`)

3.  **Quest Trigger:**
    -   AI ตรวจสอบประวัติคุยแล้วพบว่าผู้เล่นสนใจเรื่องตำนาน
    -   Action: `START_QUEST_A` -> สคริปต์สั่ง `set quest_variable, 1;`

## ✅ Summary
การใช้ AI กับ NPC ไม่ใช่ระบุทุกอย่างใน Code แต่คือการ **"โยน Context ให้ AI ตัดสินใจ แล้วรับ Action กลับมา Execute"** ทำให้ NPC ดูมีชีวิตชีวาและคาดเดาไม่ได้ (Non-deterministic/Dynamic)
