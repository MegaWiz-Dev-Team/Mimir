#!/usr/bin/env python3
"""
Sprint 48 B-48f — Embed ICD-10-TM rows + push to Qdrant collection `icd10-th`.

Embedding source:  Ollama nomic-embed-text @ http://localhost:11434
                   (already installed locally, 274 MB, dim=768)
Vector store:      Qdrant @ http://localhost:6333
                   (port-forward kubectl asgard-infra/qdrant required)

Usage:
  # 1. Ensure Qdrant port-forward is running:
  kubectl port-forward -n asgard-infra svc/qdrant 6333:6333 &

  # 2. Run embed + push:
  python3 icd10_embed_qdrant.py [--batch 64] [--workers 16] [--dry-run]

  # 3. Verify:
  curl -s http://localhost:6333/collections/icd10-th | jq

Embedding text: "{en_label}. {th_label or ''}"
  (nomic-embed-text is primarily English; Thai still useful as suffix)

Cosine similarity used for search. Idempotent: deletes + recreates collection.
"""
from __future__ import annotations
import argparse
import json
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

import requests

OLLAMA_URL = "http://localhost:11434"
QDRANT_URL = "http://localhost:6333"
# Embedding model + collection are paired — switching one forces the other.
# Default: BGE-M3 (multilingual, dim=1024, 1.2 GB Ollama tag, supports Thai well).
# Earlier v0 used nomic-embed-text (dim=768, English-tuned) — kept here as
# fallback option.
COLLECTION = "icd10-th"
EMBED_MODEL = "bge-m3"
DIM = 1024

MARIADB_POD = (
    "k8s_mariadb_mariadb-fb55894c5-xjjvb_asgard-infra_"
    "78f65c51-6439-4d1c-a9f6-ebcdad463f5c_58"
)


def fetch_rows(source_version: str) -> list[dict]:
    """Pull all icd10_codes rows from MariaDB."""
    sql = f"""
        SELECT code, en_label, th_label, chapter, source_version
          FROM icd10_codes
         WHERE source_version = '{source_version}' AND tenant_id IS NULL
         ORDER BY code
    """
    r = subprocess.run(
        ["docker", "exec", "-i", MARIADB_POD,
         "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir", "--batch"],
        input=sql.encode("utf-8"), capture_output=True, check=True,
    )
    lines = r.stdout.decode("utf-8").strip().split("\n")
    headers = lines[0].split("\t")
    return [dict(zip(headers, line.split("\t"))) for line in lines[1:]]


def make_embed_text(row: dict) -> str:
    en = row.get("en_label") or ""
    th = row.get("th_label") or ""
    if th and th != "NULL":
        return f"{en}. {th}"
    return en


def embed_one(text: str, retries: int = 3) -> list[float] | None:
    """Single Ollama embedding call. Retries on transient errors."""
    for attempt in range(retries):
        try:
            r = requests.post(
                f"{OLLAMA_URL}/api/embeddings",
                json={"model": EMBED_MODEL, "prompt": text},
                timeout=30,
            )
            r.raise_for_status()
            return r.json()["embedding"]
        except Exception as e:
            if attempt + 1 < retries:
                time.sleep(0.5 * (attempt + 1))
                continue
            print(f"  [embed-fail] {text[:40]}: {e}", file=sys.stderr)
            return None
    return None


def recreate_collection() -> None:
    """Idempotent — delete then recreate `icd10-th` with cosine similarity."""
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
    print(f"  ✓ collection {COLLECTION} recreated (dim={DIM}, distance=Cosine)")


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
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--workers", type=int, default=16)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--limit", type=int, default=0,
                    help="Limit rows for testing (0 = all)")
    args = ap.parse_args()

    # Sanity checks.
    print("=== Probing services ===")
    try:
        r = requests.get(f"{OLLAMA_URL}/api/tags", timeout=3)
        models = [m["name"] for m in r.json().get("models", [])]
        if not any(EMBED_MODEL in m for m in models):
            print(f"ERR: ollama doesn't have {EMBED_MODEL}. ollama pull {EMBED_MODEL}",
                  file=sys.stderr)
            return 1
        print(f"  ✓ Ollama models: {models}")
    except Exception as e:
        print(f"ERR: Ollama unreachable: {e}", file=sys.stderr)
        return 1

    try:
        r = requests.get(f"{QDRANT_URL}/collections", timeout=3)
        r.raise_for_status()
        print(f"  ✓ Qdrant up · existing: {[c['name'] for c in r.json()['result']['collections']]}")
    except Exception as e:
        print(f"ERR: Qdrant unreachable. Start port-forward:\n"
              f"  kubectl port-forward -n asgard-infra svc/qdrant 6333:6333 &", file=sys.stderr)
        return 1

    # Pull rows.
    print(f"\n=== Fetching rows (source_version={args.source_version}) ===")
    rows = fetch_rows(args.source_version)
    if args.limit:
        rows = rows[:args.limit]
    print(f"  rows: {len(rows):,}")

    if args.dry_run:
        print("\n[dry-run] sample:")
        for r in rows[:3]:
            print(f"  {r['code']} → '{make_embed_text(r)[:80]}'")
        return 0

    # Recreate collection.
    print("\n=== Setting up Qdrant collection ===")
    recreate_collection()

    # Embed in parallel.
    print(f"\n=== Embedding {len(rows):,} rows (workers={args.workers}) ===")
    t0 = time.time()

    def task(idx_row: tuple[int, dict]) -> tuple[int, dict, list[float] | None]:
        idx, row = idx_row
        text = make_embed_text(row)
        vec = embed_one(text)
        return idx, row, vec

    points: list[dict] = []
    pushed = 0
    failed = 0

    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futures = [ex.submit(task, (i, r)) for i, r in enumerate(rows)]
        for fut in as_completed(futures):
            idx, row, vec = fut.result()
            if vec is None:
                failed += 1
                continue
            points.append({
                "id": idx,
                "vector": vec,
                "payload": {
                    "code": row["code"],
                    "en_label": row.get("en_label"),
                    "th_label": row.get("th_label") if row.get("th_label") != "NULL" else None,
                    "chapter": row.get("chapter") if row.get("chapter") != "NULL" else None,
                    "source_version": row.get("source_version"),
                },
            })

            # Push when batch full.
            if len(points) >= args.batch:
                upsert_batch(points)
                pushed += len(points)
                points = []
                elapsed = time.time() - t0
                rate = pushed / elapsed if elapsed > 0 else 0
                eta = (len(rows) - pushed) / rate if rate > 0 else 0
                print(f"  [{int(elapsed):4d}s] pushed {pushed:6,} / {len(rows):,}  "
                      f"({rate:.1f}/s, ETA {int(eta)}s, failed {failed})")

    # Flush remaining.
    if points:
        upsert_batch(points)
        pushed += len(points)

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
