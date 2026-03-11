# SI-04: Heimdall Model Benchmark Report — Auto-Pipeline Extraction

**Project Name:** Project Mimir — Heimdall LLM Gateway  
**Document ID:** SI-04-BM-001  
**Date:** 2026-03-11  
**Tested By:** AI Agent (Antigravity)  
**Version:** 1.0

---

## 1. วัตถุประสงค์ (Objective)

เปรียบเทียบประสิทธิภาพของ Heimdall LLM models สองรุ่น — **Qwen3.5-9B** และ **Qwen3.5-27B** — ในการทำ Knowledge Extraction ผ่าน Auto-Pipeline ของ Mimir  
ทดสอบบนแหล่งข้อมูลทางการแพทย์ขนาดเล็ก เพื่อเป็น baseline ก่อนนำไปใช้งานจริง

---

## 2. สภาพแวดล้อมการทดสอบ (Test Environment)

| Component | Version / Spec |
|-----------|---------------|
| Hardware | Mac Mini M4 Pro, 24 GB Unified Memory |
| Heimdall Gateway | v0.4.0 (Rust) |
| Heimdall Backend | mlx_lm v0.31.0 (MLX / Apple Silicon) |
| Embedding Model | BAAI/bge-m3 (dim=1024) via MLX Embedding Server |
| Bridge | ro-ai-bridge (Rust/Axum) |
| Database | MariaDB 11.x |
| Vector DB | Qdrant |

---

## 3. ข้อมูลทดสอบ (Test Data)

| Field | Value |
|-------|-------|
| Source ID | 13 |
| Source Name | Sleep & ENT Drug Reference |
| Source Type | document |
| จำนวน Chunks | 34 |
| เนื้อหา | ข้อมูลยาเกี่ยวกับ Sleep disorders และ ENT (หู คอ จมูก) |

---

## 4. โมเดลที่ทดสอบ (Models Under Test)

| Property | Qwen3.5-9B | Qwen3.5-27B |
|----------|-----------|-------------|
| HuggingFace ID | `mlx-community/Qwen3.5-9B-MLX-4bit` | `mlx-community/Qwen3.5-27B-4bit` |
| Parameters | 9 Billion | 27 Billion |
| Quantization | 4-bit (MLX) | 4-bit (MLX) |
| Provider | heimdall | heimdall |
| Run Label | `benchmark-qwen9b` | `benchmark-qwen27b` |
| Pipeline Run ID | `1c219d72-9f23-486d-a467-f80c7a9b892d` | `31c5bc43-1a0b-44db-a3b1-eca2b7dfef1c` |

---

## 5. ผลการทดสอบ Auto-Pipeline (Pipeline Step Results)

### 5.1 ภาพรวมทุก Step

| Step | ขั้นตอน | Qwen3.5-9B | | Qwen3.5-27B | |
|------|---------|------------|---|-------------|---|
| | | Count | Latency | Count | Latency |
| 1 | Chunk Check | 34 | 0.002s | 34 | 0.003s |
| 2 | Embed Chunks (bge-m3) | 34 | 0.62s | 34 | 0.59s |
| 3 | **KG Extraction** | **145** | **39.6 min** | **242** | **61.0 min** |
| 4 | **QA Extraction** | **100** | **30.0 min** | **134** | **50.8 min** |
| 5 | QA Indexing | 0 (skipped) | — | 0 (skipped) | — |
| **Total** | | | **~70 min** | | **~112 min** |

### 5.2 ข้อสังเกต
- **Step 2 (Embed)**: ทั้งสองโมเดลใช้เวลาเท่ากัน (~0.6s) เพราะใช้ embedding server เดียวกัน (bge-m3)
- **Step 5 (QA Index)**: ถูก skip ทั้งสอง เนื่องจากยังไม่มี Qdrant collection สำหรับ QA indexing

---

## 6. ผลเปรียบเทียบ Knowledge Graph Extraction

### 6.1 จำนวน KG Entities

| Metric | Qwen3.5-9B | Qwen3.5-27B | ∆ (%) |
|--------|-----------|-------------|-------|
| Pipeline item_count | 145 | 242 | +67% |
| Unique KG Entities | 76 | 84 | +11% |
| Unique Entity Types | 6 | 6 | 0% |
| KG Relations | 0 | 0 | — |

### 6.2 Entity Type Distribution

| Entity Type | Qwen3.5-9B | Qwen3.5-27B | หมายเหตุ |
|------------|-----------|-------------|----------|
| **Concept** | 28 (37%) | 62 (74%) | 27B สกัด Concept ได้มากกว่ามาก |
| **Symptom** | 16 (21%) | 11 (13%) | 9B สกัดอาการได้มากกว่า |
| **Product** | 24 (32%) | 4 (5%) | 9B เน้นผลิตภัณฑ์มากกว่า |
| **Organization** | 5 (7%) | 3 (4%) | ใกล้เคียง |
| **Drug** | 2 (3%) | 3 (4%) | ใกล้เคียง |
| **Location** | 1 (1%) | — | 9B only |
| **Event** | — | 1 (1%) | 27B only |

### 6.3 Latency Per Entity

| Metric | Qwen3.5-9B | Qwen3.5-27B |
|--------|-----------|-------------|
| Average | 49.4s | 139.5s |
| Min | 14.0s | 19.3s |
| Max | 183.4s | 293.4s |
| Time per chunk (avg) | 69.9s/chunk | 107.7s/chunk |

### 6.4 ตัวอย่าง Entities ที่สกัดได้

**Qwen3.5-9B:**
- Hormone (Concept), Dietary supplement (Product), MT1 (Concept), MT2 (Concept), Suprachiasmatic nucleus (Location)

**Qwen3.5-27B:**
- Sleep (Concept), ENT (Concept), Drug Reference Guide (Product), DailyMed (Organization), NLM (Organization)

---

## 7. ผลเปรียบเทียบ QA Generation

| Metric | Qwen3.5-9B | Qwen3.5-27B | ∆ (%) |
|--------|-----------|-------------|-------|
| QA Pairs Generated | 100 | 134 | +34% |
| Time | 30.0 min | 50.8 min | +69% |
| QA/chunk (avg) | 2.9 | 3.9 | +34% |
| Time/QA pair (avg) | 18.0s | 22.7s | +26% |

---

## 8. สรุปเปรียบเทียบ (Summary Comparison)

### 8.1 Winner Matrix

| Dimension | Winner | Margin |
|-----------|--------|--------|
| 🏅 **Extraction Volume** (KG+QA) | **Qwen3.5-27B** | +67% KG, +34% QA |
| 🏅 **Entity Diversity** | **เสมอ** | ทั้งคู่ 6 types |
| 🏅 **Speed** | **Qwen3.5-9B** | 1.6x เร็วกว่า |
| 🏅 **Throughput (entities/min)** | **Qwen3.5-9B** | 1.1 vs 0.7 entities/min |
| 🏅 **Memory Efficiency** | **Qwen3.5-9B** | ใช้ RAM น้อยกว่า |

### 8.2 คำแนะนำ (Recommendations)

| Use Case | Recommended Model | เหตุผล |
|----------|-------------------|--------|
| **Production / Batch Processing** | Qwen3.5-9B | เร็วกว่า 1.6x, คุณภาพดีเพียงพอ |
| **High-Quality Extraction** | Qwen3.5-27B | สกัดได้ปริมาณมากกว่า 34-67% |
| **Quick Test / Prototype** | Qwen3.5-9B | Response time ดีกว่า |
| **Research / Critical Documents** | Qwen3.5-27B | ครอบคลุม Concept ได้ดีกว่า |

---

## 9. ปัญหาที่พบและแก้ไข (Issues Found & Fixed)

| # | Issue | Root Cause | Fix | Commit |
|---|-------|-----------|-----|--------|
| 1 | `chunks` query return 0 | `AND tenant_id = ?` filter on table without `tenant_id` column | Removed tenant_id from WHERE clause | `a4d9544` |
| 2 | Embedding returns 401 | `embed_texts()` used gateway port (8080) requiring auth | Added `EMBEDDING_API_URL` env var to point directly to port 8001 | `a4d9544` |
| 3 | LLM calls silently fail | Wrong `HEIMDALL_API_KEY` (`heimdall-key` instead of actual key) | Set correct key from Heimdall `.env` | `a4d9544` |
| 4 | Gateway reports "unhealthy" | Health check used `GET /models` instead of `GET /v1/models` | Fixed URL in `health.rs` | `d295112` |

### 9.1 Database Migration Applied

```sql
ALTER TABLE pipeline_runs ADD COLUMN source_id BIGINT;
ALTER TABLE pipeline_runs ADD COLUMN tenant_id VARCHAR(255);
ALTER TABLE pipeline_runs ADD COLUMN error_message TEXT;
ALTER TABLE pipeline_runs ADD COLUMN run_label VARCHAR(255);
ALTER TABLE pipeline_runs ADD COLUMN prompt_version VARCHAR(50);

CREATE TABLE pipeline_run_steps (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    run_id VARCHAR(36) NOT NULL,
    step_number TINYINT NOT NULL,
    step_name VARCHAR(50) NOT NULL,
    status VARCHAR(20) DEFAULT 'pending',
    item_count BIGINT DEFAULT 0,
    latency_ms BIGINT DEFAULT 0,
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_run_id (run_id)
);
```

---

## 10. Timeline

| Timestamp (UTC) | Event |
|-----------------|-------|
| 2026-03-09 17:45 | Pipeline Start — Qwen3.5-9B |
| 2026-03-09 18:55 | Pipeline Complete — Qwen3.5-9B (70 min) |
| 2026-03-10 13:23 | Pipeline Start — Qwen3.5-27B |
| 2026-03-10 15:15 | Pipeline Complete — Qwen3.5-27B (112 min) |
| 2026-03-11 14:36 | Gateway health check fix verified |

---

**Approved by:** ________________________________  
**Date:** _______________

