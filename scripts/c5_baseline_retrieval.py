#!/usr/bin/env python3
"""
Sprint 48 C.5 — Baseline retrieval benchmark on icd10-th collection.

Runs the Sprint 48 v0 ICD-10 test queries (tests/icd10/sprint48_thai_lookup_v0.jsonl)
through the existing icd10-th Qdrant collection using Ollama BGE-M3 query embedding.
Computes Hit Rate@K and per-query result detail.

This is the **baseline** — retrieval quality of the current chunking + embedding setup
*before* any chunk-size remediation. Subsequent runs (C.5.x) re-embed at different
chunk sizes and compare against this number.

Requires:
- Ollama running with bge-m3 model:  http://localhost:11434
- Qdrant port-forwarded:              http://localhost:6333
- icd10-th collection populated (15,376 points, 1024-dim BGE-M3)

Usage:
    python scripts/c5_baseline_retrieval.py [--k 3] [--output FILE.json]
"""
from __future__ import annotations
import argparse
import json
import sys
import time
from pathlib import Path
from urllib.request import Request, urlopen
from urllib.error import URLError

OLLAMA_URL = "http://localhost:11434/api/embed"
QDRANT_URL = "http://localhost:6333/collections/icd10-th/points/search"
TEST_FILE = Path(__file__).parent.parent / "tests/icd10/sprint48_thai_lookup_v0.jsonl"


def http_post_json(url: str, body: dict, timeout: float = 30.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    req = Request(url, data=data, headers={"Content-Type": "application/json"})
    with urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def embed(text: str) -> list[float]:
    """Ollama BGE-M3 embedding. Returns 1024-d vector."""
    out = http_post_json(OLLAMA_URL, {"model": "bge-m3", "input": text})
    return out["embeddings"][0]


def qdrant_search(vector: list[float], k: int) -> list[dict]:
    """Search icd10-th collection, return top-k payloads with score."""
    out = http_post_json(
        QDRANT_URL,
        {"vector": vector, "limit": k, "with_payload": True, "with_vector": False},
    )
    return out["result"]


def hit_at_k(retrieved_codes: list[str], gold_codes: list[str]) -> bool:
    """A query is a hit if ANY gold code appears in the retrieved code list.
    Matches both exact-code and prefix-code conventions used in Sprint 48 v0."""
    retrieved_set = set(retrieved_codes)
    return any(g in retrieved_set for g in gold_codes)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--k", type=int, default=3, help="Top-K to retrieve (default: 3)")
    ap.add_argument("--output", type=Path, default=None, help="Save JSON results to FILE")
    ap.add_argument("--verbose", action="store_true", help="Per-query detail")
    args = ap.parse_args()

    if not TEST_FILE.exists():
        print(f"ERROR: test file not found at {TEST_FILE}", file=sys.stderr)
        return 1

    queries = []
    with TEST_FILE.open() as f:
        for line in f:
            line = line.strip()
            if line:
                queries.append(json.loads(line))

    print(f"=== Sprint 48 C.5 baseline retrieval ===")
    print(f"Collection: icd10-th (15,376 points, BGE-M3 1024-d)")
    print(f"Queries:    {len(queries)} (Sprint 48 v0)")
    print(f"K:          {args.k}")
    print()

    results = []
    hits = 0
    total_latency_ms = 0.0

    for q in queries:
        qid = q["id"]
        query_text = q["query"]
        gold = q.get("gold_codes", [])
        if not gold and "top1" in q:
            gold = [q["top1"]]

        t0 = time.time()
        try:
            vec = embed(query_text)
            hits_qdrant = qdrant_search(vec, args.k)
            latency_ms = (time.time() - t0) * 1000
        except URLError as e:
            print(f"[{qid}] FAIL: {e}", file=sys.stderr)
            continue

        retrieved = [h["payload"]["code"] for h in hits_qdrant]
        is_hit = hit_at_k(retrieved, gold)
        if is_hit:
            hits += 1
        total_latency_ms += latency_ms

        results.append({
            "id": qid,
            "query": query_text,
            "locale": q.get("locale"),
            "gold_codes": gold,
            "retrieved_codes": retrieved,
            "retrieved_payloads": [h["payload"] for h in hits_qdrant],
            "scores": [h["score"] for h in hits_qdrant],
            "hit_at_k": is_hit,
            "latency_ms": round(latency_ms, 2),
            "notes": q.get("notes"),
        })

        if args.verbose:
            mark = "✓" if is_hit else "✗"
            print(f"[{qid}] {mark} {query_text!r}  → top-{args.k}: {retrieved}  (gold: {gold})  {latency_ms:.0f}ms")

    n = len(results)
    hit_rate = hits / n if n else 0.0
    avg_lat = total_latency_ms / n if n else 0.0

    print()
    print(f"=== Results ===")
    print(f"Hit Rate@{args.k}:   {hit_rate:.1%}  ({hits}/{n})")
    print(f"Avg latency:    {avg_lat:.0f} ms/query")

    # Breakdown by locale
    by_locale: dict[str, list] = {}
    for r in results:
        by_locale.setdefault(r["locale"] or "unknown", []).append(r)
    print()
    print("By locale:")
    for loc, items in by_locale.items():
        loc_hits = sum(1 for r in items if r["hit_at_k"])
        print(f"  {loc:10s}  {loc_hits}/{len(items)}  ({loc_hits/len(items):.0%})")

    # Per-difficulty (if 'mode' tag carries difficulty info)
    by_mode: dict[str, list] = {}
    for r in results:
        m = next((q["mode"] for q in queries if q["id"] == r["id"]), "unknown")
        by_mode.setdefault(m, []).append(r)
    print()
    print("By mode:")
    for m, items in sorted(by_mode.items()):
        m_hits = sum(1 for r in items if r["hit_at_k"])
        print(f"  {m:10s}  {m_hits}/{len(items)}  ({m_hits/len(items):.0%})")

    if args.output:
        report = {
            "dataset": "sprint48_thai_lookup_v0",
            "collection": "icd10-th",
            "embedding_model": "bge-m3 (Ollama)",
            "k": args.k,
            "n_queries": n,
            "hit_rate": hit_rate,
            "avg_latency_ms": avg_lat,
            "results": results,
            "timestamp_unix": int(time.time()),
        }
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(f"\nSaved → {args.output}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
