#!/usr/bin/env python3
"""PubMed sync via NCBI E-utilities (no scraping).

Two modes, selected by env:
  * Incremental (default) — recent PMIDs by modification date (`datetype=mdat`,
    `term=all[sb]`), for daily freshness. Set DAYS_BACK to the window.
  * Topic backfill        — set PUBMED_QUERY to an Entrez query and (optionally)
    PUBMED_TOPIC to tag the payload. Use for "หัวข้อที่สนใจ" (e.g. OSA/CPAP,
    cardiology). Combine with DAYS_BACK=0 for all-time, or DAYS_BACK=1825 +
    PUBMED_DATETYPE=pdat for the last 5 years by publication date.

Both upsert into Qdrant `pubmed-abstracts`, idempotent by pmid
(point id = uuid5(pmid)), so re-runs never duplicate.

Pipeline:
  1. esearch.fcgi  → list pmids (paged via retstart to exceed one retmax page)
  2. efetch.fcgi   → fetch metadata + abstract per pmid
  3. embed         → BGE-M3 via Heimdall (1024d dense)
  4. upsert        → Qdrant pubmed-abstracts (idempotent point.id = uuid5(pmid))

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
  HEIMDALL_API_KEY    required for embedding (or HEIMDALL_KEY — either is accepted)
  QDRANT_URL          default http://localhost:16333  (port-forwarded)
  TENANT_ID           default __global__
  PUBMED_QUERY        Entrez query for topic backfill; default 'all[sb]' (firehose)
  PUBMED_TOPIC        payload tag written to point.payload.topic (e.g. sleep_osa_cpap)
  PUBMED_DATETYPE     mdat (modification, default) | pdat (publication) | edat
  DAYS_BACK           reldate window in days; 0 = no date filter (all-time). default 7
  MAX_RECORDS         default 200 — safety cap (paged; can exceed one esearch page)
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
from datetime import datetime
import xml.etree.ElementTree as ET

NCBI_API_KEY    = os.environ.get("NCBI_API_KEY", "")
NCBI_TOOL_NAME  = os.environ.get("NCBI_TOOL_NAME", "mimir")
NCBI_EMAIL      = os.environ.get("NCBI_EMAIL", "noreply@asgard.local")
HEIMDALL_BASE   = os.environ.get("HEIMDALL_BASE", "http://localhost:8080")
# Accept either name — the K8s CronJob injects HEIMDALL_KEY, local runs use HEIMDALL_API_KEY.
HEIMDALL_KEY    = os.environ.get("HEIMDALL_API_KEY") or os.environ.get("HEIMDALL_KEY")
QDRANT_URL      = os.environ.get("QDRANT_URL", "http://localhost:16333")
TENANT_ID       = os.environ.get("TENANT_ID", "__global__")
PUBMED_QUERY    = os.environ.get("PUBMED_QUERY", "all[sb]")
PUBMED_TOPIC    = os.environ.get("PUBMED_TOPIC", "")
PUBMED_DATETYPE = os.environ.get("PUBMED_DATETYPE", "mdat")
DAYS_BACK       = int(os.environ.get("DAYS_BACK", "7"))
MAX_RECORDS     = int(os.environ.get("MAX_RECORDS", "200"))
DRY_RUN         = os.environ.get("DRY_RUN", "0") == "1"

if not HEIMDALL_KEY and not DRY_RUN:
    sys.exit("❌ Set HEIMDALL_API_KEY (or HEIMDALL_KEY) — required to embed. "
             "Use DRY_RUN=1 to preview PMIDs without embedding.")

NCBI_BASE = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils"
COLLECTION = "pubmed-abstracts"
ESEARCH_PAGE = 9999  # NCBI esearch retmax ceiling per request; page with retstart beyond it


def http_get_xml(url: str) -> ET.Element:
    req = urllib.request.Request(url, headers={"User-Agent": f"{NCBI_TOOL_NAME}/1.0 ({NCBI_EMAIL})"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return ET.fromstring(r.read())


def http_json(url: str, body=None, headers=None, timeout=60, method="POST"):
    h = headers or {}
    data = json.dumps(body).encode() if body is not None else None
    if data is not None:
        h["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=h, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def search_pmids(term: str, max_records: int, days_back: int, datetype: str) -> list[str]:
    """esearch.fcgi: list PMIDs for `term`, paged via retstart to exceed one page.

    `days_back > 0` restricts to that reldate window (by `datetype`); `days_back == 0`
    means no date filter (all-time backfill of the topic).
    """
    collected: list[str] = []
    retstart = 0
    while len(collected) < max_records:
        page = min(ESEARCH_PAGE, max_records - len(collected))
        params = {
            "db": "pubmed",
            "term": term,
            "retmax": page,
            "retstart": retstart,
            "retmode": "json",
            "tool": NCBI_TOOL_NAME,
            "email": NCBI_EMAIL,
        }
        if days_back and days_back > 0:
            params["datetype"] = datetype   # mdat=modification, pdat=publication, edat=entrez
            params["reldate"] = days_back
        if NCBI_API_KEY:
            params["api_key"] = NCBI_API_KEY
        url = f"{NCBI_BASE}/esearch.fcgi?{urllib.parse.urlencode(params)}"
        if retstart == 0:
            print(f"  → {url}")
        req = urllib.request.Request(url, headers={"User-Agent": f"{NCBI_TOOL_NAME}/1.0 ({NCBI_EMAIL})"})
        with urllib.request.urlopen(req, timeout=30) as r:
            data = json.loads(r.read())
        result = data.get("esearchresult", {})
        ids = result.get("idlist", [])
        if not ids:
            break
        collected.extend(ids)
        total = int(result.get("count", "0") or 0)
        if len(ids) < page or retstart + len(ids) >= total:
            break  # exhausted results
        retstart += len(ids)
        time.sleep(0.4 if NCBI_API_KEY else 0.34)  # rate limit
    return collected[:max_records]


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
            journal = art.findtext(".//Journal/ISOAbbreviation") or art.findtext(".//Journal/Title", "") or ""
            citation = " ".join(x for x in [journal, pub_date] if x).strip()
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
                "journal": journal,
                "citation": citation,
                "mesh_terms": mesh,
                "authors": authors,
            })
        # Rate limit: 3 req/s without key, 10 with
        time.sleep(0.4 if NCBI_API_KEY else 0.34)
    return out


def embed_text(texts: list[str]) -> list[list[float]]:
    body = {"model": "BAAI/bge-m3", "input": texts}
    res = http_json(
        f"{HEIMDALL_BASE}/v1/embeddings",
        body=body,
        headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
        timeout=120,
        method="POST",
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
                "citation": a.get("citation", ""),
                "pub_date": a["pub_date"],
                "mesh_terms": a["mesh_terms"],
                "authors": a["authors"],
                "topic": PUBMED_TOPIC,
                "source": "ncbi_eutils",
                "ingested_at": datetime.utcnow().isoformat(),
            },
        })
    res = http_json(
        f"{QDRANT_URL}/collections/{COLLECTION}/points?wait=true",
        body={"points": points},
        timeout=180,
        method="PUT",   # Qdrant upsert is PUT; POST /points expects a different schema (ids)
    )
    return res


def main():
    mode = "topic backfill" if PUBMED_QUERY != "all[sb]" else "incremental (firehose)"
    date_filter = f"last {DAYS_BACK}d by {PUBMED_DATETYPE}" if DAYS_BACK > 0 else "all-time (no date filter)"
    print("═" * 60)
    print("📚 PubMed Sync")
    print(f"   Mode:         {mode}")
    print(f"   Query:        {PUBMED_QUERY}")
    print(f"   Topic tag:    {PUBMED_TOPIC or '(none)'}")
    print(f"   Date filter:  {date_filter}")
    print(f"   Max records:  {MAX_RECORDS}")
    print(f"   API key:      {'set ✓' if NCBI_API_KEY else 'NOT set (3 req/s)'}")
    print(f"   Dry run:      {DRY_RUN}")
    print("═" * 60)

    print(f"\n1️⃣  esearch.fcgi → PMIDs")
    pmids = search_pmids(PUBMED_QUERY, MAX_RECORDS, DAYS_BACK, PUBMED_DATETYPE)
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

    print(f"\n3️⃣  Embed (BGE-M3, batches of 8) + upsert to '{COLLECTION}' incrementally")
    BATCH = 8
    FLUSH = 64            # upsert every ~64 articles so progress is saved as we go
    total_batches = (len(articles) + BATCH - 1) // BATCH
    buf_articles, buf_vectors, upserted = [], [], 0
    for i in range(0, len(articles), BATCH):
        batch = articles[i:i + BATCH]
        texts = [f"{a['title']}\n\n{a['abstract']}"[:4096] for a in batch]
        buf_vectors.extend(embed_text(texts))
        buf_articles.extend(batch)
        print(f"  embed [{(i//BATCH)+1}/{total_batches}]")
        if len(buf_articles) >= FLUSH:
            upsert_qdrant(buf_articles, buf_vectors)
            upserted += len(buf_articles)
            print(f"  ✓ upserted {upserted}/{len(articles)}")
            buf_articles, buf_vectors = [], []
    if buf_articles:
        upsert_qdrant(buf_articles, buf_vectors)
        upserted += len(buf_articles)
        print(f"  ✓ upserted {upserted}/{len(articles)}")

    print(f"\n✅ Done — {upserted} points in '{COLLECTION}'"
          f"{f' (topic={PUBMED_TOPIC})' if PUBMED_TOPIC else ''}. Idempotent: re-runs won't duplicate.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
