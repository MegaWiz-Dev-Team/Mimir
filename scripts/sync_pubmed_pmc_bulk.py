#!/usr/bin/env python3
"""PubMed Central (PMC) bulk loader — free, no BigQuery.

Downloads NXML zip files from NCBI's free FTP, parses full-text articles,
embeds via Heimdall BGE-M3, and upserts to Qdrant `pubmed-abstracts`.

Source:
  https://ftp.ncbi.nlm.nih.gov/pub/pmc/oa_bulk/oa_comm/xml/

Cost: $0 (FTP is free; embedding is on local Heimdall MLX).
Compare: BigQuery VECTOR_SEARCH = ~$10-15 per top_k=100K query (~$67 spent already)

Modes:
  --incremental  : download "incr" zip (last day's updates, ~MB scale, fast)
  --baseline     : download "baseline" zip (full snapshot, multi-GB, slow)
  --filter ENT,SLEEP,CARDS  : keep only articles with these MeSH/keyword matches
  --limit N      : safety cap (default 200)
  --dry-run      : list files only, no download/embed

Idempotent by pmid (Qdrant point.id = uuid5(pmid)).

Env (auto-discoverable):
  PMC_FTP_BASE  default https://ftp.ncbi.nlm.nih.gov/pub/pmc/oa_bulk/oa_comm/xml/
  HEIMDALL_*    same as sync_pubmed_incremental.py
  QDRANT_URL    default http://localhost:16333 (port-forwarded)
  TENANT_ID     default __global__
"""
import argparse
import hashlib
import json
import os
import re
import sys
import tarfile
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from datetime import datetime
from pathlib import Path

PMC_FTP_BASE  = os.environ.get("PMC_FTP_BASE", "https://ftp.ncbi.nlm.nih.gov/pub/pmc/oa_bulk/oa_comm/xml/")
HEIMDALL_BASE = os.environ.get("HEIMDALL_BASE", "http://localhost:8080")
HEIMDALL_KEY  = os.environ.get("HEIMDALL_KEY", "hml-REDACTED")
QDRANT_URL    = os.environ.get("QDRANT_URL", "http://localhost:16333")
COLLECTION    = "pubmed-abstracts"
CACHE_DIR     = Path(os.environ.get("CACHE_DIR", "/tmp/pmc-bulk-cache"))


def http_get(url: str, timeout: int = 60) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "mimir-pmc-bulk/1.0"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def http_post_json(url: str, body=None, headers=None, timeout=120):
    h = headers or {}
    data = json.dumps(body).encode() if body is not None else None
    if data is not None:
        h["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=h, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def list_pmc_files(mode: str) -> list[str]:
    """List available zip files via the FTP HTTP index page."""
    url = PMC_FTP_BASE
    print(f"  → GET {url}")
    html = http_get(url, timeout=30).decode("utf-8", errors="replace")
    # Match e.g. 'oa_comm_xml.incr.2025-05-04.tar.gz' or 'oa_comm_xml.baseline.2025-01-15.tar.gz'
    pattern = r'href="(oa_comm_xml\.{}\.[\d-]+\.tar\.gz)"'.format(re.escape(mode))
    files = re.findall(pattern, html)
    return sorted(set(files), reverse=True)


def parse_pmc_article(xml_bytes: bytes) -> dict | None:
    """Extract pmid/title/abstract/full-text/MeSH from a single PMC NXML."""
    try:
        root = ET.fromstring(xml_bytes)
    except ET.ParseError:
        return None
    # PMID
    pmid_el = root.find(".//article-id[@pub-id-type='pmid']")
    pmid = pmid_el.text if pmid_el is not None else None
    pmcid_el = root.find(".//article-id[@pub-id-type='pmc']")
    pmcid = pmcid_el.text if pmcid_el is not None else None
    if not pmid:
        return None
    # Title
    title_el = root.find(".//article-title")
    title = "".join(title_el.itertext()).strip() if title_el is not None else ""
    # Abstract
    abstract_parts = [
        "".join(p.itertext()) for p in root.iterfind(".//abstract//p")
    ]
    abstract = " ".join(s.strip() for s in abstract_parts if s.strip())
    # Full body text (first ~10K chars)
    body_parts = ["".join(p.itertext()) for p in root.iterfind(".//body//p")]
    body = " ".join(s.strip() for s in body_parts if s.strip())[:10000]
    # MeSH
    mesh = [m.text for m in root.iterfind(".//kwd-group//kwd") if m.text]
    # Pub date
    year_el = root.find(".//pub-date/year")
    pub_date = year_el.text if year_el is not None else ""
    return {
        "pmid": pmid,
        "pmc_id": pmcid,
        "title": title,
        "abstract": abstract,
        "body": body,
        "mesh_terms": mesh,
        "pub_date": pub_date,
    }


def matches_filter(article: dict, keywords: list[str]) -> bool:
    if not keywords:
        return True
    text = (article["title"] + " " + article["abstract"] + " " +
            " ".join(article["mesh_terms"])).lower()
    return any(k.lower() in text for k in keywords)


def embed_text(texts: list[str]) -> list[list[float]]:
    res = http_post_json(
        f"{HEIMDALL_BASE}/v1/embeddings",
        body={"model": "BAAI/bge-m3", "input": texts},
        headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
    )
    return [d["embedding"] for d in res["data"]]


def upsert_qdrant(articles: list[dict], vectors: list[list[float]]):
    points = []
    for a, vec in zip(articles, vectors):
        point_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, f"pubmed:{a['pmid']}"))
        points.append({
            "id": point_id,
            "vector": {"dense": vec},
            "payload": {
                "pmid": a["pmid"],
                "pmc_id": a["pmc_id"],
                "title": a["title"],
                "abstract": a["abstract"],
                "content": a["body"][:4096],
                "mesh_terms": a["mesh_terms"],
                "pub_date": a["pub_date"],
                "source": "pmc_ftp_bulk",
                "ingested_at": datetime.utcnow().isoformat(),
            },
        })
    return http_post_json(
        f"{QDRANT_URL}/collections/{COLLECTION}/points?wait=true",
        body={"points": points},
        timeout=180,
    )


def file_sha256(path: Path) -> str:
    """Streaming SHA-256 of a file (constant memory)."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_hash_index() -> dict:
    """Read prior file→hash map from CACHE_DIR/.hash_index.json."""
    p = CACHE_DIR / ".hash_index.json"
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text())
    except Exception:
        return {}


def save_hash_index(idx: dict) -> None:
    p = CACHE_DIR / ".hash_index.json"
    p.write_text(json.dumps(idx, sort_keys=True, indent=2))


def process_archive(file_url: str, keyword_filter: list[str], limit: int,
                    dry_run: bool, force: bool = False) -> str:
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    filename = file_url.rsplit("/", 1)[-1]
    cache_path = CACHE_DIR / filename
    hash_idx = load_hash_index()
    prior_hash = hash_idx.get(filename)

    if cache_path.exists():
        print(f"  ⊕ Using cached: {cache_path} ({cache_path.stat().st_size//1024//1024} MB)")
    else:
        print(f"  ↓ Downloading {file_url}...")
        data = http_get(file_url, timeout=600)
        cache_path.write_bytes(data)
        print(f"  ✓ Saved {len(data)//1024//1024} MB → {cache_path}")

    current_hash = file_sha256(cache_path)
    print(f"  🔑 sha256: {current_hash[:16]}…")
    if prior_hash == current_hash and not force:
        print(f"  ⏭  Hash unchanged since last run — skipping (use --force to re-ingest)")
        return "skipped_unchanged"
    hash_idx[filename] = current_hash
    save_hash_index(hash_idx)

    print(f"  ↻ Parsing tar.gz...")
    matched: list[dict] = []
    seen = 0
    with tarfile.open(cache_path, "r:gz") as tf:
        for member in tf:
            if not member.name.endswith(".xml"):
                continue
            seen += 1
            f = tf.extractfile(member)
            if f is None:
                continue
            article = parse_pmc_article(f.read())
            if not article or not article["abstract"]:
                continue
            if not matches_filter(article, keyword_filter):
                continue
            matched.append(article)
            if len(matched) >= limit:
                break
    print(f"  Found {len(matched)} articles matching filter (out of {seen} scanned)")

    if dry_run:
        print(f"\n🔍 DRY RUN — would embed + upsert {len(matched)} articles")
        for a in matched[:5]:
            print(f"   - PMID {a['pmid']}: {a['title'][:80]}")
        return "dry_run"

    if not matched:
        print("  No matches; skipping embed/upsert.")
        return "no_matches"

    print(f"  ↻ Embedding {len(matched)} articles via Heimdall BGE-M3...")
    BATCH = 8
    all_vecs = []
    for i in range(0, len(matched), BATCH):
        b = matched[i:i+BATCH]
        texts = [f"{a['title']}\n\n{a['abstract']}"[:4096] for a in b]
        all_vecs.extend(embed_text(texts))
        print(f"    [{i//BATCH + 1}/{(len(matched)+BATCH-1)//BATCH}]")

    print(f"  ↻ Upserting to Qdrant...")
    upsert_qdrant(matched, all_vecs)
    print(f"  ✓ Done {len(matched)} articles")
    return "ingested"


def main():
    p = argparse.ArgumentParser(description="PMC FTP bulk loader (free, no BigQuery)")
    p.add_argument("--mode", choices=["incremental", "baseline"], default="incremental",
                   help="incremental = daily updates (~MB), baseline = full snapshot (multi-GB)")
    p.add_argument("--filter", default="",
                   help="comma-separated keywords for MeSH/text match (e.g. 'OSA,sleep,CPAP')")
    p.add_argument("--limit", type=int, default=200, help="safety cap on articles")
    p.add_argument("--latest-n", type=int, default=1, help="how many recent files to process")
    p.add_argument("--dry-run", action="store_true", help="list files + parse counts only")
    p.add_argument("--force", action="store_true",
                   help="re-ingest even if file_hash is unchanged from last run")
    args = p.parse_args()

    print("═" * 60)
    print(f"📦 PMC FTP Bulk Loader  ({args.mode})")
    print(f"   Filter:    {args.filter or '(none — all articles)'}")
    print(f"   Limit:     {args.limit}")
    print(f"   Latest N:  {args.latest_n}")
    print(f"   Dry run:   {args.dry_run}")
    print("═" * 60)

    print(f"\n1️⃣  Listing files at {PMC_FTP_BASE}")
    files = list_pmc_files("incr" if args.mode == "incremental" else "baseline")
    print(f"   Found {len(files)} {args.mode} files")
    if not files:
        print("   ❌ No files found. Check PMC_FTP_BASE.")
        return 1

    keywords = [k.strip() for k in args.filter.split(",") if k.strip()]
    target_files = files[:args.latest_n]
    print(f"\n   Will process: {target_files}")

    results = []
    for filename in target_files:
        url = PMC_FTP_BASE.rstrip("/") + "/" + filename
        print(f"\n2️⃣  Processing {filename}")
        outcome = process_archive(url, keywords, args.limit, args.dry_run, force=args.force)
        results.append((filename, outcome))

    print(f"\n✅ Done.")
    for f, o in results:
        print(f"  · {o:<22} {f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
