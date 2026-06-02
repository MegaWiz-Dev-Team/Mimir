#!/usr/bin/env python3
"""Ingest RCAT (Royal College of Anesthesiologists of Thailand) clinical
practice guidelines into Mimir / Qdrant anesthesia_kb_001 collection.

Source:  /Volumes/T7 Shield/asgard-data/mimir-kb/anesth/*.pdf  (22 files)
Target:  Qdrant collection anesthesia_kb_001 (1024-dim BGE-M3 Cosine)
Tenant:  asgard_surgical
Namespace: eir-anesthesia

Pipeline:
    PDF → pypdf text extract → Thai sentence-aware chunk →
    BGE-M3 embed via Heimdall → Qdrant insert with metadata

Usage:
    bash _start_qdrant_pf.sh &   # starts kubectl port-forward
    /tmp/docx-venv/bin/python3 02_ingest_rcat.py
    # or
    /tmp/docx-venv/bin/python3 02_ingest_rcat.py --dry-run
    /tmp/docx-venv/bin/python3 02_ingest_rcat.py --limit 1   # just 1 PDF
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
import uuid
from datetime import datetime
from pathlib import Path

import requests
from pypdf import PdfReader

try:
    from pythainlp.tokenize import sent_tokenize as thai_sent_tokenize
    HAS_PYTHAINLP = True
except ImportError:
    HAS_PYTHAINLP = False

# ─────────────────────────────────────────────────────────────────
# Config
# ─────────────────────────────────────────────────────────────────
PDF_DIR    = Path("/Volumes/T7 Shield/asgard-data/mimir-kb/anesth")
EXTRACT_DIR = Path("/Users/mimir/Developer/Mimir/data/eir-anesthesia/extracted")
HEIMDALL   = os.environ.get("HEIMDALL_URL", "http://localhost:8080/v1").rstrip("/")
QDRANT     = os.environ.get("QDRANT_URL",   "http://localhost:16333").rstrip("/")
COLLECTION = "anesthesia_kb_001"
TENANT     = "asgard_surgical"
NAMESPACE  = "eir-anesthesia"
EMBED_MODEL = "bge-m3"
DOC_TYPE   = "rcat_guideline"
LANG       = "th"
JURISDICTION = "thai"
LICENSE_KIND = "professional_society_public"
SOURCE_URL = "https://rcat.org/"

CHUNK_TARGET_CHARS = 1200   # target chunk size
CHUNK_MAX_CHARS    = 1600   # hard max
CHUNK_OVERLAP      = 200
BATCH_SIZE         = 16     # embedding batch

# ─────────────────────────────────────────────────────────────────
# Heimdall API key
# ─────────────────────────────────────────────────────────────────
def load_heimdall_key() -> str:
    env_file = Path("/Users/mimir/Developer/Heimdall/.env")
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            if line.startswith("API_KEYS="):
                value = line.split("=", 1)[1].strip().strip("'\"")
                # API_KEYS may be comma-separated, take first
                return value.split(",")[0]
    raise RuntimeError("Heimdall API key not found in .env")


HEIMDALL_KEY = load_heimdall_key()


# ─────────────────────────────────────────────────────────────────
# PDF text extraction
# ─────────────────────────────────────────────────────────────────
def extract_pdf_pages(pdf_path: Path) -> list[dict]:
    """Extract text per page. Returns list of {page_no, text}."""
    pages = []
    reader = PdfReader(str(pdf_path))
    for i, page in enumerate(reader.pages, start=1):
        text = page.extract_text() or ""
        text = text.strip()
        if text:
            pages.append({"page_no": i, "text": text})
    return pages


# ─────────────────────────────────────────────────────────────────
# Thai-aware chunking
# ─────────────────────────────────────────────────────────────────
def thai_chunk(text: str, target=CHUNK_TARGET_CHARS, max_chars=CHUNK_MAX_CHARS,
               overlap=CHUNK_OVERLAP) -> list[str]:
    """Chunk text by Thai sentences if pythainlp available, else by chars."""
    if HAS_PYTHAINLP:
        try:
            sentences = thai_sent_tokenize(text, engine="whitespace+newline")
        except Exception:
            sentences = thai_sent_tokenize(text)
    else:
        # Fallback: split on punctuation
        import re
        sentences = re.split(r"(?<=[.!?。\n])\s+", text)

    chunks = []
    current = ""
    for sent in sentences:
        if not sent.strip():
            continue
        # Force split if single sentence exceeds max
        if len(sent) > max_chars:
            for i in range(0, len(sent), max_chars - overlap):
                chunks.append(sent[i:i + max_chars])
            continue
        if len(current) + len(sent) > target and current:
            chunks.append(current.strip())
            # Overlap with last ~overlap chars
            tail = current[-overlap:] if len(current) > overlap else ""
            current = tail + " " + sent
        else:
            current = (current + " " + sent) if current else sent
    if current.strip():
        chunks.append(current.strip())
    return chunks


# ─────────────────────────────────────────────────────────────────
# Embed via Heimdall BGE-M3
# ─────────────────────────────────────────────────────────────────
def embed_batch(texts: list[str], retries=3) -> list[list[float]]:
    """POST to Heimdall /v1/embeddings."""
    headers = {
        "Authorization": f"Bearer {HEIMDALL_KEY}",
        "Content-Type": "application/json",
    }
    body = {"model": EMBED_MODEL, "input": texts}
    last_err = None
    for attempt in range(retries):
        try:
            r = requests.post(f"{HEIMDALL}/embeddings", headers=headers,
                              json=body, timeout=120)
            r.raise_for_status()
            data = r.json()
            return [d["embedding"] for d in data["data"]]
        except Exception as e:
            last_err = e
            wait = 2 ** attempt
            print(f"  ⚠ embed retry {attempt+1}/{retries} after {wait}s: {e}")
            time.sleep(wait)
    raise RuntimeError(f"Embed failed after {retries} retries: {last_err}")


# ─────────────────────────────────────────────────────────────────
# Qdrant insert
# ─────────────────────────────────────────────────────────────────
def qdrant_insert(points: list[dict]) -> dict:
    r = requests.put(f"{QDRANT}/collections/{COLLECTION}/points?wait=true",
                     json={"points": points}, timeout=60)
    r.raise_for_status()
    return r.json()


# ─────────────────────────────────────────────────────────────────
# Main per-PDF ingest
# ─────────────────────────────────────────────────────────────────
def ingest_pdf(pdf_path: Path, dry_run: bool = False) -> dict:
    print(f"\n📄 {pdf_path.name} ({pdf_path.stat().st_size / 1024:.0f}KB)")
    t0 = time.time()

    # 1. Extract pages
    pages = extract_pdf_pages(pdf_path)
    if not pages:
        print(f"  ⚠ No text extracted — skipping")
        return {"pdf": pdf_path.name, "skipped": True, "reason": "no_text"}

    total_chars = sum(len(p["text"]) for p in pages)
    print(f"  📖 {len(pages)} pages, {total_chars:,} chars")

    # Save extracted (for debugging + audit)
    EXTRACT_DIR.mkdir(parents=True, exist_ok=True)
    extract_file = EXTRACT_DIR / (pdf_path.stem + ".json")
    extract_file.write_text(json.dumps({
        "pdf": pdf_path.name,
        "pages": pages,
        "extracted_at": datetime.now().isoformat(),
    }, ensure_ascii=False, indent=2))

    # 2. Chunk per-page (preserve page metadata)
    chunks_meta = []  # list of {text, page_no, chunk_idx}
    for p in pages:
        page_chunks = thai_chunk(p["text"])
        for idx, c in enumerate(page_chunks):
            chunks_meta.append({
                "text": c,
                "page_no": p["page_no"],
                "chunk_idx_in_page": idx,
            })

    print(f"  ✂  {len(chunks_meta)} chunks "
          f"(avg {total_chars // max(len(chunks_meta), 1)} chars)")

    if dry_run:
        print("  🟡 dry-run — skipping embed + insert")
        return {"pdf": pdf_path.name, "chunks": len(chunks_meta), "dry_run": True}

    # 3. Embed in batches
    embed_t0 = time.time()
    all_embeddings = []
    for i in range(0, len(chunks_meta), BATCH_SIZE):
        batch_texts = [c["text"] for c in chunks_meta[i:i + BATCH_SIZE]]
        emb = embed_batch(batch_texts)
        all_embeddings.extend(emb)
        print(f"  🔢 embedded {min(i + BATCH_SIZE, len(chunks_meta))}/{len(chunks_meta)}", end="\r")
    print()
    embed_dur = time.time() - embed_t0
    print(f"  ⏱  embed: {embed_dur:.1f}s ({len(chunks_meta) / max(embed_dur, 0.1):.1f} chunks/s)")

    # 4. Build Qdrant points
    now = datetime.now().isoformat()
    source_pdf = pdf_path.name
    points = []
    for chunk, vec in zip(chunks_meta, all_embeddings):
        points.append({
            "id": str(uuid.uuid4()),
            "vector": vec,
            "payload": {
                "tenant_id": TENANT,
                "namespace": NAMESPACE,
                "source_pdf": source_pdf,
                "source_url": SOURCE_URL,
                "doc_type": DOC_TYPE,
                "license": LICENSE_KIND,
                "lang": LANG,
                "jurisdiction": JURISDICTION,
                "page_no": chunk["page_no"],
                "chunk_idx_in_page": chunk["chunk_idx_in_page"],
                "char_count": len(chunk["text"]),
                "text": chunk["text"],
                "ingested_at": now,
                "kb_version": "rcat-2026-05-28",
                "embed_model": EMBED_MODEL,
            }
        })

    # 5. Insert (batch of 100 to avoid huge payload)
    insert_t0 = time.time()
    for i in range(0, len(points), 100):
        qdrant_insert(points[i:i + 100])
    insert_dur = time.time() - insert_t0
    print(f"  💾 insert: {insert_dur:.1f}s")

    total_dur = time.time() - t0
    print(f"  ✅ done in {total_dur:.1f}s")

    return {
        "pdf": source_pdf,
        "pages": len(pages),
        "chars": total_chars,
        "chunks": len(chunks_meta),
        "embed_seconds": round(embed_dur, 2),
        "insert_seconds": round(insert_dur, 2),
        "total_seconds": round(total_dur, 2),
    }


# ─────────────────────────────────────────────────────────────────
# Entry
# ─────────────────────────────────────────────────────────────────
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true",
                    help="extract + chunk only, no embed/insert")
    ap.add_argument("--limit", type=int, default=0,
                    help="ingest only first N PDFs (default: all)")
    ap.add_argument("--filter", type=str, default="",
                    help="ingest only files whose name contains this substring")
    args = ap.parse_args()

    # Verify Heimdall accessible
    try:
        emb = embed_batch(["test"])
        assert len(emb[0]) == 1024
        print(f"✓ Heimdall BGE-M3 reachable (dim=1024)")
    except Exception as e:
        print(f"✗ Heimdall test failed: {e}", file=sys.stderr)
        sys.exit(1)

    # Verify Qdrant accessible
    try:
        r = requests.get(f"{QDRANT}/collections/{COLLECTION}", timeout=5)
        r.raise_for_status()
        info = r.json()["result"]
        existing_pts = info.get("vectors_count", 0)
        print(f"✓ Qdrant collection {COLLECTION} reachable (existing pts: {existing_pts})")
    except Exception as e:
        print(f"✗ Qdrant test failed: {e}", file=sys.stderr)
        print(f"  → start port-forward: kubectl port-forward -n asgard-infra svc/qdrant 16333:6333", file=sys.stderr)
        sys.exit(1)

    # Find PDFs
    pdfs = sorted([p for p in PDF_DIR.glob("*.pdf") if not p.name.startswith("._")])
    if args.filter:
        pdfs = [p for p in pdfs if args.filter in p.name]
    if args.limit:
        pdfs = pdfs[:args.limit]
    print(f"\n📚 {len(pdfs)} PDFs to ingest")

    if not pdfs:
        print("Nothing to ingest")
        return

    # Per-PDF ingest
    results = []
    for pdf in pdfs:
        try:
            r = ingest_pdf(pdf, dry_run=args.dry_run)
            results.append(r)
        except Exception as e:
            print(f"  ✗ FAILED: {e}")
            results.append({"pdf": pdf.name, "error": str(e)})

    # Summary
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    succeeded = [r for r in results if "error" not in r and not r.get("skipped")]
    failed = [r for r in results if "error" in r]
    skipped = [r for r in results if r.get("skipped")]

    total_chunks = sum(r.get("chunks", 0) for r in succeeded)
    total_dur = sum(r.get("total_seconds", 0) for r in succeeded)
    print(f"  PDFs OK:     {len(succeeded)}")
    print(f"  PDFs failed: {len(failed)}")
    print(f"  PDFs skip:   {len(skipped)}")
    print(f"  Chunks:      {total_chunks}")
    print(f"  Time:        {total_dur:.1f}s")

    if failed:
        print("\nFAILED:")
        for r in failed:
            print(f"  - {r['pdf']}: {r['error']}")

    # Save summary
    summary_file = EXTRACT_DIR.parent / f"ingest_summary_{datetime.now():%Y%m%d_%H%M%S}.json"
    summary_file.write_text(json.dumps({
        "kb_version": "rcat-2026-05-28",
        "embed_model": EMBED_MODEL,
        "collection": COLLECTION,
        "tenant": TENANT,
        "results": results,
        "summary": {
            "pdfs_ok": len(succeeded),
            "pdfs_failed": len(failed),
            "chunks_total": total_chunks,
            "duration_seconds": round(total_dur, 1),
        }
    }, ensure_ascii=False, indent=2))
    print(f"\n📊 Summary saved: {summary_file}")


if __name__ == "__main__":
    main()
