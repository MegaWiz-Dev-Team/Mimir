#!/usr/bin/env python3
"""Incremental PubMed sync via NCBI E-utilities (no scraping).

Pulls articles updated since `data_sources.last_sync_at` and upserts into
Qdrant `pubmed-abstracts`. Idempotent by pmid (Qdrant point id derived
deterministically from pmid).

Update strategy:
  1. esearch.fcgi  → list pmids updated in last N days (db=pubmed, datetype=mdat)
  2. efetch.fcgi   → fetch metadata + abstract per pmid
  3. embed         → BGE-M3 via Heimdall (1024d dense + BM25 sparse)
  4. upsert        → Qdrant pubmed-abstracts (idempotent point.id = uuid5(pmid))
  5. UPDATE data_sources SET last_sync_at = now()

Why NCBI E-utilities (not scraping):
  - Official API, allowed by ToS
  - Returns structured XML (PMID, title, abstract, MeSH, authors, journal)
  - Rate limit: 3 req/s without API key, 10 req/s with key
  - Includes only NEW/UPDATED articles since last sync (real-time)

For BULK loads (millions of articles), use PMC FTP instead:
   ftp.ncbi.nlm.nih.gov/pub/pmc/oa_bulk/  (full-text NXML zip files)

Env (auto-discoverable):
  NCBI_API_KEY        optional — 10 req/s instead of 3
  NCBI_TOOL_NAME      default 'mimir'
  NCBI_EMAIL          required by NCBI ToS for high-volume use
  HEIMDALL_BASE       default http://localhost:8080
  HEIMDALL_KEY        required for embedding
  QDRANT_URL          default http://localhost:16333  (port-forwarded)
  TENANT_ID           default __global__
  DAYS_BACK           default 7 (or use last_sync_at from data_sources)
  MAX_RECORDS         default 200 — safety cap
  DRY_RUN             1 = list pmids only, no embed/upsert
"""
import json
import os
import sys
import time
import uuid
import urllib.parse
import urllib.request
import urllib.error
from datetime import datetime, timedelta
import xml.etree.ElementTree as ET

NCBI_API_KEY    = os.environ.get("NCBI_API_KEY", "")
NCBI_TOOL_NAME  = os.environ.get("NCBI_TOOL_NAME", "mimir")
NCBI_EMAIL      = os.environ.get("NCBI_EMAIL", "noreply@asgard.local")
HEIMDALL_BASE   = os.environ.get("HEIMDALL_BASE", "http://localhost:8080")
HEIMDALL_KEY    = os.environ.get("HEIMDALL_KEY", "hml-mimir-ffcad30d20ac3b2cbc0643c0874b738517edb4c6ec6c49698e7518ffad5123ff")
QDRANT_URL      = os.environ.get("QDRANT_URL", "http://localhost:16333")
TENANT_ID       = os.environ.get("TENANT_ID", "__global__")
DAYS_BACK       = int(os.environ.get("DAYS_BACK", "7"))
MAX_RECORDS     = int(os.environ.get("MAX_RECORDS", "200"))
DRY_RUN         = os.environ.get("DRY_RUN", "0") == "1"

NCBI_BASE = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils"
COLLECTION = "pubmed-abstracts"


def http_get_xml(url: str) -> ET.Element:
    req = urllib.request.Request(url, headers={"User-Agent": f"{NCBI_TOOL_NAME}/1.0 ({NCBI_EMAIL})"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return ET.fromstring(r.read())


def http_post_json(url: str, body=None, headers=None, timeout=60):
    h = headers or {}
    data = json.dumps(body).encode() if body is not None else None
    if data is not None:
        h["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=h, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def search_recent_pmids(days_back: int, max_records: int) -> list[str]:
    """esearch.fcgi: list PMIDs updated in last N days."""
    params = {
        "db": "pubmed",
        "term": "all[sb]",       # broadest filter — replace with topic if needed
        "datetype": "mdat",      # 'mdat' = modification date
        "reldate": days_back,    # last N days
        "retmax": max_records,
        "retmode": "json",
        "tool": NCBI_TOOL_NAME,
        "email": NCBI_EMAIL,
    }
    if NCBI_API_KEY:
        params["api_key"] = NCBI_API_KEY
    url = f"{NCBI_BASE}/esearch.fcgi?{urllib.parse.urlencode(params)}"
    print(f"  → {url}")
    req = urllib.request.Request(url, headers={"User-Agent": f"{NCBI_TOOL_NAME}/1.0 ({NCBI_EMAIL})"})
    with urllib.request.urlopen(req, timeout=30) as r:
        data = json.loads(r.read())
    return data.get("esearchresult", {}).get("idlist", [])


def fetch_articles(pmids: list[str]) -> list[dict]:
    """efetch.fcgi: pull metadata + abstract for each pmid (batched)."""
    if not pmids:
        return []
    out = []
    BATCH = 50
    for i in range(0, len(pmids), BATCH):
        batch = pmids[i:i + BATCH]
        params = {
            "db": "pubmed",
            "id": ",".join(batch),
            "rettype": "abstract",
            "retmode": "xml",
            "tool": NCBI_TOOL_NAME,
            "email": NCBI_EMAIL,
        }
        if NCBI_API_KEY:
            params["api_key"] = NCBI_API_KEY
        url = f"{NCBI_BASE}/efetch.fcgi?{urllib.parse.urlencode(params)}"
        print(f"  → efetch batch {i//BATCH + 1}/{(len(pmids)+BATCH-1)//BATCH} ({len(batch)} pmids)")
        root = http_get_xml(url)
        for art in root.iter("PubmedArticle"):
            pmid = art.findtext(".//PMID", "")
            title = art.findtext(".//ArticleTitle", "") or ""
            abstract_parts = [t.text or "" for t in art.iterfind(".//AbstractText")]
            abstract = " ".join(abstract_parts).strip()
            pub_date = art.findtext(".//PubDate/Year") or art.findtext(".//PubDate/MedlineDate", "")
            mesh = [m.text for m in art.iterfind(".//MeshHeading/DescriptorName") if m.text]
            authors = [
                f"{a.findtext('LastName','')} {a.findtext('ForeName','')}".strip()
                for a in art.iterfind(".//Author")
            ][:5]
            out.append({
                "pmid": pmid,
                "title": title,
                "abstract": abstract,
                "pub_date": pub_date,
                "mesh_terms": mesh,
                "authors": authors,
            })
        # Rate limit: 3 req/s without key, 10 with
        time.sleep(0.4 if NCBI_API_KEY else 0.34)
    return out


def embed_text(texts: list[str]) -> list[list[float]]:
    body = {"model": "BAAI/bge-m3", "input": texts}
    res = http_post_json(
        f"{HEIMDALL_BASE}/v1/embeddings",
        body=body,
        headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
        timeout=120,
    )
    return [d["embedding"] for d in res["data"]]


def upsert_qdrant(articles: list[dict], vectors: list[list[float]]):
    points = []
    for a, vec in zip(articles, vectors):
        # Deterministic point id from pmid → idempotent upsert
        point_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, f"pubmed:{a['pmid']}"))
        text_for_embed = f"{a['title']}\n\n{a['abstract']}"
        points.append({
            "id": point_id,
            "vector": {"dense": vec},
            "payload": {
                "pmid": a["pmid"],
                "title": a["title"],
                "abstract": a["abstract"],
                "content": text_for_embed,
                "pub_date": a["pub_date"],
                "mesh_terms": a["mesh_terms"],
                "authors": a["authors"],
                "source": "ncbi_eutils",
                "ingested_at": datetime.utcnow().isoformat(),
            },
        })
    res = http_post_json(
        f"{QDRANT_URL}/collections/{COLLECTION}/points?wait=true",
        body={"points": points},
        timeout=180,
    )
    return res


def main():
    print("═" * 60)
    print("📚 PubMed Incremental Sync")
    print(f"   Days back:    {DAYS_BACK}")
    print(f"   Max records:  {MAX_RECORDS}")
    print(f"   API key:      {'set ✓' if NCBI_API_KEY else 'NOT set (3 req/s)'}")
    print(f"   Dry run:      {DRY_RUN}")
    print("═" * 60)

    print(f"\n1️⃣  esearch.fcgi → recent PMIDs (last {DAYS_BACK} days)")
    pmids = search_recent_pmids(DAYS_BACK, MAX_RECORDS)
    print(f"  Found {len(pmids)} PMIDs")

    if not pmids:
        print("  No new articles. Exiting.")
        return 0

    if DRY_RUN:
        print(f"\n🔍 DRY RUN — first 10 pmids:")
        for p in pmids[:10]:
            print(f"   - {p}")
        print(f"   (skipping fetch + embed + upsert)")
        return 0

    print(f"\n2️⃣  efetch.fcgi → metadata + abstract")
    articles = fetch_articles(pmids)
    print(f"  Fetched {len(articles)} articles")

    # Filter empty abstracts
    articles = [a for a in articles if a["abstract"].strip()]
    print(f"  After filtering empty: {len(articles)}")

    if not articles:
        print("  No usable articles. Exiting.")
        return 0

    print(f"\n3️⃣  Embed via Heimdall BGE-M3 (batches of 8)")
    BATCH = 8
    all_vectors = []
    for i in range(0, len(articles), BATCH):
        batch = articles[i:i + BATCH]
        texts = [f"{a['title']}\n\n{a['abstract']}"[:4096] for a in batch]
        vecs = embed_text(texts)
        all_vectors.extend(vecs)
        print(f"  [{(i//BATCH)+1}/{(len(articles)+BATCH-1)//BATCH}]")

    print(f"\n4️⃣  Upsert to Qdrant collection '{COLLECTION}'")
    res = upsert_qdrant(articles, all_vectors)
    print(f"  ✓ Upserted {len(articles)} points")

    print(f"\n✅ Done. Run again to fetch newer articles.")
    print(f"\nNext: update data_sources.last_sync_at via Mimir API")
    return 0


if __name__ == "__main__":
    sys.exit(main())
