#!/usr/bin/env python3
"""
Sprint 48 C.5 — Full cascade retrieval benchmark (Qdrant-payload backed).

Reimplements the icd10_lookup cascade (exact → naive → semantic) from
ro-ai-bridge/src/routes/icd10.rs, including the medical-acronym expansion
in expand_acronyms.

DATA NOTE: the MariaDB `icd10_codes` table is empty in the current K8s
deployment (table dropped during a recent rotation; Sprint 48 B-48d Phase A
ingested into it, but state was reset). However, the Qdrant `icd10-th`
collection retains all 15,376 rows as payloads — so we use those as the
in-memory source of truth for exact/naive lookups. This also avoids
needing MariaDB credentials inside the benchmark.

Compare against:
  - c5_baseline_retrieval.py = semantic-only path (Hit Rate@3 = 61.1%)

Requires:
  - Qdrant port-forwarded: kubectl port-forward svc/qdrant 6333:6333 -n asgard-infra
  - Heimdall gateway running at http://localhost:8080 (per `asgard_heimdall_deployment`)
    serving BGE-M3 via /v1/embeddings (OpenAI-compatible)
  - HEIMDALL_API_KEY env var with a valid Bearer token from Heimdall's API_KEYS

Refactored 2026-05-18 to remove Ollama dependency per `feedback_no_ollama`.
Asgard production stack uses Heimdall as the LLM/embedding gateway.

Usage:
    HEIMDALL_API_KEY=hml-... /opt/homebrew/bin/python3 scripts/c5_cascade_retrieval.py \\
        --k 3 --verbose \\
        --output docs/04_evaluation_and_testing/results/c5_cascade_k3.json
"""
from __future__ import annotations
import argparse
import json
import os
import sys
import time
from pathlib import Path
from urllib.request import Request, urlopen

HEIMDALL_URL = os.environ.get("HEIMDALL_URL", "http://localhost:8080/v1/embeddings")
HEIMDALL_API_KEY = os.environ.get("HEIMDALL_API_KEY")
EMBED_MODEL = os.environ.get("HEIMDALL_EMBED_MODEL", "BAAI/bge-m3")
QDRANT_BASE = "http://localhost:6333/collections/icd10-th"
QDRANT_SEARCH = f"{QDRANT_BASE}/points/search"
QDRANT_SCROLL = f"{QDRANT_BASE}/points/scroll"
TEST_FILE = Path(__file__).parent.parent / "tests/icd10/sprint48_thai_lookup_v0.jsonl"
DEFAULT_SOURCE_VERSION = "anamai-moph-2010"

# Mirror of expand_acronyms PAIRS in icd10.rs (line 318-384).
# Keep in sync when adding entries.
ACRONYM_PAIRS = [
    ("STEMI", "ST elevation myocardial infarction"),
    ("NSTEMI", "Non ST elevation myocardial infarction"),
    ("MI", "myocardial infarction"),
    ("AMI", "acute myocardial infarction"),
    ("CHF", "congestive heart failure"),
    ("CABG", "coronary artery bypass graft"),
    ("AFIB", "atrial fibrillation"),
    ("AF", "atrial fibrillation"),
    ("DVT", "deep vein thrombosis"),
    ("PE", "pulmonary embolism"),
    ("HTN", "hypertension"),
    ("COPD", "chronic obstructive pulmonary disease"),
    ("URTI", "upper respiratory tract infection"),
    ("ARDS", "acute respiratory distress syndrome"),
    ("PNA", "pneumonia"),
    ("T1DM", "type 1 diabetes mellitus"),
    ("T2DM", "type 2 diabetes mellitus"),
    ("DM", "diabetes mellitus"),
    ("DKA", "diabetic ketoacidosis"),
    ("CVA", "cerebrovascular accident stroke"),
    ("TIA", "transient ischemic attack"),
    ("AKI", "acute kidney injury"),
    ("CKD", "chronic kidney disease"),
    ("ESRD", "end stage renal disease"),
    ("UTI", "urinary tract infection"),
    ("GERD", "gastroesophageal reflux disease"),
    ("IBD", "inflammatory bowel disease"),
    ("GIB", "gastrointestinal bleeding"),
    ("RDS", "respiratory distress syndrome"),
    ("PROM", "premature rupture of membranes"),
    ("MDD", "major depressive disorder"),
    ("GAD", "generalized anxiety disorder"),
    ("PTSD", "post traumatic stress disorder"),
    ("OCD", "obsessive compulsive disorder"),
]


def expand_acronyms(query: str) -> str:
    out_parts = []
    changed = False
    for token in query.split():
        core = "".join(c for c in token if c.isalnum())
        punct = token[len(core):]
        upper = core.upper()
        matched = next((full for k, full in ACRONYM_PAIRS if k == upper), None)
        if matched:
            out_parts.append(matched + punct)
            changed = True
        else:
            out_parts.append(token)
    return " ".join(out_parts) if changed else query


def http_post_json(url: str, body: dict, headers: dict | None = None,
                   timeout: float = 60.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    merged = {"Content-Type": "application/json"}
    if headers:
        merged.update(headers)
    req = Request(url, data=data, headers=merged)
    with urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def load_corpus_from_qdrant(source_version: str) -> list[dict]:
    """Scroll all points from icd10-th and collect their payloads.
    Returns list of {code, en_label, th_label, chapter, source_version} dicts."""
    out = []
    next_page = None
    while True:
        body = {"limit": 500, "with_payload": True, "with_vector": False}
        if next_page is not None:
            body["offset"] = next_page
        resp = http_post_json(QDRANT_SCROLL, body)
        for p in resp["result"]["points"]:
            pl = p["payload"]
            if pl.get("source_version") == source_version:
                out.append(pl)
        next_page = resp["result"].get("next_page_offset")
        if next_page is None:
            break
    return out


def sql_like_exact(corpus: list[dict], q: str, locale: str, k: int) -> list[dict]:
    """Exact match on code or label, with same ORDER BY as Rust impl."""
    q_lower = q.lower()
    matches = []
    for row in corpus:
        code = row.get("code", "")
        en = (row.get("en_label") or "")
        th = (row.get("th_label") or "")
        code_hit = code == q
        en_hit = en.lower() == q_lower if locale in ("en", "both") else False
        th_hit = th == q if locale in ("th", "both") else False
        if code_hit or en_hit or th_hit:
            matches.append(row)
    # ORDER BY: exact code first, then prefix code, then shorter codes
    matches.sort(key=lambda r: (
        0 if r.get("code", "") == q else 1,
        0 if r.get("code", "").startswith(q) else 1,
        len(r.get("code", "")),
        r.get("code", ""),
    ))
    return matches[:k]


def sql_like_naive(corpus: list[dict], q: str, locale: str, k: int) -> list[dict]:
    """%q% LIKE match on code or label."""
    q_lower = q.lower()
    matches = []
    for row in corpus:
        code = row.get("code", "")
        en = (row.get("en_label") or "").lower()
        th = row.get("th_label") or ""
        if q_lower in code.lower():
            matches.append(row); continue
        if locale in ("en", "both") and q_lower in en:
            matches.append(row); continue
        if locale in ("th", "both") and q in th:
            matches.append(row); continue
    matches.sort(key=lambda r: (
        0 if r.get("code", "") == q else 1,
        0 if r.get("code", "").startswith(q) else 1,
        len(r.get("code", "")),
        r.get("code", ""),
    ))
    return matches[:k]


def embed(text: str) -> list[float]:
    """BGE-M3 embedding via Heimdall gateway. Returns 1024-d vector."""
    if not HEIMDALL_API_KEY:
        raise RuntimeError(
            "HEIMDALL_API_KEY env var required. Set to a valid Bearer token from "
            "Heimdall's API_KEYS (see ~/Library/LaunchAgents/com.asgard.heimdall-gateway.plist)."
        )
    out = http_post_json(
        HEIMDALL_URL,
        {"model": EMBED_MODEL, "input": text},
        headers={"Authorization": f"Bearer {HEIMDALL_API_KEY}"},
    )
    return out["data"][0]["embedding"]


def qdrant_search(vector: list[float], k: int, source_version: str) -> list[dict]:
    out = http_post_json(QDRANT_SEARCH, {
        "vector": vector,
        "limit": k,
        "with_payload": True,
        "with_vector": False,
        "filter": {"must": [
            {"key": "source_version", "match": {"value": source_version}},
        ]},
    })
    return out["result"]


def cascade(corpus: list[dict], q: str, locale: str, k: int,
            source_version: str) -> tuple[str, list[dict]]:
    rows = sql_like_exact(corpus, q, locale, k)
    if rows:
        return "exact", rows
    rows = sql_like_naive(corpus, q, locale, k)
    if rows:
        return "naive", rows
    try:
        expanded = expand_acronyms(q)
        vec = embed(expanded)
        hits = qdrant_search(vec, k, source_version)
        return "semantic", [{**h["payload"], "score": h.get("score")} for h in hits]
    except Exception as e:
        print(f"[semantic fail] {e}", file=sys.stderr)
        return "miss", []


def hit_at_k(retrieved: list[str], gold: list[str]) -> bool:
    return any(g in set(retrieved) for g in gold)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--k", type=int, default=3)
    ap.add_argument("--output", type=Path, default=None)
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    queries = []
    with TEST_FILE.open() as f:
        for line in f:
            line = line.strip()
            if line:
                queries.append(json.loads(line))

    print(f"=== Sprint 48 C.5 cascade retrieval ===")
    print(f"Cascade: exact → naive → semantic (matches icd10.rs prod path)")
    print(f"Acronym expansion: pre-embedding, 34 entries")
    print(f"Queries: {len(queries)} (Sprint 48 v0)")
    print(f"K:       {args.k}")
    print()
    print("Loading corpus from Qdrant icd10-th payloads...")
    t0 = time.time()
    corpus = load_corpus_from_qdrant(DEFAULT_SOURCE_VERSION)
    print(f"  Loaded {len(corpus)} rows in {time.time()-t0:.1f}s")
    print()

    results = []
    hits = 0
    mode_counts = {"exact": 0, "naive": 0, "semantic": 0, "miss": 0}
    total_latency_ms = 0.0

    for q in queries:
        qid = q["id"]
        query_text = q["query"]
        gold = q.get("gold_codes") or ([q["top1"]] if "top1" in q else [])
        locale = q.get("locale", "both")

        t0 = time.time()
        mode_used, rows = cascade(corpus, query_text, locale, args.k,
                                  DEFAULT_SOURCE_VERSION)
        latency_ms = (time.time() - t0) * 1000

        retrieved = [r.get("code", "") for r in rows]
        is_hit = hit_at_k(retrieved, gold)
        if is_hit:
            hits += 1
        mode_counts[mode_used] = mode_counts.get(mode_used, 0) + 1
        total_latency_ms += latency_ms

        results.append({
            "id": qid,
            "query": query_text,
            "locale": locale,
            "gold_codes": gold,
            "retrieved_codes": retrieved,
            "mode_used": mode_used,
            "hit_at_k": is_hit,
            "latency_ms": round(latency_ms, 2),
            "notes": q.get("notes"),
        })
        if args.verbose:
            mark = "✓" if is_hit else "✗"
            print(f"[{qid}] {mark} {query_text!r:35s} mode={mode_used:8s} → {retrieved} (gold {gold}) {latency_ms:.0f}ms")

    n = len(results)
    hit_rate = hits / n if n else 0.0
    avg_lat = total_latency_ms / n if n else 0.0
    print()
    print(f"=== Results ===")
    print(f"Hit Rate@{args.k}:           {hit_rate:.1%}  ({hits}/{n})")
    print(f"vs baseline semantic-only:  61.1%  (Δ={hit_rate-0.611:+.1%})")
    print(f"Avg latency:                {avg_lat:.0f} ms/query")
    print(f"Mode usage:                 {mode_counts}")

    by_locale: dict[str, list] = {}
    for r in results:
        by_locale.setdefault(r["locale"], []).append(r)
    print()
    print("By locale:")
    for loc, items in by_locale.items():
        loc_hits = sum(1 for r in items if r["hit_at_k"])
        print(f"  {loc:10s} {loc_hits}/{len(items)}  ({loc_hits/len(items):.0%})")

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({
            "dataset": "sprint48_thai_lookup_v0",
            "approach": "cascade(exact→naive→semantic) + acronym expansion",
            "corpus_source": "Qdrant icd10-th payloads (15376 rows, anamai-moph-2010)",
            "k": args.k,
            "n_queries": n,
            "hit_rate": hit_rate,
            "baseline_semantic_only_hit_rate": 0.611,
            "improvement": hit_rate - 0.611,
            "avg_latency_ms": avg_lat,
            "mode_usage": mode_counts,
            "results": results,
            "timestamp_unix": int(time.time()),
        }, ensure_ascii=False, indent=2))
        print(f"\nSaved → {args.output}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
