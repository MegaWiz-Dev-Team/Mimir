#!/usr/bin/env python3
"""L3 unified-search benchmark — Sprint 1 retrospective measurement.

Runs the 75 M1 retrieval queries through the L3 unified cross-KB search
endpoint (`/api/v1/knowledge/search`) and measures Hit Rate "anywhere
across the 6 KBs at k=3 per KB".

Different metric from `m1_bench_retrieval.py` Hit Rate@3:
  - m1_bench_retrieval: routes by category → exactly one KB → top-3 items
                        (the "structured retrieval" path)
  - this script        : fan-out to all 6 KBs at k=3 each → up to 18 items
                        the user actually sees in the search UI

Both are honest metrics for different surfaces. This one answers:
"If a user types this query into the cross-KB search UI, does the answer
appear anywhere on screen?"

Usage:
    python3 scripts/l3_bench_retrieval.py
    L3_INGEST_DB=0 python3 scripts/l3_bench_retrieval.py   # skip DB write
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

HERE     = Path(__file__).resolve().parent
DATASET  = HERE.parent / "tests/eval_datasets/m1/v1.0/queries.jsonl"
L3_URL   = os.environ.get("L3_URL", "http://localhost:30000/api/v1/knowledge/search")
K_PER_KB = int(os.environ.get("L3_K", "3"))

M1_DATASET_ID   = "10659f35-fbde-5961-9b8f-75e9b5f93648"
M1_DATASET_NAME = "Medical Retrieval Benchmark — M1 v1.0 (TH+EN) [L3 fan-out]"


def http_get_json(url: str, timeout: float = 15.0) -> dict:
    req = urllib.request.Request(url)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def l3_search(query: str, k: int = K_PER_KB) -> dict:
    qs = urllib.parse.urlencode({"q": query, "k": k})
    return http_get_json(f"{L3_URL}?{qs}")


# ── hit match (same shape as m1_bench / primekg_isolation) ───────────────


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
    out.extend(q.get("expected_icd_codes", []) or [])
    if q.get("category") == "drug_interaction" and not out:
        out = _interaction_expected(q["query"])
    return out


def retrieved_text_blob(l3_result: dict) -> list[str]:
    """Flatten all KB items into a list of `text` strings for hit_match."""
    texts: list[str] = []
    for kb in l3_result.get("results", []):
        for item in kb.get("items", []):
            # Pull every text-y field from every KB shape
            for key in ("name", "code", "code_formatted", "en_label", "th_label", "fsn",
                        "long_common_name", "short_name", "tmt_id", "tmlt_id",
                        "loinc_num", "entity_id", "concept_type", "chapter"):
                v = item.get(key)
                if isinstance(v, str) and v:
                    texts.append(v)
                elif isinstance(v, (int, float)):
                    texts.append(str(v))
    return texts


def hits_per_kb(l3_result: dict) -> dict[str, int]:
    return {kb["kb_id"]: kb["count"] for kb in l3_result.get("results", [])}


# ── main loop ─────────────────────────────────────────────────────────────


def main() -> int:
    if not DATASET.exists():
        print(f"ERR: {DATASET} not found", file=sys.stderr); return 1
    queries = [json.loads(l) for l in DATASET.read_text().splitlines() if l.strip()]

    print(f"=== L3 cross-KB fan-out benchmark ===")
    print(f"  Endpoint: {L3_URL}")
    print(f"  k per KB: {K_PER_KB}  (up to {K_PER_KB*6} items aggregated)")
    print(f"  M1 dataset: {len(queries)} queries (TH+EN)")
    print()

    rows = []
    by_cat: dict[str, list] = defaultdict(list)
    kb_contribution: dict[str, int] = defaultdict(int)  # which KBs ever had a hit
    t0 = time.time()

    for i, q in enumerate(queries, 1):
        cat = q.get("category", "?")
        expected = collect_expected(q)
        q_t0 = time.time()
        try:
            r = l3_search(q["query"], K_PER_KB)
            texts = retrieved_text_blob(r)
            hit = hit_match(expected, texts)
            kb_counts = hits_per_kb(r)
            # Identify which KB(s) carried the hit, if any
            if hit:
                for kb_id in kb_counts:
                    kb_items = next((x.get("items", [])
                                     for x in r.get("results", [])
                                     if x.get("kb_id") == kb_id), [])
                    kb_texts = []
                    for item in kb_items:
                        for key in ("name", "code", "en_label", "th_label", "fsn",
                                    "long_common_name", "short_name"):
                            v = item.get(key)
                            if isinstance(v, str) and v:
                                kb_texts.append(v)
                    if hit_match(expected, kb_texts):
                        kb_contribution[kb_id] += 1
            err = None
        except Exception as e:
            r, texts, hit, kb_counts, err = {}, [], False, {}, str(e)[:80]
        latency = int((time.time() - q_t0) * 1000)
        rows.append({
            "id": q["id"], "query": q["query"], "category": cat,
            "expected": expected,
            "retrieved_count": len(texts),
            "kb_counts": kb_counts,
            "hit": hit,
            "latency_ms": latency,
            "error": err,
        })
        by_cat[cat].append(hit)
        mark = "✓" if hit else "✗"
        err_t = f"  ERR:{err}" if err else ""
        kb_pp = ",".join(f"{k}:{v}" for k, v in kb_counts.items() if v > 0)
        print(f"  {i:>2}/{len(queries)}  {mark}  {q['id']:<8s} "
              f"[{cat:24s}] {q['query'][:24]!r:<26s}  ({kb_pp}){err_t}")

    elapsed = int(time.time() - t0)
    hits = sum(1 for r in rows if r["hit"])
    rate = hits / len(rows) if rows else 0
    print()
    print("=" * 70)
    print(f"  L3 fan-out Hit Rate (anywhere @ k={K_PER_KB}):  "
          f"{rate:.1%}  ({hits}/{len(rows)})  · {elapsed}s")
    print("=" * 70)

    print("\nBy category:")
    for cat in sorted(by_cat, key=lambda k: -len(by_cat[k])):
        items = by_cat[cat]; h = sum(items); n = len(items)
        bar = "█" * int(h/n*20) + "░" * (20-int(h/n*20))
        print(f"  {cat:<24s}  {h}/{n}  ({h/n:.0%})  {bar}")

    print("\nKB contribution (queries where THIS KB carried the hit):")
    for kb_id, cnt in sorted(kb_contribution.items(), key=lambda x: -x[1]):
        print(f"  {kb_id:<12s} {cnt:>3}")

    # Persist report
    out_path = HERE / "reports" / f"l3_bench_{time.strftime('%Y%m%d_%H%M')}.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps({
        "ran_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "scope": f"L3 unified fan-out (k={K_PER_KB} per KB, 6 KBs)",
        "endpoint": L3_URL,
        "n_queries": len(rows),
        "hit_rate_anywhere": rate,
        "by_category": {c: {"hits": sum(items), "n": len(items)} for c, items in by_cat.items()},
        "kb_contribution": dict(kb_contribution),
        "rows": rows,
    }, ensure_ascii=False, indent=2))
    print(f"\nReport: {out_path}")

    if os.environ.get("L3_INGEST_DB", "1") == "1":
        ingest_to_mimir_eval(rows, rate, elapsed)

    return 0


# ── Mimir DB ingest (mirrors primekg_isolation_bench.py) ──────────────────


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
    name = f"L3 fan-out bench {time.strftime('%Y-%m-%d %H:%M')}"
    started = time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime(time.time() - elapsed))
    finished = time.strftime('%Y-%m-%d %H:%M:%S')
    n = len(rows)
    avg_latency = sum(r.get("latency_ms", 0) for r in rows) / n if n else 0
    mrr = hit_rate  # crude proxy until we wire actual rank tracking

    print(f"\n=== Ingest run_id={run_id[:8]}… ===")
    _mariadb_exec(f"""
INSERT INTO rag_eval_runs
  (id, tenant_id, name, status, hit_rate, mrr, top_k, avg_latency_ms,
   collections, embed_model, search_provider, search_model,
   dataset_id, dataset_name, started_at, finished_at, is_baseline)
VALUES
  ({_sql_quote(run_id)}, {_sql_quote(tenant)}, {_sql_quote(name)}, 'completed',
   {hit_rate}, {mrr}, 3, {avg_latency},
   {_sql_quote('icd10-tm,tpc,loinc,tmt,tmlt,primekg')},
   'BAAI/bge-m3', 'heimdall+mariadb+neo4j', 'L3 fan-out v2.3.18',
   {_sql_quote(M1_DATASET_ID)}, {_sql_quote(M1_DATASET_NAME)},
   {_sql_quote(started)}, {_sql_quote(finished)}, 0);
""")

    batch = 50
    for i in range(0, n, batch):
        chunk = rows[i:i + batch]
        values = []
        for r in chunk:
            expected_titles = json.dumps(r.get("expected", []), ensure_ascii=False)
            kb_pp = ", ".join(f"{k}:{v}" for k, v in r.get("kb_counts", {}).items() if v > 0)
            retrieved_snippet = kb_pp[:160]
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
