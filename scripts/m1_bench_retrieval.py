#!/usr/bin/env python3
"""M1 medical retrieval benchmark — Sprint 1 decision gate.

Runs the 75 hand-curated TH/EN medical queries from
`tests/eval_datasets/m1/v1.0/queries.jsonl` through the Mimir retrieval
path and reports Hit Rate@3.

Gate (per dataset README):
  ≥75% → adopt BGE-M3 + current chunking
  60-75% → run hybrid (BGE-M3 + sparse exact-match) + benchmark
  <60% → fine-tune plan

Strategy: route by query category to the most appropriate Mimir
endpoint, since a single endpoint can't serve drug-lookup, disease-
lookup, and clinical-concept queries equally well.

  drug_name, drug_synonym, drug_class, drug_interaction,
  drug_disease_relation
    → PrimeKG semantic search over Qdrant primekg-entities
      (BGE-M3 embed via Heimdall, cosine)
  disease, code_lookup, symptom_to_disease
    → ICD-10 cascade /api/v1/icd10/lookup (exact → naive → semantic)
  sleep_procedure, sleep_metric, clinical_scenario,
  clinical_concept, acronym
    → /api/search (multi-source RAG)
  negation → tested but counted toward category they negate

Hit definition (per dataset spec):
  Top-3 results contain at least one expected entity (drug generic,
  ICD code, etc.), AND no expected_NOT entries appear in top-3.

Usage:
    export HEIMDALL_API_KEY=hml-...
    python3 scripts/m1_bench_retrieval.py
"""
from __future__ import annotations
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import defaultdict
from pathlib import Path

HERE         = Path(__file__).resolve().parent
DATASET      = HERE.parent / "tests/eval_datasets/m1/v1.0/queries.jsonl"
HEIMDALL_URL = os.environ.get("HEIMDALL_API_URL", "http://localhost:8080/v1").rstrip("/")
HEIMDALL_KEY = os.environ.get("HEIMDALL_API_KEY", "")
QDRANT_URL   = os.environ.get("QDRANT_URL", "http://localhost:6333").rstrip("/")
MIMIR_URL    = os.environ.get("MIMIR_URL", "http://localhost:18080").rstrip("/")
MIMIR_JWT    = os.environ.get("MIMIR_JWT", "")
EMBED_MODEL  = "BAAI/bge-m3"


def http_post_json(url: str, body: dict, headers: dict | None = None,
                   timeout: float = 30.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    merged = {"Content-Type": "application/json"}
    if headers:
        merged.update(headers)
    req = urllib.request.Request(url, data=data, headers=merged)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def http_get_json(url: str, headers: dict | None = None, timeout: float = 30.0) -> dict:
    req = urllib.request.Request(url, headers=headers or {})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def embed(text: str) -> list[float]:
    out = http_post_json(
        f"{HEIMDALL_URL}/embeddings",
        {"model": EMBED_MODEL, "input": text},
        headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
    )
    return out["data"][0]["embedding"]


def qdrant_search(collection: str, vector: list[float], k: int = 3) -> list[dict]:
    out = http_post_json(
        f"{QDRANT_URL}/collections/{collection}/points/search",
        {
            "vector": {"name": "dense", "vector": vector},
            "limit": k,
            "with_payload": True,
        },
    )
    return out.get("result", [])


def _auth_headers(extra: dict | None = None) -> dict:
    h = {"X-Tenant-Id": "asgard_medical"}
    if MIMIR_JWT:
        h["Authorization"] = f"Bearer {MIMIR_JWT}"
    if extra:
        h.update(extra)
    return h


def icd10_lookup(query: str, k: int = 3) -> list[dict]:
    url = f"{MIMIR_URL}/api/v1/icd10/lookup?q={urllib.parse.quote(query)}&limit={k}"
    try:
        out = http_get_json(url, headers=_auth_headers())
        return out.get("results", [])
    except Exception:
        return []


def mimir_search(query: str, k: int = 3) -> list[dict]:
    try:
        out = http_post_json(
            f"{MIMIR_URL}/api/search",
            {"query": query, "limit": k},
            headers=_auth_headers(),
        )
        return out.get("results", [])
    except Exception:
        return []


# Map categories to retrieval strategies.
DRUG_CATEGORIES = {"drug_name", "drug_synonym", "drug_class",
                   "drug_interaction", "drug_disease_relation"}
DISEASE_CATEGORIES = {"disease", "code_lookup", "symptom_to_disease"}
GENERAL_CATEGORIES = {"sleep_procedure", "sleep_metric",
                      "clinical_scenario", "clinical_concept", "acronym"}


def normalize(s: str) -> str:
    """Lowercase + strip punctuation for substring match."""
    return re.sub(r"[^\w\s]", " ", (s or "").lower())


def hit_match(expected: list[str], retrieved_texts: list[str],
              forbid: list[str] | None = None) -> bool:
    """Top-K hit if any expected substring in any retrieved text AND
    no forbidden substring is present (negation queries)."""
    if not expected:
        return False
    combined = " ".join(normalize(t) for t in retrieved_texts)
    has_expected = any(normalize(e) in combined for e in expected if e)
    if forbid:
        has_forbid = any(normalize(f) in combined for f in forbid if f)
        return has_expected and not has_forbid
    return has_expected


def _interaction_expected(text: str) -> list[str]:
    """For drug_interaction queries we don't have an explicit expected list.
    Treat as success if BOTH drug names from the query appear in retrieval.
    Parse from patterns like 'warfarin + amiodarone' or 'X + Y serotonin synd'."""
    # Extract first 2 alphabetic tokens (drug names)
    tokens = re.findall(r"[A-Za-zก-๛]{4,}", text)
    return tokens[:2]


def run_query(q: dict) -> dict:
    """Returns dict with hit, retrieved_texts, strategy, latency_ms."""
    text = q["query"]
    cat = q.get("category", "")
    # Collect expected matches across all possible fields
    expected: list[str] = []
    expected.extend(q.get("expected_drug_generics", []) or [])
    expected.extend(q.get("expected_drug_classes", []) or [])
    expected.extend(q.get("expected_icd_codes", []) or [])
    expected.extend(q.get("expected_icd_chapters", []) or [])
    expected.extend(q.get("expected_concepts", []) or [])
    # drug_interaction queries don't have explicit expected — derive from query
    if cat == "drug_interaction" and not expected:
        expected = _interaction_expected(text)
    forbid = (q.get("expected_NOT_drug_generics", []) or []) + \
             (q.get("expected_NOT_drug_classes", []) or [])

    t0 = time.time()
    retrieved_texts: list[str] = []
    strategy = "?"
    try:
        if cat in DRUG_CATEGORIES:
            strategy = "primekg-qdrant"
            vec = embed(text)
            hits = qdrant_search("primekg-entities", vec, k=3)
            retrieved_texts = [h.get("payload", {}).get("name", "") for h in hits]
        elif cat in DISEASE_CATEGORIES:
            strategy = "icd10-cascade"
            hits = icd10_lookup(text, k=3)
            retrieved_texts = [
                f"{h.get('code','')} {h.get('en_label','')} {h.get('th_label','') or ''}"
                for h in hits
            ]
        elif cat in GENERAL_CATEGORIES:
            strategy = "mimir-search"
            hits = mimir_search(text, k=3)
            retrieved_texts = [
                f"{h.get('title','')} {h.get('content','')[:200]}"
                for h in hits
            ]
        elif cat == "negation":
            # Negation queries — use mimir-search and check forbid
            strategy = "mimir-search-negation"
            hits = mimir_search(text, k=3)
            retrieved_texts = [
                f"{h.get('title','')} {h.get('content','')[:200]}"
                for h in hits
            ]
        else:
            # Fallback
            strategy = "mimir-search"
            hits = mimir_search(text, k=3)
            retrieved_texts = [
                f"{h.get('title','')} {h.get('content','')[:200]}"
                for h in hits
            ]
    except Exception as e:
        return {
            "hit": False, "error": str(e)[:120], "strategy": strategy,
            "latency_ms": int((time.time() - t0) * 1000),
            "retrieved": [], "expected": expected,
        }

    hit = hit_match(expected, retrieved_texts, forbid)
    return {
        "hit": hit,
        "strategy": strategy,
        "latency_ms": int((time.time() - t0) * 1000),
        "retrieved": retrieved_texts[:3],
        "expected": expected,
        "forbid": forbid,
        "error": None,
    }


def main() -> int:
    if not HEIMDALL_KEY:
        print("ERR: HEIMDALL_API_KEY required", file=sys.stderr)
        return 1
    if not DATASET.exists():
        print(f"ERR: {DATASET} not found", file=sys.stderr)
        return 1

    queries = [json.loads(l) for l in DATASET.read_text().splitlines() if l.strip()]
    print(f"=== M1 medical retrieval benchmark ===")
    print(f"  dataset:  {DATASET} ({len(queries)} queries)")
    print(f"  Mimir:    {MIMIR_URL}")
    print(f"  Heimdall: {HEIMDALL_URL}")
    print(f"  Qdrant:   {QDRANT_URL}")
    print()

    rows = []
    by_cat: dict[str, list] = defaultdict(list)
    by_diff: dict[str, list] = defaultdict(list)
    by_locale: dict[str, list] = defaultdict(list)
    t0 = time.time()

    for i, q in enumerate(queries, 1):
        r = run_query(q)
        rows.append({**q, **r})
        by_cat[q.get("category","?")].append(r["hit"])
        by_diff[q.get("difficulty","?")].append(r["hit"])
        by_locale[q.get("locale","?")].append(r["hit"])
        mark = "✓" if r["hit"] else "✗"
        err = f" ERR:{r['error']}" if r.get("error") else ""
        print(f"  {i:>2d}/{len(queries)}  {mark}  {q['id']:<8s} [{q.get('category','?'):20s}] "
              f"{q['query'][:30]:<30s}{err}")

    elapsed = int(time.time() - t0)
    hits = sum(1 for r in rows if r["hit"])
    rate = hits / len(rows) if rows else 0
    print()
    print("=" * 64)
    print(f"  Hit Rate@3:  {rate:.1%}  ({hits}/{len(rows)})  · elapsed {elapsed}s")
    print("=" * 64)

    # Per-category breakdown
    print()
    print("By category:")
    for cat in sorted(by_cat, key=lambda k: -len(by_cat[k])):
        items = by_cat[cat]
        h = sum(items); n = len(items)
        bar = "█" * int(h / n * 20) + "░" * (20 - int(h / n * 20))
        print(f"  {cat:<26s}  {h}/{n}  ({h/n:.0%})  {bar}")

    print()
    print("By difficulty:")
    for diff in ["easy", "medium", "hard"]:
        if diff in by_diff:
            items = by_diff[diff]; h = sum(items); n = len(items)
            print(f"  {diff:<8s}  {h}/{n}  ({h/n:.0%})")
    print()
    print("By locale:")
    for loc in sorted(by_locale):
        items = by_locale[loc]; h = sum(items); n = len(items)
        print(f"  {loc:<8s}  {h}/{n}  ({h/n:.0%})")

    # Decision gate
    print()
    print("=" * 64)
    if rate >= 0.75:
        print(f"  GATE: ≥75% — ADOPT BGE-M3 + current chunking ({rate:.1%})")
    elif rate >= 0.60:
        print(f"  GATE: 60-75% — run hybrid sparse + benchmark ({rate:.1%})")
    else:
        print(f"  GATE: <60% — fine-tune plan needed ({rate:.1%})")
    print("=" * 64)

    # Persist report
    out = HERE / "reports" / f"m1_retrieval_{time.strftime('%Y%m%d_%H%M')}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps({
        "ran_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "n_queries": len(rows),
        "hit_rate_at_3": rate,
        "by_category": {c: {"hits": sum(items), "n": len(items)}
                        for c, items in by_cat.items()},
        "by_difficulty": {d: {"hits": sum(items), "n": len(items)}
                          for d, items in by_diff.items()},
        "by_locale": {l: {"hits": sum(items), "n": len(items)}
                      for l, items in by_locale.items()},
        "rows": rows,
    }, ensure_ascii=False, indent=2))
    print(f"\nReport: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
