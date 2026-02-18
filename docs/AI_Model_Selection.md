# AI Model Selection Strategy

เอกสารนี้รวบรวมรายชื่อ Model ที่เหมาะสมสำหรับโปรเจกต์ **Project-Mimir** โดยแบ่งตาม Environment และ Hardware Capability

## 1. Embedding Models (สำหรับ RAG & Vector Search)

เนื่องจากโปรเจกต์ต้องจัดการเอกสาร **ภาษาไทย** (Wiki, Game Guide) เป็นหลัก การเลือก Embedding Model ที่รองรับ Multilingual จึงสำคัญมาก

| Model Name          | Size    | Parameter | Max Context | เหมาะสำหรับ               | Recommend        |
| :------------------ | :------ | :-------- | :---------- | :---------------------- | :--------------- |
| **`bge-m3`**        | ~1.2 GB | 567M      | 8192        | **Thai / Multilingual** | 🏆 **Prod & Dev** |
| `nomic-embed-text`  | ~274 MB | 137M      | 8192        | English / General       | 🥈 Alternative    |
| `mxbai-embed-large` | ~670 MB | 335M      | 512         | English / Search        | -                |
| `all-minilm`        | ~46 MB  | 22M       | 512         | Extremely Low Res       | 🥉 Baseline       |

### 🛠️ Config ที่แนะนำ

#### 💻 Development (MacBook Air/Pro M3 - 16/24GB RAM)
*   **Model:** `bge-m3` (คุณภาพดีที่สุด คุ้มค่า memory ที่เสียไปเล็กน้อย)
*   **Concurrency:** 1-2 concurrent requests (ตอน Embedding data)
*   **Reason:** M3 รัน `bge-m3` ได้สบายมาก (Latency < 50ms) ไม่จำเป็นต้องลดไปใช้ `minilm` ยกเว้น Ram เต็มจริงๆ

#### 🚀 Production / Staging (Mac mini M4 Pro - 64GB RAM)
*   **Model:** **`bge-m3`**
*   **Optimization:**
    *   สามารถรัน Model นี้ค้างไว้ใน Memory ได้ตลอดเวลา (กิน VRAM แค่ ~2GB รวม Context overhead)
    *   **High Concurrency:** สามารถยิง Embedding request พร้อมกัน 10-20 thread ได้สบายๆ เพื่อทำ Data Ingestion ให้เสร็จเร็วขึ้น 10 เท่า
    *   **Context Window:** สามารถปรับ context length ให้เต็ม 8192 ได้โดยไม่ต้องกังวลเรื่อง OOM

---

## 2. Main LLM Models (สำหรับ Chat & Logic)

สำหรับ M4 Pro (64GB RAM) เรามีตัวเลือกที่ทรงพลังกว่า M3 มาก

| Model Name        | Size (Q4) | VRAM Usage | Speed (M4 Pro) | เหมาะสำหรับ                       | Recommend     |
| :---------------- | :-------- | :--------- | :------------- | :------------------------------ | :------------ |
| **`qwen2.5:32b`** | ~19 GB    | ~22 GB     | ~30-40 t/s     | **General Intelligence / Thai** | 🏆 **Primary** |
| `llama3.1:70b`    | ~40 GB    | ~42 GB     | ~15-20 t/s     | Complex Reasoning               | 🥈 Boss Logic  |
| `gemma2:27b`      | ~16 GB    | ~18 GB     | ~40-50 t/s     | Creative Writing                | 🥉 Alternative |
| `deepseek-r1:32b` | ~19 GB    | ~22 GB     | ~30-40 t/s     | **Reasoning / Logic**           | 🧠 Logic Only  |

### 🛠️ Config ที่แนะนำ

#### 💻 Development (MacBook Air/Pro M3)
*   **Primary:** `gemma:2b` หรือ `qwen2.5:3b` (เร็ว, เบาเครื่อง)
*   **Task-specific:** โหลด `qwen2.5:14b` มาเทสเฉพาะจุดที่ต้องการความฉลาด แล้ว unload ออก

#### 🚀 Production / Staging (Mac mini M4 Pro - 64GB RAM)
*   **Architecture:** **Hybrid Model Strategy**
    1.  **Main Agent (Fast):** ใช้ **`qwen2.5:32b`** (เหลือพื้นที่ RAM 20GB+)
        *   ฉลาดกว่า GPT-3.5 มาก
        *   ตอบภาษาไทยเป็นธรรมชาติ
        *   Latency ต่ำมากบน M4 Pro
    2.  **Complex Logic (Optional):** สลับไปใช้ `llama3.1:70b` (4-bit quant) ได้ถ้าจำเป็น (แต่จะเหลือ RAM น้อยสำหรับส่วนอื่น)
    3.  **Parallelism:** รัน `qwen2.5:32b` พร้อมกับ `bge-m3` และ Docker services (MariaDB/Qdrant) ได้พร้อมกันโดยเครื่องไม่หน่วง

---

## 3. สรุปคำสั่งสำหรับติดตั้ง (Environment Setup)

### สำหรับ Mac mini M4 Pro (64GB)

```bash
# 1. Embedding Model (Best for Thai)
ollama pull bge-m3

# 2. Main Intelligence Model (Best Balance)
ollama pull qwen2.5:32b

# 3. (Optional) Reasoning Model
ollama pull deepseek-r1:32b
```

### สำหรับ MacBook M3 (Dev)

```bash
# 1. Embedding Model
ollama pull bge-m3

# 2. Small Model for Testing
ollama pull gemma:2b
```
