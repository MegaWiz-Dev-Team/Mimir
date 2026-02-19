# 🧪 Test Results Design Specification

เป้าหมาย: เพื่อเก็บประวัติการทดสอบ (Benchmark, Latency, Accuracy) แยกตาม Phase และ Sprint อย่างเป็นระบบ เพื่อใช้ในการเปรียบเทียบประสิทธิภาพระหว่าง Hardware (M3 vs M4 Pro) และการตรวจสอบความถูกต้องของระบบ

## 📂 Directory Structure

```text
tests/results/
├── phase_1/
│   ├── sprint_1.1_latency/
│   │   ├── 2026-02-18_1300_m3_gemma2b.json
│   │   └── 2026-02-28_1000_m4pro_gemma2b.json
│   ├── sprint_1.4_scraper/
│   │   └── crawler_wiki_report.json
│   └── summary.md
├── phase_2/
│   ├── sprint_2.1_chat_latency/
│   └── sprint_2.2_oracle_accuracy/
└── reports/ (Generated summaries/graphs)
```

## 📄 JSON Schema (Standard Result)

ทุกผลการทดสอบควรเก็บในรูปแบบ JSON เพื่อให้นำไปประมวลผลต่อได้ง่าย:

```json
{
  "test_id": "latency_test_v1",
  "timestamp": "2026-02-18T13:00:00Z",
  "phase": "phase_1",
  "sprint": "sprint_1.1",
  "environment": {
    "os": "macOS",
    "hardware": "M3",
    "ram": "24GB",
    "llm_provider": "Ollama",
    "model": "gemma:2b"
  },
  "metrics": {
    "duration_ms": 8400,
    "target_ms": 1800,
    "status": "WARN",
    "token_per_sec": null
  },
  "raw_output": "...",
  "tester": "Antigravity-AI"
}
```

## 🛠️ Implementation Strategy

1.  **Phase 1:** สร้างโฟลเดอร์ `tests/results/phase_1/sprint_1.1_latency/`
2.  **Latency Test Update:** ปรับปรุง `src/bin/test_latency.rs` ให้บันทึกผลลัพธ์ลงไฟล์อัตโนมัติเมื่อรันเสร็จ
3.  **Wiki Scraper Report:** สร้างรายงานหลังจากรัน `fetch_wiki` เพื่อเก็บสถิติการดึงข้อมูล
