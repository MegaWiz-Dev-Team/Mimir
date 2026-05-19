#!/usr/bin/env python3
"""PrimeKG isolation benchmark — Sprint 1 W2.1 quick MVP.

Re-runs M1's PrimeKG-targeting queries (34 of 75) through a PrimeKG-only
retrieval path (Qdrant primekg-entities via Heimdall BGE-M3) — no
fallback to TMT, ICD-10, or clinical-wisdom. This isolates pure PrimeKG
semantic signal so we can answer:

  "When the only KB available is PrimeKG, what Hit Rate@3 do drug
  queries get?"

Compare to the M1 multi-store bench (PR #320) where drug_synonym
routed through TMT FULLTEXT hit 100%. Here we deliberately disable
that routing so we see what PrimeKG alone delivers.

Categories included (34 queries from M1):
  drug_name (11), drug_synonym (9), drug_class (1),
  drug_interaction (6), drug_disease_relation (7)

Usage:
    export HEIMDALL_API_KEY=hml-...
    python3 scripts/primekg_isolation_bench.py
    M1_INGEST_DB=0 python3 scripts/primekg_isolation_bench.py   # skip DB write
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
import uuid
from collections import defaultdict
from pathlib import Path

HERE         = Path(__file__).resolve().parent
DATASET      = HERE.parent / "tests/eval_datasets/m1/v1.0/queries.jsonl"
HEIMDALL_URL = os.environ.get("HEIMDALL_API_URL", "http://localhost:8080/v1").rstrip("/")
HEIMDALL_KEY = os.environ.get("HEIMDALL_API_KEY", "")
QDRANT_URL   = os.environ.get("QDRANT_URL", "http://localhost:6333").rstrip("/")
EMBED_MODEL  = "BAAI/bge-m3"

PRIMEKG_CATEGORIES = {
    "drug_name", "drug_synonym", "drug_class",
    "drug_interaction", "drug_disease_relation",
}

M1_DATASET_ID   = "10659f35-fbde-5961-9b8f-75e9b5f93648"
M1_DATASET_NAME = "Medical Retrieval Benchmark — M1 v1.0 (TH+EN) [PrimeKG isolation]"


def http_post_json(url: str, body: dict, headers: dict | None = None,
                   timeout: float = 30.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    merged = {"Content-Type": "application/json"}
    if headers:
        merged.update(headers)
    req = urllib.request.Request(url, data=data, headers=merged)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def embed(text: str) -> list[float]:
    out = http_post_json(
        f"{HEIMDALL_URL}/embeddings",
        {"model": EMBED_MODEL, "input": text},
        headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
    )
    return out["data"][0]["embedding"]


def primekg_search(text: str, k: int = 3) -> list[dict]:
    vec = embed(text)
    out = http_post_json(
        f"{QDRANT_URL}/collections/primekg-entities/points/search",
        {"vector": {"name": "dense", "vector": vec},
         "limit": k, "with_payload": True},
    )
    return out.get("result", [])


def normalize(s: str) -> str:
    return re.sub(r"[^\w\s]", " ", (s or "").lower())


def hit_match(expected: list[str], retrieved_texts: list[str]) -> bool:
    if not expected:
        return False
    combined = " ".join(normalize(t) for t in retrieved_texts)
    return any(normalize(e) in combined for e in expected if e)


def _interaction_expected(text: str) -> list[str]:
    tokens = re.findall(r"[A-Za-zก-๛]{4,}", text)
    return tokens[:2]


def collect_expected(q: dict) -> list[str]:
    out: list[str] = []
    out.extend(q.get("expected_drug_generics", []) or [])
    out.extend(q.get("expected_drug_classes", []) or [])
    out.extend(q.get("expected_concepts", []) or [])
    if q.get("category") == "drug_interaction" and not out:
        out = _interaction_expected(q["query"])
    return out


def main() -> int:
    if not HEIMDALL_KEY:
        print("ERR: HEIMDALL_API_KEY required", file=sys.stderr); return 1
    if not DATASET.exists():
        print(f"ERR: {DATASET} not found", file=sys.stderr); return 1

    all_queries = [json.loads(l) for l in DATASET.read_text().splitlines() if l.strip()]
    queries = [q for q in all_queries if q.get("category") in PRIMEKG_CATEGORIES]

    print(f"=== PrimeKG isolation benchmark ===")
    print(f"  M1 dataset total: {len(all_queries)}")
    print(f"  PrimeKG-targeting subset: {len(queries)}")
    print(f"  collection: primekg-entities (Qdrant) via {EMBED_MODEL}")
    print()

    rows = []
    by_cat: dict[str, list] = defaultdict(list)
    t0 = time.time()

    for i, q in enumerate(queries, 1):
        cat = q.get("category", "?")
        expected = collect_expected(q)
        q_t0 = time.time()
        try:
            hits = primekg_search(q["query"], k=3)
            retrieved = [h.get("payload", {}).get("name", "") for h in hits]
            hit = hit_match(expected, retrieved)
            err = None
        except Exception as e:
            retrieved, hit, err = [], False, str(e)[:80]
        latency = int((time.time() - q_t0) * 1000)
        rows.append({
            "id": q["id"], "query": q["query"], "category": cat,
            "expected": expected, "retrieved": retrieved,
            "hit": hit, "latency_ms": latency, "error": err,
        })
        by_cat[cat].append(hit)
        mark = "✓" if hit else "✗"
        err_t = f"  ERR:{err}" if err else ""
        print(f"  {i:>2}/{len(queries)}  {mark}  {q['id']:<8s} [{cat:24s}] {q['query'][:30]!r:<32s}{err_t}")

    elapsed = int(time.time() - t0)
    hits = sum(1 for r in rows if r["hit"])
    rate = hits / len(rows) if rows else 0
    print()
    print("=" * 62)
    print(f"  Hit Rate@3 (PrimeKG-only):  {rate:.1%}  ({hits}/{len(rows)})  · {elapsed}s")
    print("=" * 62)

    print("\nBy category:")
    for cat in sorted(by_cat, key=lambda k: -len(by_cat[k])):
        items = by_cat[cat]; h = sum(items); n = len(items)
        bar = "█" * int(h/n*20) + "░" * (20-int(h/n*20))
        print(f"  {cat:<24s}  {h}/{n}  ({h/n:.0%})  {bar}")

    # Persist report
    out_path = HERE / "reports" / f"primekg_isolation_{time.strftime('%Y%m%d_%H%M')}.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps({
        "ran_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "scope": "PrimeKG-only (no TMT/ICD-10 fallback)",
        "n_queries": len(rows),
        "hit_rate_at_3": rate,
        "by_category": {c: {"hits": sum(items), "n": len(items)} for c, items in by_cat.items()},
        "rows": rows,
    }, ensure_ascii=False, indent=2))
    print(f"\nReport: {out_path}")

    if os.environ.get("M1_INGEST_DB", "1") == "1":
        ingest_to_mimir_eval(rows, rate, elapsed)

    return 0


# ── Mimir DB ingest (mirrors m1_bench_retrieval.py pattern) ───────────────


def _sql_quote(s) -> str:
    if s is None:
        return "NULL"
    if isinstance(s, bool):
        return "1" if s else "0"
    if isinstance(s, (int, float)):
        return str(s)
    return "'" + str(s).replace("\\", "\\\\").replace("'", "\\'") + "'"


def _mariadb_exec(sql: str) -> None:
    import subprocess
    ns = os.environ.get("MARIADB_NAMESPACE", "asgard-infra")
    r = subprocess.run(
        ["kubectl", "-n", ns, "exec", "-i", "deploy/mariadb", "--",
         "mariadb", "-uroot", "-proot", "mimir", "-B", "-N"],
        input=sql.encode("utf-8"), capture_output=True, timeout=30,
    )
    if r.returncode != 0:
        raise RuntimeError(f"mariadb err: {r.stderr.decode()[:300]}")


def ingest_to_mimir_eval(rows: list[dict], hit_rate: float, elapsed: int) -> None:
    run_id = str(uuid.uuid4())
    tenant = "asgard_medical"
    name = f"PrimeKG isolation bench {time.strftime('%Y-%m-%d %H:%M')}"
    started = time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime(time.time() - elapsed))
    finished = time.strftime('%Y-%m-%d %H:%M:%S')
    n = len(rows)
    avg_latency = sum(r.get("latency_ms", 0) for r in rows) / n if n else 0
    mrr = hit_rate  # crude proxy

    print(f"\n=== Ingest run_id={run_id[:8]}… ===")
    _mariadb_exec(f"""
INSERT INTO rag_eval_runs
  (id, tenant_id, name, status, hit_rate, mrr, top_k, avg_latency_ms,
   collections, embed_model, search_provider, search_model,
   dataset_id, dataset_name, started_at, finished_at, is_baseline)
VALUES
  ({_sql_quote(run_id)}, {_sql_quote(tenant)}, {_sql_quote(name)}, 'completed',
   {hit_rate}, {mrr}, 3, {avg_latency},
   {_sql_quote('primekg-entities')},
   {_sql_quote(EMBED_MODEL)}, 'heimdall', {_sql_quote(EMBED_MODEL)},
   {_sql_quote(M1_DATASET_ID)}, {_sql_quote(M1_DATASET_NAME)},
   {_sql_quote(started)}, {_sql_quote(finished)}, 0);
""")

    batch = 50
    for i in range(0, n, batch):
        chunk = rows[i:i + batch]
        values = []
        for r in chunk:
            expected_titles = json.dumps(r.get("expected", []), ensure_ascii=False)
            retrieved_snippet = " | ".join(t[:80] for t in r.get("retrieved", [])[:3])
            values.append(
                f"({_sql_quote(run_id)}, {_sql_quote(tenant)}, "
                f"{_sql_quote(r['query'])}, "
                f"{_sql_quote(expected_titles)}, {_sql_quote(retrieved_snippet)}, "
                f"{1 if r['hit'] else 0}, "
                f"{1.0 if r['hit'] else 0.0}, "
                f"0, 0, 0, NULL, 0, 0)"
            )
        _mariadb_exec(
            "INSERT INTO rag_eval_queries (run_id, tenant_id, query, "
            "expected_titles, expected_content, hit, reciprocal_rank, "
            "ndcg_score, precision_score, recall_score, matched_at_rank, "
            "vector_contributed, tree_contributed) VALUES "
            + ",\n".join(values) + ";"
        )
    print(f"  ✓ Ingested {n} per-query rows. View at /evaluations")


if __name__ == "__main__":
    sys.exit(main())
