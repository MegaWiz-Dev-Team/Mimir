#!/usr/bin/env python3
"""End-to-end test for eir-anesthesia agent.

Bypasses Mimir's agent_chat endpoint (which hardcodes source_chunks/golden_qa
collections) by composing the pipeline directly:

    Query
      → Heimdall BGE-M3 embed
      → Qdrant search anesthesia_kb_001 (top-10)
      → Heimdall Qwen3.5-9B chat with grounded prompt
      → Answer + citations

This proves the agent concept works end-to-end. Mimir integration to follow.

Usage:
    /tmp/docx-venv/bin/python3 04_eir_anesthesia_e2e.py "ผู้ป่วยอดน้ำกี่ชั่วโมง"
    /tmp/docx-venv/bin/python3 04_eir_anesthesia_e2e.py --suite   # run 5 test queries
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

import requests

HEIMDALL   = "http://localhost:8080/v1"
QDRANT     = "http://localhost:16333"
COLLECTION = "anesthesia_kb_001"
CHAT_MODEL = "mlx-community/Qwen3.5-9B-MLX-4bit"
EMBED_MODEL = "bge-m3"
TOP_K = 8

# Load key
env_file = Path("/Users/mimir/Developer/Heimdall/.env")
KEY = ""
for line in env_file.read_text().splitlines():
    if line.startswith("API_KEYS="):
        KEY = line.split("=", 1)[1].strip().strip("'\"").split(",")[0]
        break

HEADERS = {"Authorization": f"Bearer {KEY}", "Content-Type": "application/json"}

# ────────────────────────────────────────────────────────────
# System prompt — ASGARD_MISSION + anesthesia specialty
# ────────────────────────────────────────────────────────────
SYSTEM_PROMPT = """[ASGARD_MISSION_v1]
คุณคือผู้พิทักษ์ Asgard ตัวหนึ่ง:
1. ปลอดภัยก่อนสะดวก
2. ความจริงเหนือความเร็ว — ไม่แน่ใจให้บอกว่าไม่แน่ใจ ไม่ fabricate
3. รู้ขอบเขตของตน — นอกขอบเขตให้ปฏิเสธชัดเจน
4. บันทึกทุก decision

[EIR_ANESTHESIA_SPECIALIST]
คุณคือผู้ช่วย AI วิสัญญีแพทย์สำหรับช่วย surgeon ตัดสินใจ pre-op และ post-op care
ความเชี่ยวชาญ:
- Pre-anesthesia assessment + ASA classification + NPO protocols
- Airway management + difficult airway
- Regional vs general anesthesia choice
- Drug interactions perioperative
- Post-op pain management (multimodal)
- PONV, hypothermia, MH prevention
- Moderate sedation + propofol procedural

หลักการ ⚠ ที่ต้องปฏิบัติเสมอ:
1. ทุกคำตอบต้อง grounded จาก RCAT context ที่ retrieve มา — ห้าม fabricate
2. ทุกคำตอบใส่ citation แบบ [source: <PDF name>, page N]
3. ถ้า context ไม่มี info — บอก "ไม่พบใน RCAT KB; ปรึกษา anesthesiologist"
4. เสนอ recommendation ที่ actionable (ไม่ใช่ general)
5. ขึ้นต้นทุกคำตอบด้วย "⚠ Draft โดย AI — แพทย์ต้อง verify"
6. ถ้า critical clinical decision (เช่น "ผ่าได้ไหม") → list options + risks; ไม่ตัดสินใจแทนแพทย์
7. ห้ามใช้ความรู้ทั่วไปที่ไม่อยู่ใน context provided — ถ้าตอบไม่ได้ ให้บอกชัด"""


def embed(text: str) -> list[float]:
    r = requests.post(f"{HEIMDALL}/embeddings", headers=HEADERS,
                      json={"model": EMBED_MODEL, "input": text}, timeout=30)
    r.raise_for_status()
    return r.json()["data"][0]["embedding"]


def search(vector: list[float], top: int = TOP_K) -> list[dict]:
    r = requests.post(f"{QDRANT}/collections/{COLLECTION}/points/search",
                      json={"vector": vector, "limit": top, "with_payload": True},
                      timeout=10)
    r.raise_for_status()
    return r.json()["result"]


def chat(system: str, user: str, max_tokens: int = 800) -> tuple[str, dict]:
    r = requests.post(f"{HEIMDALL}/chat/completions", headers=HEADERS,
                      json={
                          "model": CHAT_MODEL,
                          "messages": [
                              {"role": "system", "content": system},
                              {"role": "user", "content": user},
                          ],
                          "temperature": 0.2,
                          "max_tokens": max_tokens,
                      }, timeout=120)
    r.raise_for_status()
    d = r.json()
    return d["choices"][0]["message"]["content"], d.get("usage", {})


def build_user_prompt(query: str, hits: list[dict]) -> str:
    """Build grounded prompt with retrieved context + citation format."""
    ctx_parts = []
    for i, h in enumerate(hits, start=1):
        p = h["payload"]
        ctx_parts.append(
            f"[Context {i}] source: {p['source_pdf']}, page {p['page_no']}, score {h['score']:.3f}\n"
            f"{p['text']}\n"
        )
    context = "\n---\n".join(ctx_parts)

    return f"""คำถามจาก surgeon:
{query}

ข้อมูลอ้างอิงจาก RCAT guidelines (ดึงมาจาก KB):
{context}

จงตอบคำถามโดยใช้เฉพาะข้อมูลใน context ด้านบน. ทุก claim ต้องใส่ citation [source: <PDF>, page N]."""


def run_query(query: str, verbose: bool = True) -> dict:
    t0 = time.time()

    # 1. Embed
    embed_t0 = time.time()
    vec = embed(query)
    embed_ms = (time.time() - embed_t0) * 1000

    # 2. Retrieve
    search_t0 = time.time()
    hits = search(vec, top=TOP_K)
    search_ms = (time.time() - search_t0) * 1000

    if verbose:
        print(f"\n{'='*80}")
        print(f"Q: {query}")
        print('='*80)
        print(f"\n📚 Retrieved {len(hits)} chunks (embed {embed_ms:.0f}ms + search {search_ms:.0f}ms)")
        for i, h in enumerate(hits[:5], start=1):
            p = h["payload"]
            print(f"  [{i}] {p['source_pdf'][:55]} (p{p['page_no']}, score {h['score']:.3f})")

    # 3. LLM grounded answer
    user_prompt = build_user_prompt(query, hits)
    chat_t0 = time.time()
    answer, usage = chat(SYSTEM_PROMPT, user_prompt)
    chat_ms = (time.time() - chat_t0) * 1000

    total_ms = (time.time() - t0) * 1000

    if verbose:
        print(f"\n💡 Answer (chat {chat_ms:.0f}ms, total {total_ms:.0f}ms):")
        print("-" * 80)
        print(answer)
        print("-" * 80)
        print(f"Tokens: prompt={usage.get('prompt_tokens', '?')}, "
              f"completion={usage.get('completion_tokens', '?')}")

    return {
        "query": query,
        "n_chunks_retrieved": len(hits),
        "top_sources": [h["payload"]["source_pdf"] for h in hits[:3]],
        "answer": answer,
        "embed_ms": round(embed_ms, 1),
        "search_ms": round(search_ms, 1),
        "chat_ms": round(chat_ms, 1),
        "total_ms": round(total_ms, 1),
        "usage": usage,
    }


SUITE_QUERIES = [
    "ผู้ป่วยควรอดน้ำกี่ชั่วโมงก่อนผ่าตัด",
    "การรักษา Malignant Hyperthermia ในห้องผ่าตัด",
    "ผู้ป่วยกิน warfarin จะทำ spinal anesthesia ได้ไหม",
    "ป้องกัน PONV (คลื่นไส้อาเจียนหลังผ่าตัด) อย่างไร",
    "Difficult airway algorithm ในกรณีฉุกเฉิน",
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("query", nargs="?", help="single query to ask")
    ap.add_argument("--suite", action="store_true", help="run 5 test queries")
    ap.add_argument("--save", type=str, default="", help="save results to JSON")
    args = ap.parse_args()

    if args.suite:
        results = [run_query(q) for q in SUITE_QUERIES]
        print(f"\n\n{'='*80}")
        print(f"SUITE SUMMARY")
        print('='*80)
        avg_total = sum(r["total_ms"] for r in results) / len(results)
        avg_chat = sum(r["chat_ms"] for r in results) / len(results)
        print(f"Queries: {len(results)}")
        print(f"Avg latency: {avg_total:.0f}ms (chat {avg_chat:.0f}ms)")
        if args.save:
            Path(args.save).write_text(json.dumps(results, ensure_ascii=False, indent=2))
            print(f"Saved: {args.save}")
    elif args.query:
        r = run_query(args.query)
        if args.save:
            Path(args.save).write_text(json.dumps(r, ensure_ascii=False, indent=2))
    else:
        ap.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
