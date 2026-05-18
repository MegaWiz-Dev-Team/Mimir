#!/usr/bin/env python3
"""
Sprint 48/49 — Embed ICD-10-TM rows + push to Qdrant collection `icd10-th`.

Embedding source:  Heimdall gateway (BGE-M3 via MLX) at HEIMDALL_API_URL
                   (OpenAI-compatible /v1/embeddings, dim=1024).
                   Per `feedback_no_ollama`: never use Ollama in Asgard stack.
Vector store:      Qdrant @ QDRANT_URL (default localhost:6333; in-cluster
                   path: http://qdrant.asgard-infra.svc:6333).

Refactored 2026-05-18 — Ollama path removed, docker-exec path replaced
with `mysql` client (configurable host/port/user/pw from env or args).

Usage:
  export HEIMDALL_API_URL=http://localhost:8080/v1
  export HEIMDALL_API_KEY=hml-...
  # MariaDB defaults to 127.0.0.1:33306 root:root mimir (matches local
  # port-forward recipe in s1_e2e_manual_2026_05_18 memory)

  python3 icd10_embed_qdrant.py [--batch 64] [--workers 16] [--dry-run]

  # Verify:
  curl -s $QDRANT_URL/collections/icd10-th | jq

Embedding text: "{en_label}. {th_label or ''}"  (BGE-M3 is multilingual,
                                                  so Thai is fully usable).

Cosine similarity used for search. Idempotent: skips recreate if the
collection already has the expected dim + count.
"""
from __future__ import annotations
import argparse
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

import requests

HEIMDALL_URL = os.environ.get("HEIMDALL_API_URL", "http://localhost:8080/v1").rstrip("/")
HEIMDALL_KEY = os.environ.get("HEIMDALL_API_KEY", "")
QDRANT_URL   = os.environ.get("QDRANT_URL", "http://localhost:6333").rstrip("/")
COLLECTION   = "icd10-th"
EMBED_MODEL  = os.environ.get("EMBED_MODEL", "BAAI/bge-m3")
DIM          = 1024


def fetch_rows(source_version: str) -> list[dict]:
    """Pull all icd10_codes rows from MariaDB via the mysql CLI."""
    host = os.environ.get("MARIADB_HOST", "127.0.0.1")
    port = os.environ.get("MARIADB_PORT", "33306")
    user = os.environ.get("MARIADB_USER", "root")
    pw   = os.environ.get("MARIADB_PASS", "root")
    db   = os.environ.get("MARIADB_DB", "mimir")
    sql = (
        "SELECT code, en_label, th_label, chapter, source_version "
        "  FROM icd10_codes "
        f" WHERE source_version = '{source_version}' AND tenant_id IS NULL "
        " ORDER BY code"
    )
    r = subprocess.run(
        ["mysql", "-h", host, "-P", port, "-u", user, f"-p{pw}", db, "--batch"],
        input=sql.encode("utf-8"), capture_output=True, check=True,
    )
    lines = r.stdout.decode("utf-8").strip().split("\n")
    if len(lines) < 2:
        return []
    headers = lines[0].split("\t")
    return [dict(zip(headers, line.split("\t"))) for line in lines[1:]]


def make_embed_text(row: dict) -> str:
    en = row.get("en_label") or ""
    th = row.get("th_label") or ""
    if th and th != "NULL":
        return f"{en}. {th}"
    return en


def embed_batch(texts: list[str], retries: int = 3) -> list[list[float]] | None:
    """Heimdall batched embeddings — OpenAI-compatible /v1/embeddings.
    Returns one vector per input text; None on hard failure."""
    if not HEIMDALL_KEY:
        print("ERR: HEIMDALL_API_KEY env var required", file=sys.stderr)
        return None
    for attempt in range(retries):
        try:
            r = requests.post(
                f"{HEIMDALL_URL}/embeddings",
                headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
                json={"model": EMBED_MODEL, "input": texts},
                timeout=60,
            )
            r.raise_for_status()
            data = r.json().get("data", [])
            if len(data) != len(texts):
                raise ValueError(f"got {len(data)} vectors for {len(texts)} inputs")
            return [item["embedding"] for item in data]
        except Exception as e:
            if attempt + 1 < retries:
                time.sleep(0.5 * (attempt + 1))
                continue
            print(f"  [embed-fail] batch of {len(texts)}: {e}", file=sys.stderr)
            return None
    return None


def ensure_collection() -> None:
    """Idempotent collection setup. Keeps existing collection if dim matches;
    recreates only when schema differs."""
    r = requests.get(f"{QDRANT_URL}/collections/{COLLECTION}", timeout=10)
    if r.ok:
        info = r.json()["result"]
        size = (info.get("config", {}).get("params", {}).get("vectors", {}) or {})
        # Handle both {size:N} and {dense:{size:N}} shapes
        cur_dim = size.get("size") or (size.get("dense") or {}).get("size")
        if cur_dim == DIM:
            n = info.get("points_count", 0)
            print(f"  ✓ collection {COLLECTION} exists with dim={DIM} ({n:,} points); reusing.")
            return
        print(f"  ⚠ collection {COLLECTION} dim mismatch ({cur_dim} != {DIM}); recreating.")
        requests.delete(f"{QDRANT_URL}/collections/{COLLECTION}", timeout=10)
    r = requests.put(
        f"{QDRANT_URL}/collections/{COLLECTION}",
        json={
            "vectors": {"size": DIM, "distance": "Cosine"},
            "optimizers_config": {"default_segment_number": 2},
        },
        timeout=10,
    )
    r.raise_for_status()
    print(f"  ✓ collection {COLLECTION} created (dim={DIM}, distance=Cosine)")


def upsert_batch(points: list[dict]) -> None:
    r = requests.put(
        f"{QDRANT_URL}/collections/{COLLECTION}/points?wait=true",
        json={"points": points},
        timeout=60,
    )
    r.raise_for_status()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source-version", default="anamai-moph-2010")
    ap.add_argument("--batch", type=int, default=64,
                    help="Rows per Heimdall embed batch + per Qdrant upsert.")
    ap.add_argument("--workers", type=int, default=4,
                    help="Concurrent embed batches.")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--limit", type=int, default=0,
                    help="Limit rows for testing (0 = all).")
    args = ap.parse_args()

    # Sanity checks.
    print("=== Probing services ===")
    try:
        r = requests.get(f"{HEIMDALL_URL}/models",
                         headers={"Authorization": f"Bearer {HEIMDALL_KEY}"} if HEIMDALL_KEY else {},
                         timeout=5)
        r.raise_for_status()
        models = [m["id"] for m in r.json().get("data", [])]
        print(f"  ✓ Heimdall up · models: {len(models)}")
    except Exception as e:
        print(f"ERR: Heimdall unreachable at {HEIMDALL_URL}: {e}", file=sys.stderr)
        return 1

    try:
        r = requests.get(f"{QDRANT_URL}/collections", timeout=3)
        r.raise_for_status()
        print(f"  ✓ Qdrant up · existing: {[c['name'] for c in r.json()['result']['collections']]}")
    except Exception as e:
        print(f"ERR: Qdrant unreachable at {QDRANT_URL}: {e}", file=sys.stderr)
        return 1

    # Pull rows.
    print(f"\n=== Fetching rows (source_version={args.source_version}) ===")
    rows = fetch_rows(args.source_version)
    if args.limit:
        rows = rows[:args.limit]
    print(f"  rows: {len(rows):,}")
    if not rows:
        print("  (no rows — did you run icd10_tm_anamai_ingest.py first?)")
        return 1

    if args.dry_run:
        print("\n[dry-run] sample:")
        for r in rows[:3]:
            print(f"  {r['code']} → '{make_embed_text(r)[:80]}'")
        return 0

    # Ensure collection.
    print("\n=== Setting up Qdrant collection ===")
    ensure_collection()

    # Embed in batches (Heimdall handles arrays natively — no need for thousands
    # of solo HTTP calls).
    print(f"\n=== Embedding {len(rows):,} rows (batch={args.batch}, workers={args.workers}) ===")
    t0 = time.time()
    pushed = 0
    failed = 0

    def task(start_idx: int) -> tuple[int, list[dict], int]:
        chunk = rows[start_idx:start_idx + args.batch]
        texts = [make_embed_text(r) for r in chunk]
        vecs = embed_batch(texts)
        if vecs is None:
            return start_idx, [], len(chunk)
        points = []
        for offset, (row, vec) in enumerate(zip(chunk, vecs)):
            points.append({
                "id": start_idx + offset,
                "vector": vec,
                "payload": {
                    "code": row["code"],
                    "en_label": row.get("en_label"),
                    "th_label": row.get("th_label") if row.get("th_label") != "NULL" else None,
                    "chapter": row.get("chapter") if row.get("chapter") != "NULL" else None,
                    "source_version": row.get("source_version"),
                    "tenant_id": None,
                },
            })
        return start_idx, points, 0

    starts = list(range(0, len(rows), args.batch))
    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futures = [ex.submit(task, s) for s in starts]
        for fut in as_completed(futures):
            start_idx, points, n_failed = fut.result()
            failed += n_failed
            if points:
                upsert_batch(points)
                pushed += len(points)
            if pushed % (args.batch * 8) == 0 or pushed >= len(rows) - args.batch:
                elapsed = time.time() - t0
                rate = pushed / elapsed if elapsed > 0 else 0
                eta = (len(rows) - pushed) / rate if rate > 0 else 0
                print(f"  [{int(elapsed):4d}s] pushed {pushed:6,} / {len(rows):,}  "
                      f"({rate:.0f}/s, ETA {int(eta)}s, failed {failed})")

    elapsed = time.time() - t0
    print(f"\n=== Done · pushed={pushed:,}, failed={failed}, elapsed={int(elapsed)}s ===")

    # Verify.
    r = requests.get(f"{QDRANT_URL}/collections/{COLLECTION}", timeout=5)
    info = r.json()["result"]
    count = info.get("points_count", 0)
    print(f"=== Qdrant {COLLECTION}: {count:,} points indexed ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())
