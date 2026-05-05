#!/usr/bin/env python3
"""⚠️ DEPRECATED — costs $$ on BigQuery. DO NOT RUN without explicit ALLOW_BIGQUERY=1.

This script ran 6 specialty batches × top_k=100,000 VECTOR_SEARCH on
bigquery-public-data.pmc_open_access_commercial → cost ~$67 USD.

Use these instead (free):
  - Incremental: scripts/sync_pubmed_incremental.py  (NCBI E-utilities)
  - Bulk:        scripts/sync_pubmed_pmc_bulk.py     (PMC FTP)

Kept for reference only.

PubMed batch import: BQ VECTOR_SEARCH → BGE-M3 embed → Qdrant + Postgres.

Usage:
  python3.14 pipeline_pubmed.py --plan pubmed_overnight.yaml
  python3.14 pipeline_pubmed.py --plan pubmed_overnight.yaml --only sleep-1
  python3.14 pipeline_pubmed.py --status

Each batch is idempotent: PMIDs already in medical.pubmed_articles are skipped.
Crash-safe: progress is committed every upsert batch.

Requirements (pre-flight, all on localhost):
  - Heimdall BGE-M3 at :8080 (HEIMDALL_API_KEY env)
  - Qdrant at :6333 (kubectl port-forward svc/qdrant)
  - Postgres mimir DB at :5433 (kubectl port-forward svc/postgres)
  - gcloud auth application-default login (for BQ + Vertex AI)
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import uuid
import urllib.request
import urllib.error
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

import yaml
import psycopg2
from psycopg2.extras import execute_values
from qdrant_client import QdrantClient
from qdrant_client.http import models as qm
from google.cloud import bigquery

# ─── Config ──────────────────────────────────────────────────────────────────
# OrbStack k3s ClusterIPs are reachable from host (no kubectl port-forward needed).
# Override via env if topology changes.
HEIMDALL_API   = os.environ.get("HEIMDALL_API", "http://localhost:8080")
HEIMDALL_KEY   = os.environ.get("HEIMDALL_API_KEY", "hml-mimir-ffcad30d20ac3b2cbc0643c0874b738517edb4c6ec6c49698e7518ffad5123ff")
QDRANT_HOST    = os.environ.get("QDRANT_HOST", "192.168.194.178")
QDRANT_PORT    = int(os.environ.get("QDRANT_PORT", "6333"))
PG_DSN         = os.environ.get(
    "PG_DSN",
    "host=192.168.194.211 port=5432 user=mimir password=mimir_password dbname=mimir",
)
GCP_PROJECT    = os.environ.get("GCP_PROJECT", "asgard-mimir")
GCP_LOCATION   = os.environ.get("GCP_LOCATION", "us-central1")
VERTEX_MODEL   = "text-embedding-004"   # 768d, matches BQ pre-computed
EMBED_MODEL    = "bge-m3"               # 1024d via Heimdall
COLLECTION     = "pubmed-abstracts"
DENSE_DIM      = 1024
SOURCE_TABLE   = "`bigquery-public-data.pmc_open_access_commercial.articles`"

# ─── Logging ─────────────────────────────────────────────────────────────────
def log(msg: str, *args: Any) -> None:
    ts = time.strftime("%H:%M:%S")
    if args:
        msg = msg % args
    print(f"[{ts}] {msg}", flush=True)


# ─── Vertex AI seed embedding (REST, no SDK dependency) ──────────────────────
def vertex_token() -> str:
    out = subprocess.check_output(
        ["gcloud", "auth", "application-default", "print-access-token"],
        text=True
    ).strip()
    return out


def embed_seed_query(text: str) -> list[float]:
    """Embed seed query via Vertex AI text-embedding-004 (768d)."""
    url = (f"https://{GCP_LOCATION}-aiplatform.googleapis.com/v1/"
           f"projects/{GCP_PROJECT}/locations/{GCP_LOCATION}/"
           f"publishers/google/models/{VERTEX_MODEL}:predict")
    body = {"instances": [{"task_type": "RETRIEVAL_QUERY", "content": text}]}
    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {vertex_token()}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        result = json.load(r)
    vec = result["predictions"][0]["embeddings"]["values"]
    if len(vec) != 768:
        raise RuntimeError(f"Unexpected dim: {len(vec)} (want 768)")
    return vec


# ─── Heimdall BGE-M3 embedding ───────────────────────────────────────────────
def embed_batch(texts: list[str], retries: int = 3) -> list[list[float]]:
    """Batch-embed via Heimdall /v1/embeddings, returns list of 1024d vectors."""
    body = {"model": EMBED_MODEL, "input": texts}
    req = urllib.request.Request(
        f"{HEIMDALL_API}/v1/embeddings",
        data=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {HEIMDALL_KEY}",
            "Content-Type": "application/json",
        },
    )
    last_err: Exception | None = None
    for attempt in range(retries):
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                data = json.load(r)
            embs = [d["embedding"] for d in data["data"]]
            if any(len(e) != DENSE_DIM for e in embs):
                raise RuntimeError(f"Bad embedding dim, got {[len(e) for e in embs[:3]]}")
            return embs
        except (urllib.error.URLError, urllib.error.HTTPError, RuntimeError) as e:
            last_err = e
            wait = 2 ** attempt
            log("⚠️  embed retry %d/%d in %ds: %s", attempt + 1, retries, wait, e)
            time.sleep(wait)
    raise RuntimeError(f"embed_batch failed after {retries} retries: {last_err}")


# ─── BigQuery vector search ──────────────────────────────────────────────────
@dataclass
class BqArticle:
    pmid: int
    pmc_id: str | None
    title: str
    abstract_proxy: str
    citation: str | None
    distance: float


def bq_vector_search(
    bq: bigquery.Client,
    seed_vec: list[float],
    top_k: int,
    abstract_chars: int,
    skip_pmids: set[int] | None = None,
    fetch_page_size: int = 2000,
) -> Iterable[BqArticle]:
    """Two-stage VECTOR_SEARCH:
       Stage 1: get top_k (pmid, pmc_id, distance) only — fits BQ memory
       Stage 2: dedup, then fetch title+abstract in pages
    """
    skip_pmids = skip_pmids or set()

    # ── Stage 1: pmids only ────────────────────────────────────────────────
    stage1 = f"""
        SELECT base.pmid AS pmid, base.pmc_id AS pmc_id, distance
        FROM VECTOR_SEARCH(
          TABLE {SOURCE_TABLE},
          'ml_generate_embedding_result',
          (SELECT @qvec AS embedding),
          top_k => @top_k,
          distance_type => 'COSINE'
        )
        WHERE base.retracted IS NULL OR base.retracted != 'yes'
        ORDER BY distance
    """
    job1 = bq.query(stage1, job_config=bigquery.QueryJobConfig(
        query_parameters=[
            bigquery.ArrayQueryParameter("qvec", "FLOAT64", seed_vec),
            bigquery.ScalarQueryParameter("top_k", "INT64", top_k),
        ],
    ))

    candidates: list[tuple[str, str | None, float, int]] = []  # (pmid_str, pmc_id, distance, pmid_int)
    for row in job1.result():
        try:
            pmid_int = int(row.pmid)
        except (TypeError, ValueError):
            continue
        if pmid_int in skip_pmids:
            continue
        candidates.append((row.pmid, row.pmc_id, float(row.distance), pmid_int))
    log("  ↳ stage1: %d pmids after dedup (from top_k=%d)", len(candidates), top_k)

    if not candidates:
        return

    # ── Stage 2: fetch title + abstract per page ───────────────────────────
    for offset in range(0, len(candidates), fetch_page_size):
        page = candidates[offset:offset + fetch_page_size]
        page_pmids = [c[0] for c in page]
        meta_by_pmid: dict[str, tuple[str | None, float]] = {c[0]: (c[1], c[2]) for c in page}

        stage2 = f"""
            SELECT pmid, title,
                   SUBSTR(article_text, 1, @abstract_chars) AS abstract,
                   article_citation
            FROM {SOURCE_TABLE}
            WHERE pmid IN UNNEST(@pmids)
        """
        job2 = bq.query(stage2, job_config=bigquery.QueryJobConfig(
            query_parameters=[
                bigquery.ArrayQueryParameter("pmids", "STRING", page_pmids),
                bigquery.ScalarQueryParameter("abstract_chars", "INT64", abstract_chars),
            ],
        ))
        for row in job2.result():
            try:
                pmid_int = int(row.pmid)
            except (TypeError, ValueError):
                continue
            if not row.title or not row.abstract:
                continue
            pmc_id, distance = meta_by_pmid.get(row.pmid, (None, 0.0))
            yield BqArticle(
                pmid=pmid_int,
                pmc_id=pmc_id,
                title=row.title.strip()[:1000],
                abstract_proxy=row.abstract.strip(),
                citation=(row.article_citation or "").strip()[:2000] or None,
                distance=distance,
            )


# ─── Postgres helpers ────────────────────────────────────────────────────────
def pg_existing_pmids(conn) -> set[int]:
    with conn.cursor() as cur:
        cur.execute("SELECT pmid FROM medical.pubmed_articles")
        return {r[0] for r in cur.fetchall()}


def pg_insert_articles(conn, batch_id: str, rows: list[BqArticle]) -> None:
    with conn.cursor() as cur:
        execute_values(
            cur,
            """
            INSERT INTO medical.pubmed_articles
                (pmid, title, abstract, pmc_id, journal, distance, batch_id)
            VALUES %s
            ON CONFLICT (pmid) DO NOTHING
            """,
            [(r.pmid, r.title, r.abstract_proxy, r.pmc_id, r.citation, r.distance, batch_id) for r in rows],
        )
    conn.commit()


def pg_upsert_batch(conn, batch_id: str, fields: dict[str, Any]) -> None:
    cols = ", ".join(fields.keys())
    placeholders = ", ".join(f"%({k})s" for k in fields.keys())
    set_clause = ", ".join(f"{k} = EXCLUDED.{k}" for k in fields.keys() if k != "batch_id")
    sql = f"""
        INSERT INTO medical.pubmed_batches ({cols})
        VALUES ({placeholders})
        ON CONFLICT (batch_id) DO UPDATE SET {set_clause}
    """
    with conn.cursor() as cur:
        cur.execute(sql, fields)
    conn.commit()


# ─── Qdrant upsert ───────────────────────────────────────────────────────────
def qdrant_upsert(qd: QdrantClient, articles: list[BqArticle], vectors: list[list[float]], batch_id: str, topic: str) -> None:
    points = []
    for art, vec in zip(articles, vectors):
        # Stable point ID: UUID v5 from pmid (so re-imports overwrite)
        pid = str(uuid.uuid5(uuid.NAMESPACE_URL, f"pmid:{art.pmid}"))
        points.append(qm.PointStruct(
            id=pid,
            vector={"dense": vec},
            payload={
                "pmid": art.pmid,
                "pmc_id": art.pmc_id,
                "title": art.title,
                "content": art.abstract_proxy,
                "citation": art.citation,
                "distance_seed": art.distance,
                "batch_id": batch_id,
                "topic": topic,
                "source": "pmc_open_access_commercial",
            },
        ))
    qd.upsert(collection_name=COLLECTION, points=points, wait=True)


# ─── Run a single batch ──────────────────────────────────────────────────────
def run_batch(
    bq: bigquery.Client,
    qd: QdrantClient,
    conn,
    batch_id: str,
    seed_query: str,
    topic: str,
    count: int,
    abstract_chars: int,
    embed_batch_size: int,
    upsert_batch_size: int,
) -> dict[str, int]:
    log("═" * 60)
    log("▶ batch=%s topic=%s count=%d", batch_id, topic, count)
    log("  seed_query: %s", seed_query)

    pg_upsert_batch(conn, batch_id, {
        "batch_id": batch_id,
        "seed_query": seed_query,
        "topic": topic,
        "top_k": count,
        "status": "running",
    })

    # 1) Vertex AI seed embed
    t0 = time.time()
    seed_vec = embed_seed_query(seed_query)
    log("  ✓ seed embedded via Vertex %s (%.2fs)", VERTEX_MODEL, time.time() - t0)

    # 2) BQ vector search → stream (two-stage: pmids → text per page)
    log("  → BQ VECTOR_SEARCH top_k=%d ...", count)
    existing = pg_existing_pmids(conn)
    log("  ↳ %d PMIDs already in DB (will skip)", len(existing))

    pending: list[BqArticle] = []
    found = 0
    new_count = 0
    imported = 0
    t_start = time.time()

    for art in bq_vector_search(bq, seed_vec, count, abstract_chars, skip_pmids=existing):
        found += 1
        if art.pmid in existing:
            continue
        existing.add(art.pmid)  # avoid intra-batch dup
        pending.append(art)
        new_count += 1

        if len(pending) >= upsert_batch_size:
            imported += _flush(qd, conn, batch_id, topic, pending, embed_batch_size)
            pending = []
            elapsed = time.time() - t_start
            rate = imported / max(elapsed, 1)
            log("  · imported %d / new %d / found %d (%.1f/sec)", imported, new_count, found, rate)

    if pending:
        imported += _flush(qd, conn, batch_id, topic, pending, embed_batch_size)

    pg_upsert_batch(conn, batch_id, {
        "batch_id": batch_id,
        "seed_query": seed_query,
        "topic": topic,
        "top_k": count,
        "articles_found": found,
        "articles_new": new_count,
        "articles_imported": imported,
        "status": "done",
        "finished_at": time.strftime("%Y-%m-%d %H:%M:%S"),
    })

    elapsed = time.time() - t_start
    log("✓ batch=%s done — imported=%d new=%d found=%d in %.1fs (%.1f articles/sec)",
        batch_id, imported, new_count, found, elapsed, imported / max(elapsed, 1))
    return {"imported": imported, "new": new_count, "found": found}


def _flush(
    qd: QdrantClient,
    conn,
    batch_id: str,
    topic: str,
    pending: list[BqArticle],
    embed_batch_size: int,
) -> int:
    """Embed pending articles in sub-batches, upsert to Qdrant, insert to PG."""
    all_vecs: list[list[float]] = []
    for i in range(0, len(pending), embed_batch_size):
        chunk = pending[i:i + embed_batch_size]
        texts = [f"{a.title}\n\n{a.abstract_proxy}" for a in chunk]
        all_vecs.extend(embed_batch(texts))

    qdrant_upsert(qd, pending, all_vecs, batch_id, topic)
    pg_insert_articles(conn, batch_id, pending)
    return len(pending)


# ─── Status command ──────────────────────────────────────────────────────────
def cmd_status(conn) -> None:
    with conn.cursor() as cur:
        cur.execute("""
            SELECT batch_id, topic, status,
                   articles_found, articles_new, articles_imported,
                   started_at, finished_at,
                   EXTRACT(EPOCH FROM (COALESCE(finished_at, now()) - started_at))::int AS elapsed_sec
            FROM medical.pubmed_batches
            ORDER BY started_at
        """)
        rows = cur.fetchall()
        print(f"\n{'batch_id':<20} {'topic':<22} {'status':<10} {'imported':>10} {'new':>8} {'found':>8} {'elapsed':>8}")
        print("─" * 100)
        total_imported = 0
        for r in rows:
            bid, topic, status, found, new, imp, started, finished, elapsed = r
            print(f"{bid:<20} {topic or '':<22} {status:<10} {imp:>10} {new:>8} {found:>8} {elapsed:>7}s")
            total_imported += imp or 0

        cur.execute("SELECT COUNT(*) FROM medical.pubmed_articles")
        total = cur.fetchone()[0]
        print(f"\nTotal articles in medical.pubmed_articles: {total}")
        print(f"Total imported across all batches:          {total_imported}\n")


# ─── Main ────────────────────────────────────────────────────────────────────
def main() -> int:
    # ⛔ Cost guard — running this without ALLOW_BIGQUERY=1 will exit immediately.
    if os.environ.get("ALLOW_BIGQUERY") != "1":
        print("=" * 70, file=sys.stderr)
        print("⛔ BLOCKED: This script costs money on BigQuery (~$10+/batch).", file=sys.stderr)
        print("   Already spent ~$67 USD on previous runs.", file=sys.stderr)
        print("", file=sys.stderr)
        print("✅ USE THESE FREE ALTERNATIVES INSTEAD:", file=sys.stderr)
        print("   - Incremental: scripts/sync_pubmed_incremental.py  (NCBI E-utilities)", file=sys.stderr)
        print("   - Bulk:        scripts/sync_pubmed_pmc_bulk.py     (PMC FTP)", file=sys.stderr)
        print("", file=sys.stderr)
        print("To override (with full understanding of cost):", file=sys.stderr)
        print("   ALLOW_BIGQUERY=1 python3 pipeline_pubmed.py ...", file=sys.stderr)
        print("=" * 70, file=sys.stderr)
        return 2

    p = argparse.ArgumentParser(description="PubMed → Qdrant batch importer")
    p.add_argument("--plan", help="YAML file with batches", default=None)
    p.add_argument("--only", help="Run only batch with this id from plan", default=None)
    p.add_argument("--status", action="store_true", help="Show progress and exit")
    args = p.parse_args()

    conn = psycopg2.connect(PG_DSN)

    if args.status:
        cmd_status(conn)
        return 0

    if not args.plan:
        p.print_help()
        return 1

    plan_path = Path(args.plan)
    if not plan_path.exists():
        log("plan not found: %s", plan_path)
        return 1
    plan = yaml.safe_load(plan_path.read_text())
    defaults = plan.get("defaults", {})
    batches = plan.get("batches", [])

    bq = bigquery.Client(project=GCP_PROJECT)
    qd = QdrantClient(host=QDRANT_HOST, port=QDRANT_PORT, timeout=120)
    log("✓ connected: BQ=%s Qdrant=%s:%d Heimdall=%s", GCP_PROJECT, QDRANT_HOST, QDRANT_PORT, HEIMDALL_API)

    plan_started = time.time()
    summary: list[tuple[str, dict[str, int] | str]] = []

    for batch in batches:
        batch_id = batch["id"]
        if args.only and batch_id != args.only:
            continue
        # Skip only if already done; failed/running → retry
        with conn.cursor() as cur:
            cur.execute("SELECT status FROM medical.pubmed_batches WHERE batch_id = %s", (batch_id,))
            r = cur.fetchone()
            if r and r[0] == "done":
                log("⏭  skipping %s (already done)", batch_id)
                summary.append((batch_id, "skipped (already done)"))
                continue
            if r and r[0] == "failed":
                log("↻ retrying %s (previously failed)", batch_id)

        try:
            res = run_batch(
                bq, qd, conn,
                batch_id=batch_id,
                seed_query=batch["seed_query"],
                topic=batch.get("topic", ""),
                count=batch.get("count", defaults.get("count", 100000)),
                abstract_chars=batch.get("abstract_chars", defaults.get("abstract_chars", 3000)),
                embed_batch_size=batch.get("embed_batch", defaults.get("embed_batch", 32)),
                upsert_batch_size=batch.get("upsert_batch", defaults.get("upsert_batch", 256)),
            )
            summary.append((batch_id, res))
        except Exception as e:
            log("✗ batch %s FAILED: %s", batch_id, e)
            pg_upsert_batch(conn, batch_id, {
                "batch_id": batch_id,
                "seed_query": batch["seed_query"],
                "topic": batch.get("topic", ""),
                "top_k": batch.get("count", 0),
                "status": "failed",
                "error_message": str(e)[:500],
                "finished_at": time.strftime("%Y-%m-%d %H:%M:%S"),
            })
            summary.append((batch_id, f"failed: {e}"))
            # Continue to next batch — do not halt overnight run

    elapsed = time.time() - plan_started
    log("═" * 60)
    log("PLAN COMPLETE in %.1f min", elapsed / 60)
    for bid, res in summary:
        log("  %s: %s", bid, res)

    cmd_status(conn)
    return 0


if __name__ == "__main__":
    sys.exit(main())
