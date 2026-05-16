# RefGraph-Mimir Integration: Decision Framework

**ช่วยให้คิด** ว่าเลือก Option ไหนเหมาะสม

---

## คำถาม 1: อนาคต (Future-proof)

**S2 (Medical), S3 (Legal) ต่อไป จะเขียน RefGraph ใหม่หรือรีใช้?**

```
Option A (Sequential):
  RefGraph → JSON → Mimir
  ✅ RefGraph เป็น standalone tool
  ✅ ใช้ได้ทั้ง insurance, medical, legal
  ✅ Team ต่อไปเขียน: refgraph --domain medical

Option B (Embedded in Mimir):
  ⚠️ RefGraph bind กับ Mimir
  ❌ Medical pipeline ต้อง modify Mimir code ใหม่
  ❌ ไม่ reusable ข้ามโปรเจค

Option C (Separate Service):
  ✅ RefGraph เป็น service แยก
  ✅ Medical/Legal/Finance ใช้ service เดิม
  ✅ Reusable สุด
  ⚠️ แต่ต้อง build more infrastructure
```

**→ ถ้า S2/S3 ใช้ RefGraph ด้วย = เลือก A หรือ C**

---

## คำถาม 2: ปัญหาจริงที่เจอในการทำงาน

**เคยเจอเรื่องไหนที่ทำให้ integration ลำบาก?**

```
ขอ Option แบบไหนจึงจะ smooth:

A. DebugB ได้เห็นข้อมูล:
   - Mimir ไม่ ingest → ดู consolidated.json ดึง error
   - ง่าย debug ทีละ step
   
B. ต้อง ingest ทันที (atomic):
   - ไม่อยากรอ 2 step แยก
   - ต้องการ atomic transaction
   - เดี๋ยวค้างระหว่างทาง

C. ต้องการ scale ทีหลัง:
   - อยากรัน RefGraph parallel (10 machines)
   - Mimir scale separately
   - Needs microservices
```

**→ เคยเจอปัญหาไหน?**

---

## คำถาม 3: Team & Deployment

**ทีหลัง (S2, S3) จะ maintain พร้อมกันหรือแยก?**

```
Option A (Sequential):
  - Team A ทำ RefGraph (Rust)
  - Team B ทำ Mimir (Rust)
  - Orchestration script ใครเขียน?
  ⚠️ Need clear ownership

Option B (Embedded):
  - 1 codebase (Mimir)
  - 1 team maintain ทั้งหมด
  ✅ Clear ownership
  ❌ Mimir team ต้อง know RefGraph

Option C (Separate Service):
  - Team A owns RefGraph service
  - Team B owns Mimir service
  - Team C owns orchestration
  ⚠️ 3 teams coordinate
  ✅ Clear boundaries
```

**→ ทีหลัง collaborate ยังไง?**

---

## คำถาม 4: Performance vs Simplicity

**ไหนสำคัญกว่า?**

```
Option A (Sequential):
  ✅ Simplicity: ต่อ JSON, ต่อ curl, ต่อ API
  ⚠️ Performance: 
    - Write JSON disk
    - Read JSON disk
    - HTTP POST
    - Total: ~3-4 min (acceptable)

Option B (Embedded):
  ✅ Performance: in-memory, atomic, fast (~2 min)
  ⚠️ Simplicity: modify Mimir code, more coupling

Option C (Microservices):
  ⚠️ Performance: network latency add up
  ✅ Scalability: can parallelize
  ⚠️ Simplicity: most complex
```

**→ S1 June 2 deadline เร่งหรือไม่? Performance critical?**

---

## คำถาม 5: ทำง่ายที่สุด (May 29)

**ถ้าต้องเลือก ให้จบใน 2 ชั่วโมง?**

```
Option A: ✅ Possible in 2 hours
  - Write bash script
  - Test with sample data
  - Done

Option B: ⚠️ 2-3 hours (tight)
  - Need to understand Mimir codebase
  - Integrate RefGraph lib
  - Test integration
  - May find surprises

Option C: ❌ Not possible in 2 hours
  - Need to add HTTP layer to RefGraph
  - Build orchestrator service
  - Test orchestration
  - 4-5 hours minimum
```

**→ จะ commit 2-3 ชั่วโมง May 29 ได้ไหม?**

---

## คำถาม 6: Deployment Reality

**จะ deploy ยังไง June 2?**

```
Option A (Sequential):
  - Deploy refgraph binary (static file)
  - Deploy bash/Python script (orchestration)
  - Keep running Mimir service
  
  Deployment: Easy ✅
  ```bash
  docker run refgraph --input data.jsonl --output out.json
  curl -X POST mimir-api --data @out.json
  ```

Option B (Embedded):
  - Modify Mimir + RefGraph code
  - Deploy new Mimir image
  - Rollback if issue: revert code + redeploy
  
  Deployment: Medium ⚠️
  ```bash
  docker build . -t mimir:v2.2.0 (includes RefGraph)
  docker run mimir:v2.2.0
  ```

Option C (Separate):
  - Deploy RefGraph service
  - Deploy orchestrator service
  - Deploy Mimir service
  - 3 things to manage
  
  Deployment: Complex ❌
  ```bash
  docker-compose up -d (3 services)
  debug if one fails
  ```
```

**→ Deployment simplicity สำคัญไหม?**

---

## ตัวอย่างจากการทำงาน

**จากการ build RefGraph ที่แล้ว:**

```
✅ RefGraph standalone Rust binary ทำไป
   - ใช้ได้ standalone (cargo run)
   - มี CLI args (--input, --output)
   - Output ทำให้ compatible กับ Mimir API

→ ให้ดำเนิน Option A ก็เหมาะเพราะ:
  - โค้ด RefGraph ตั้งไว้สำหรับ standalone
  - ต้องเพิ่มแค่ orchestration script
  - Minimal changes ต่อ Mimir
```

---

## Decision Tree

**ตามลำดับสำคัญ:**

### ถ้า Priority = Reusability (S2, S3 ใช้ RefGraph ด้วย)
```
→ Option A or C
→ ฉันแนะ A (simpler)
```

### ถ้า Priority = Speed of Implementation (May 29 deadline)
```
→ Option A (2 hrs)
```

### ถ้า Priority = Performance (atomic, fast)
```
→ Option B or C
→ But B faster to implement
```

### ถ้า Priority = Simple Deployment (June 2, don't break)
```
→ Option A (script + binary)
```

### ถ้า Priority = Long-term Scalability (parallel processing)
```
→ Option C (but costs 4-5 hrs now)
```

---

## My Analysis (based on what you said)

**Evidence that points to Option A:**

1. **You said: "อย่าลืม Heimdall + Laminar"**
   - Shows you care about clean architecture
   - Option A = cleanest (each component independent)

2. **You said: "เรา execute คนเดียว"**
   - Option A = simplest for solo (script + curl)
   - No complex Mimir modification needed

3. **You're focused on May 19-28 RefGraph**
   - Then May 29-30 bridge
   - Option A uses RefGraph as-is (no rework)

4. **You asked: "pipeline ใหม่ หรือแก้ของเดิม"**
   - Implies you prefer new separate pipeline
   - = Option A thinking

5. **Asgard principle: "If Rust can do it, use Rust"**
   - Both RefGraph + Mimir are Rust
   - But need clean separation
   - Option A enforces separation

---

## Recommendation Path

**I recommend this sequence of thinking:**

1. **If you plan S2/S3 with RefGraph:**
   → Choose A (reusable foundation)

2. **If you DON'T plan reuse:**
   → Choose B (simpler May 29, faster ingest)

3. **If you need massive scale:**
   → Choose C (but risky for June 2)

---

## Questions to Help You Decide

1. **Will medical (S2) also use RefGraph, or completely different?**
   - Yes → A
   - No → B or C

2. **Can May 29 orchestration script be simple bash? Or needs robustness?**
   - Simple bash OK → A
   - Needs error handling/logging → B

3. **Is Mimir ingest API ready now, or need to build?**
   - Ready → A
   - Not ready → B

4. **How important is "atomic" consolidate + ingest operation?**
   - Not important → A
   - Very important → B or C

5. **If something breaks June 2, can you revert easily?**
   - Yes (A: just change script or binary) → A
   - No (B: need redeploy, C: 3 services) → A

---

## My Strong Recommendation

**Option A (Sequential Pipeline)** because:

1. ✅ You already have RefGraph standalone
2. ✅ You work solo (script simpler than integrated code)
3. ✅ Clean architecture (independent services)
4. ✅ Enables future reuse (S2, S3)
5. ✅ Can finish May 29-30 (2-3 hours)
6. ✅ Easy to debug June 2 if issues
7. ✅ Aligns with Rust-first principle (two services)
8. ✅ Matches your statement: "new pipeline" (not "edit existing")

**But before I finalize plan:**

**Which 2 questions matter most to you?**

A. Future reuse (S2 medical)?
B. May 29 timeline (how much time)?
C. Performance (atomic vs sequential)?
D. Deployment simplicity?
E. Something else?

เลือกสอง คำถาม ที่คิดว่าสำคัญสุด แล้วผมจะ lock in architecture ครับ
