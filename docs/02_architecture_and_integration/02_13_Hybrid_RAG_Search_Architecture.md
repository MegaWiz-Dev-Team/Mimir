# Mimir Multi-Engine RAG Retrieval Architecture

การดึงข้อมูลสำหรับ RAG ในระดับ Enterprise สมัยใหม่ ไม่สามารถพึ่งพาแค่ Vector Database อย่างเดียวได้อีกต่อไป Mimir จึงต้องถูกออกแบบให้มี **Ensemble Retrieval (ระบบดึงข้อมูลแบบผสมผสาน)** เพื่อดึงจุดเด่นของแต่ละ Database ออกมา และส่ง Context ที่แม่นยำที่สุดกลับไปยัง Bifrost Agents

---

## 🏗️ 1. Core Architecture Flow

เมื่อระบบได้รับ `query` จาก BIfrost Agents (เช่น Mimir `search_knowledge` tool) จะมีขั้นตอนหลัก 4 ขั้นตอนดังนี้:

### Step 1: Query Router & Entity Extraction (Analyzer)
รับคำถามมาวิเคราะห์ว่าต้องการข้อมูลรูปแบบไหน:
*   **LLM Extraction:** รัน Prompt เล็กๆ (ผ่าน Heimdall) เพื่อดึง Entities หลักจากคำถาม เช่น `[Patient: John Doe, Drug: Aspirin]` (สำหรับ Graph)
*   **Query Rewrite:** ปรับแต่งคำถามให้อ่านง่ายขึ้น หรือแตกคำถามสำหรับ Vector Search (Dense)

### Step 2: Parallel Retrieval (ค้นหา 3 แหล่งพร้อมกัน)
เพื่อให้ Latency ต่ำที่สุด Mimir จะยิง Request ค้นหาไปยังทั้ง 3 แหล่งแบบ Asynchronous ทันที:

1.  **Qdrant (Vector Similarity Search)**
    *   **จุดประสงค์:** หา "ความเกี่ยวข้องทางความหมาย" (Semantic) ในระดับย่อหน้าหรือ Chunk
    *   **วิธีการ:** เข้าถึง `/collections/mimir/points/search` ดึงมา 10 Chunks ที่มีความหมายตรงที่สุด
    *   **ข้อดี:** เก่งที่การอธิบายคอนเซปต์ซับซ้อน / Unstructured Data ที่ไม่มีความเกี่ยวโยงโดยตรง

2.  **SQL Graph (Relational Knowledge Graph)**
    *   **จุดประสงค์:** หา "ความสัมพันธ์ระหว่างสิ่งต่างๆ" (Relational & Multi-hop) แบบมีโครงสร้าง
    *   **วิธีการ:** รัน SQL JOIN Query บน Edge List (kg_entities/kg_relations) ใน MariaDB ด้วยคำหรือ Entity ที่ดึงมา
    *   **ข้อดี:** ตอบคำถามข้าม Entities ได้แม่นยำ 100% ป้องกันภาพลวงตา (Hallucination) แถมรักษา Multi-tenant แยกกันง่ายดายวัง

3.  **PageIndex (Reasoning-based RAG)**
    *   **จุดประสงค์:** ดึงความเข้าใจ "บริบททั้งหน้า" ไม่ใช่แค่บรรทัดสั้นๆ
    *   **วิธีการ:** ยิง API ไปที่ `http://pageindex:8600/reason` เพื่อให้ Sidecar ใช้ LLM กวาดข้อมูลแบบหน้าระดับ Macro สรุปเอาเนื้อหาภาพกว้างและเหตุผลมา
    *   **ข้อดี:** ถ้าเอกสารมีความยาวมากๆ แง่มุมระดับหน้าจะช่วยให้สรุปประเด็นที่ไม่โดนตัดขาดด้วย Chunking ของ Qdrant

### Step 3: Aggregation & De-duplication
นำผลลัพธ์ (Chunks, Graph Nodes, Page Summaries) ทั้งสามกองมารวมเป็นกองเดียวกัน (Merged Pool)
*   **De-duplication:** ตัดเนื่อหาที่ได้ซ้ำซากออก (อย่างเช่นเนื้อหาที่ดึงได้จาก Qdrant มีอยู่ใน PageIndex Summary อยู่แล้ว)
*   **Context Formatting:** เรียบเรียงให้เป็น Text Block เช่น `[Source: Knowledge Graph] Relationship: A treated B`, `[Source: Qdrant] Text: ...`

### Step 4: Re-ranking (Cross-Encoder / LLM Reranker)
*   นำ Merged Pool ส่งให้ Reranking Model ให้คะแนนความเกี่ยวข้องเทียบกับคำถามหลักอีกครั้ง (Score 0-1)
*   นำแค่ Top-K (เช่น 5 ตัวที่มีคะแนนดีที่สุดจาก 30 ตัวที่ดึงมา) ส่งกลับเป็น Response ให้ Agent

---

## ⚙️ 2. Mimir Implementation Design (Rust / Axum)

ภายใน Mimir API ตัว Source Code ควรถูกแบ่งเป็น Modules ดังนี้:

```rust
// โครงสร้างภายใน Mimir
src/
 ├── retrieval/
 │   ├── qdrant.rs    // Implements VectorSearch Trait
 │   ├── graph.rs     // Implements GraphSearch Trait via SQL (MariaDB)
 │   ├── pageindex.rs // Implements ReasoningSearch Trait
 │   └── ensemble.rs  // Asynchronously spawns all 3 searches + Rerank
 ├── models/
 │   ├── query.rs
 │   └── document.rs
 └── routes/
     └── search.rs    // POST /api/search
```

## 🛠️ 3. How to Execute (วิธีการทำงาน)

1.  **Ingestion Phase (ขาเข้า):** เมื่อมีการอัปโหลดเอกสาร จะต้อง:
    *   ตัดลง Qdrant (Vector Embedding)
    *   ทำ Entity Extraction ส่ง Graph Node เข้าตาราง Relation ใน MariaDB
    *   ส่งให้ PageIndex Summary
2.  **Search Phase (ขาออก):** เรียกใช้งาน `ensemble.rs` ดึงข้อมูลกลับมาให้ Agent
3.  **Bifrost Integration:** Bifrost `MimirAgent` ยังคงใช้ท่าเดิมคือ `mimir.search_knowledge("query")` แต่ได้ผลลัพธ์ที่ឆ្លาดรอบด้านขึ้นทันที

---

## 🚨 4. Gap Analysis (ส่วนที่ขาดหายไปและต้องแก้ไข)

จากการตรวจสอบ Source Code ของ Mimir ในปัจจุบัน (`ro-ai-bridge` และ `ro-ai-dashboard`) พบว่ามีช่องโหว่และ Tech Debt หลายจุดที่ต้องทำก่อนจะไปถึง Design ด้านบนได้:

### 🔴 Backend Gaps (Mimir Rust API)
1. **Fake Vector Search:** ใน `src/routes/tenant_query.rs` ฟังก์ชัน `vector_search()` ไม่ได้ยิงไปหา Qdrant จริงๆ! แต่มันถูก Hardcode ให้ดึงข้อมูลแบบสุ่ม `SELECT ... LIMIT 5` จาก MariaDB แทน (ส่วนที่ต่อกับ Qdrant จริงๆ ดันไปอยู่ใน `vector.rs` แยกต่างหากและยังไม่เชื่อมกับ Tenant Query)
2. **Sequential Blocking (คอขวดของ PageIndex):** การค้นหาแบบ `tree` มีการใช้ `for` loop วนทีละ Document เพื่อยิงไปหา `pageindex:8600` แบบเรียงตัว ทำให้ถ้า Tenant มีเอกสาร 1,000 ไฟล์ การค้นหาครั้งเดียวอาจใช้เวลามหาศาล ต้องแก้เป็น Asynchronous `join_all` (Parallel)
3. **Missing Graph Integration:** ในระบบ Hybrid Query ของ Tenant ปัจจุบัน ยังไม่มีโค้ดส่วนไหนเลยที่ดึงข้อมูลจาก SQL Graph มีเพียงแค่ Tree กับ Vector เท่านั้น (อัปเดต: ปัจจุบันเชื่อมต่อแล้วใน graph.rs)
4. **No Reranker Model:** ปัจจุบันระบบแค่เอา String คำตอบจากทุกที่มาต่อกันตรงๆ (`all_answers.join("\n\n")`) ซึ่งจะทำให้ LLM Context Window ล้นและไม่ได้ข้อมูลที่ดีที่สุด ต้องเพิ่ม Layer ของ Cross-Encoder ให้คะแนนก่อนรวม

### 🟡 Frontend Gaps (Mimir Dashboard Next.js)
1. **No RAG Ensemble Playground:** ในหน้า Playground ปัจจุบันมีให้เลือกลอง LLM Model แต่ไม่มีหน้าให้ลอง "Search Strategy" ผู้ใช้ไม่สามารถพิมพ์คำถามแล้วเห็นภาพแยกได้ว่า ข้อมูลมาจาก Qdrant เท่าไหร่? มาจาก Graph คือ Node ไหน? และ Reranker ให้คะแนนความน่าเชื่อถือเท่าไหร่
2. **Missing Ingestion Status สำหรับ Graph:** ในหน้าอัปโหลด Knowledge มีให้เห็นแค่สถานะ Chunking ลง Qdrant แต่ยังขาด UI ที่บอกว่ากระบวนการ "Entity Extraction" เพื่อลง Graph สำเร็จไปกี่ %
3. **No Weighting Controls:** Dashboard ยังขาดหน้า Settings ที่ให้แอดมินเข้าไปปรับน้ำหนักความสำคัญ (เช่น Vector 70%, Graph 30%) ได้ด้วยตัวเอง
