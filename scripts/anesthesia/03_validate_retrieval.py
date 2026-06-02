#!/usr/bin/env python3
"""Validate Qdrant retrieval for anesthesia_kb_001 with 10 test queries.

Pass criteria:
  - Hit Rate @ 3 ≥ 80% (top-3 contains expected source PDF)
  - Hit Rate @ 5 ≥ 95%
  - p95 latency < 500ms

Usage:
    /tmp/docx-venv/bin/python3 03_validate_retrieval.py
"""
from __future__ import annotations

import json
import os
import time
from pathlib import Path

import requests

HEIMDALL   = os.environ.get("HEIMDALL_URL", "http://localhost:8080/v1").rstrip("/")
QDRANT     = os.environ.get("QDRANT_URL",   "http://localhost:16333").rstrip("/")
COLLECTION = "anesthesia_kb_001"

# Load Heimdall key
env_file = Path("/Users/mimir/Developer/Heimdall/.env")
HEIMDALL_KEY = ""
for line in env_file.read_text().splitlines():
    if line.startswith("API_KEYS="):
        HEIMDALL_KEY = line.split("=", 1)[1].strip().strip("'\"").split(",")[0]
        break

# 10 test queries — each with expected source PDF substring (Thai or English)
TEST_QUERIES = [
    {"q": "ผู้ป่วยควรอดน้ำกี่ชั่วโมงก่อนผ่าตัด",
     "expect": "งดน้ำและอาหาร"},
    {"q": "ASA classification คือการประเมินอะไร",
     "expect": "ประเมินผู้ป่วยก่อนการระงับความรู้สึก"},
    {"q": "ภาวะแทรกซ้อนจาก spinal anesthesia",
     "expect": "Spinal-Anesthesia"},
    {"q": "การรักษา Malignant Hyperthermia",
     "expect": "Malignant-Hyperthermia"},
    {"q": "ป้องกันคลื่นไส้อาเจียนหลังผ่าตัด PONV",
     "expect": "คลื่นไส้อาเจียนหลังการผ่าตัด"},
    {"q": "ใส่ท่อช่วยหายใจในกรณีฉุกเฉิน difficult airway",
     "expect": "ใส่ท่อช่วยหายใจในกรณีฉุกเฉิน"},
    {"q": "การฉีดยาชาเฉพาะส่วนในผู้ป่วยที่กิน warfarin",
     "expect": "ผู้ป่วยที่ได้รับยาต้าน"},
    {"q": "การจัดการความปวดเฉียบพลันหลังผ่าตัด",
     "expect": "ระงับปวดเฉียบพลันหลังผ่าตัด"},
    {"q": "ผู้ป่วยใช้กัญชาก่อนผ่าตัดต้องดูแลอย่างไร",
     "expect": "ผู้ใช้กัญชา"},
    {"q": "การให้ propofol สำหรับ moderate sedation",
     "expect": "โปรโปฟอล"},  # may not be found - that PDF was skipped (no_text)
]


def embed(text: str) -> list[float]:
    r = requests.post(f"{HEIMDALL}/embeddings",
                      headers={"Authorization": f"Bearer {HEIMDALL_KEY}",
                               "Content-Type": "application/json"},
                      json={"model": "bge-m3", "input": text},
                      timeout=30)
    r.raise_for_status()
    return r.json()["data"][0]["embedding"]


def search(vector: list[float], top: int = 10) -> list[dict]:
    r = requests.post(f"{QDRANT}/collections/{COLLECTION}/points/search",
                      json={"vector": vector, "limit": top, "with_payload": True},
                      timeout=10)
    r.raise_for_status()
    return r.json()["result"]


def run_query(q: dict) -> dict:
    t0 = time.time()
    vec = embed(q["q"])
    embed_ms = (time.time() - t0) * 1000

    t1 = time.time()
    results = search(vec, top=10)
    search_ms = (time.time() - t1) * 1000

    # Check if expected source appears in top results
    expect = q["expect"]
    hit_at = None
    for i, r in enumerate(results, start=1):
        src = r["payload"].get("source_pdf", "")
        if expect in src:
            hit_at = i
            break

    return {
        "query": q["q"],
        "expect": expect,
        "hit_at": hit_at,
        "top1_src": results[0]["payload"].get("source_pdf", "") if results else "",
        "top1_score": results[0]["score"] if results else 0,
        "embed_ms": round(embed_ms, 1),
        "search_ms": round(search_ms, 1),
        "total_ms": round(embed_ms + search_ms, 1),
    }


def main():
    print("=" * 80)
    print(f"Anesthesia KB Retrieval Validation — 10 queries")
    print(f"Collection: {COLLECTION}")
    print("=" * 80)

    results = []
    for i, q in enumerate(TEST_QUERIES, start=1):
        r = run_query(q)
        results.append(r)
        hit_str = (f"hit@{r['hit_at']}" if r["hit_at"] else "MISS")
        print(f"\n[{i:2}/{len(TEST_QUERIES)}] {hit_str} ({r['total_ms']}ms)")
        print(f"   Q: {q['q']}")
        print(f"   Expect:  *{q['expect']}*")
        print(f"   Top-1:   {r['top1_src'][:60]} (score {r['top1_score']:.3f})")

    print("\n" + "=" * 80)
    print("METRICS")
    print("=" * 80)

    hits_3  = sum(1 for r in results if r["hit_at"] and r["hit_at"] <= 3)
    hits_5  = sum(1 for r in results if r["hit_at"] and r["hit_at"] <= 5)
    hits_10 = sum(1 for r in results if r["hit_at"])
    n = len(results)

    latencies = sorted(r["total_ms"] for r in results)
    p50 = latencies[n // 2]
    p95 = latencies[int(n * 0.95)] if n >= 20 else latencies[-1]

    print(f"  Hit Rate @ 3:  {hits_3}/{n} = {hits_3/n:.0%}  (target ≥80%)")
    print(f"  Hit Rate @ 5:  {hits_5}/{n} = {hits_5/n:.0%}  (target ≥95%)")
    print(f"  Hit Rate @ 10: {hits_10}/{n} = {hits_10/n:.0%}")
    print(f"  Latency p50:   {p50}ms")
    print(f"  Latency p95:   {p95}ms (target <500ms)")

    # Pass/fail
    print()
    pass_3 = hits_3 / n >= 0.8
    pass_5 = hits_5 / n >= 0.95
    pass_lat = p95 < 500
    print(f"  HR@3 ≥80%:  {'✓ PASS' if pass_3 else '✗ FAIL'}")
    print(f"  HR@5 ≥95%:  {'✓ PASS' if pass_5 else '✗ FAIL'}")
    print(f"  Latency:    {'✓ PASS' if pass_lat else '✗ FAIL'}")

    # Misses detail
    misses = [r for r in results if not r["hit_at"]]
    if misses:
        print("\n--- MISSES (review these) ---")
        for r in misses:
            print(f"  Q: {r['query']}")
            print(f"    Expected: {r['expect']}")
            print(f"    Got top-1: {r['top1_src']}")

    # Save
    from datetime import datetime
    out = Path("/Users/mimir/Developer/Mimir/data/eir-anesthesia")
    out.mkdir(parents=True, exist_ok=True)
    fn = out / f"validation_{datetime.now():%Y%m%d_%H%M%S}.json"
    fn.write_text(json.dumps({
        "collection": COLLECTION,
        "n_queries": n,
        "hit_rate_3": hits_3 / n,
        "hit_rate_5": hits_5 / n,
        "hit_rate_10": hits_10 / n,
        "latency_p50_ms": p50,
        "latency_p95_ms": p95,
        "all_pass": pass_3 and pass_5 and pass_lat,
        "per_query": results,
    }, ensure_ascii=False, indent=2))
    print(f"\n📊 Saved: {fn}")


if __name__ == "__main__":
    main()
