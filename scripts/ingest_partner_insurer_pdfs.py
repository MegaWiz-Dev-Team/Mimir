#!/usr/bin/env python3
"""Ingest partner-insurer POC PDFs into Qdrant insurance_products_001.

Reads PDFs from the directory pointed to by PDF_INGEST_DIR (default
`data/insurance/partner-insurer/`), chunks each into ~2400-char segments
(per sprint48_c3 asgard_insurance config), embeds via Heimdall BGE-M3,
and inserts as new points alongside the existing web-scraped entries.

Payload schema matches the existing points in the collection (so the
Underwriter app's downstream queries don't need to special-case PDF vs
web sources). insurer_id = `insurer_001` is the vendor-abstracted POC
partner ID already used in the collection.

The per-file metadata table (PDF_META) lives in an external JSON file —
see PDF_META_PATH env var — so customer-specific filenames stay out of
the public-repo source. A template ships next to this script.

Usage:
    export HEIMDALL_API_KEY=hml-...
    export PDF_INGEST_DIR=/abs/path/to/insurer-pdfs
    export PDF_META_PATH=/abs/path/to/pdf_meta.json   # optional override
    python3 scripts/ingest_partner_insurer_pdfs.py
"""
from __future__ import annotations
import json
import os
import re
import subprocess
import time
import urllib.request
import uuid
from datetime import datetime
from pathlib import Path

HERE         = Path(__file__).resolve().parent
PDF_DIR      = Path(os.environ.get("PDF_INGEST_DIR",
                                   str(HERE.parent / "data/insurance/partner-insurer")))
PDF_META_PATH = Path(os.environ.get("PDF_META_PATH",
                                    str(HERE / "ingest_partner_insurer_meta.json")))
HEIMDALL_URL = os.environ.get("HEIMDALL_API_URL", "http://localhost:8080/v1").rstrip("/")
HEIMDALL_KEY = os.environ.get("HEIMDALL_API_KEY", "")
QDRANT_URL   = os.environ.get("QDRANT_URL", "http://localhost:6334").rstrip("/")
COLLECTION   = "insurance_products_001"
TENANT_ID    = "asgard_insurance"
INSURER_ID   = "insurer_001"  # vendor-abstracted POC partner ID
CHUNK_CHARS  = 2400  # sprint48_c3 setting for asgard_insurance
CHUNK_OVERLAP = 200


def load_pdf_meta() -> dict[str, dict]:
    """Per-file metadata. Sourced from `PDF_META_PATH` (JSON) so that
    customer-specific filenames never enter the public repo.

    Shape:
      { "<exact-pdf-filename>": {
          "product_name": "...", "product_type": "...",
          "language": "th"|"en", "document_kind": "..."
        }, ... }
    """
    if PDF_META_PATH.is_file():
        return json.loads(PDF_META_PATH.read_text(encoding="utf-8"))
    print(f"WARN: PDF_META_PATH not found ({PDF_META_PATH}). "
          f"Provide it to map filenames → product metadata.")
    return {}


def http_post_json(url: str, body: dict, headers: dict | None = None,
                   timeout: float = 60.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    merged = {"Content-Type": "application/json"}
    if headers:
        merged.update(headers)
    req = urllib.request.Request(url, data=data, headers=merged)
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def http_put_json(url: str, body: dict, timeout: float = 60.0) -> dict:
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data,
                                  headers={"Content-Type": "application/json"},
                                  method="PUT")
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def extract_pdf_text(pdf: Path) -> str:
    """Use `pdftotext -raw` for Thai-script fidelity. Verified 2026-05-20
    that `-layout` splits Thai words with stray spaces (e.g. `ค่ า` instead
    of `ค่า`, `รักษำ` instead of `รักษา`) — sampling 10 chunks showed 1145
    mangle indicators. `-raw` keeps glyphs in font draw order so words
    stay intact; BGE-M3's tokenizer does Thai word-segmentation internally
    so the lost paragraph structure doesn't hurt retrieval."""
    r = subprocess.run(
        ["pdftotext", "-raw", "-enc", "UTF-8", str(pdf), "-"],
        capture_output=True, timeout=60, check=True,
    )
    return r.stdout.decode("utf-8")


def delete_existing_pdf_chunks() -> bool:
    """Remove old PDF-derived chunks (source_id prefix `pdf_insurer_001_`)
    before re-ingest. Idempotent on a fresh run.

    Web-scraped entries (`url_insurer_001_*`) are preserved; only the PDF
    chunks are churned. Without this, re-runs add duplicate chunks under
    fresh UUIDs and pollute the collection."""
    body = json.dumps({
        "filter": {
            "must": [
                {"key": "source_id", "match": {"text": "pdf_insurer_001_"}}
            ]
        }
    }).encode("utf-8")
    req = urllib.request.Request(
        f"{QDRANT_URL}/collections/{COLLECTION}/points/delete?wait=true",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            json.loads(resp.read().decode("utf-8"))
        return True
    except Exception:
        return False


def normalize(text: str) -> str:
    """Collapse runs of whitespace + drop empty lines. Keeps newlines
    between paragraphs so chunking can find natural break points."""
    text = re.sub(r"[ \t]+", " ", text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip()


def chunk_text(text: str, size: int = CHUNK_CHARS,
               overlap: int = CHUNK_OVERLAP) -> list[str]:
    """Recursive-style chunker: prefer breaking on paragraph (`\n\n`) then
    sentence-ish (`.` `। `), fall back to fixed window with overlap.

    For Thai (no spaces between words) the paragraph-break path is the
    main signal — long policy paragraphs may exceed `size` and get
    fixed-window split, which is the right behaviour for embedding."""
    if len(text) <= size:
        return [text]
    # Try paragraph splits first
    paras = [p.strip() for p in text.split("\n\n") if p.strip()]
    chunks: list[str] = []
    buf = ""
    for p in paras:
        if len(buf) + len(p) + 2 <= size:
            buf = f"{buf}\n\n{p}" if buf else p
        else:
            if buf:
                chunks.append(buf)
            if len(p) <= size:
                buf = p
            else:
                # Single para too big — fixed-window
                start = 0
                while start < len(p):
                    end = min(start + size, len(p))
                    chunks.append(p[start:end])
                    if end == len(p):
                        break
                    start = end - overlap
                buf = ""
    if buf:
        chunks.append(buf)
    return chunks


def embed(text: str) -> list[float]:
    out = http_post_json(
        f"{HEIMDALL_URL}/embeddings",
        {"model": "BAAI/bge-m3", "input": text},
        headers={"Authorization": f"Bearer {HEIMDALL_KEY}"},
    )
    return out["data"][0]["embedding"]


def build_payload(pdf_name: str, chunk_idx: int, n_chunks: int,
                  chunk_text_: str, pdf_meta: dict[str, dict]) -> dict:
    meta = pdf_meta[pdf_name]
    extraction_date = datetime.utcnow().isoformat()
    source_id = f"pdf_{INSURER_ID}_{Path(pdf_name).stem}_{chunk_idx:03d}"
    return {
        "source_id": source_id,
        "content": chunk_text_,
        "insurer_id": INSURER_ID,
        "tenant_id": TENANT_ID,
        "product_type": meta["product_type"],
        "status": "active",
        "language": meta["language"],
        "extraction_date": extraction_date,
        "metadata": {
            "source_file": pdf_name,
            "document_type": "pdf",
            "document_kind": meta["document_kind"],
            "language": meta["language"],
            "extraction_date": extraction_date,
            "vendor": "VENDOR_ABSTRACTED",
            "source_type": "pdf",
            "product_type": meta["product_type"],
            "channel": "direct",
            "product_name": meta["product_name"],
            "product_launch_date": "",
            "insurer_id": INSURER_ID,
            "product_version": "1.0",
            "product_end_date": None,
            "is_active": True,
            "status": "active",
            "chunk_index": chunk_idx,
            "chunk_total": n_chunks,
        },
    }


def main() -> int:
    if not HEIMDALL_KEY:
        print("ERR: HEIMDALL_API_KEY required", flush=True)
        return 1
    pdf_meta = load_pdf_meta()
    pdfs = sorted(PDF_DIR.glob("*.pdf"))
    if not pdfs:
        print(f"ERR: no PDFs in {PDF_DIR}", flush=True)
        return 1
    print(f"=== Ingesting {len(pdfs)} partner-insurer PDFs into {COLLECTION} ===")
    print(f"  Heimdall: {HEIMDALL_URL}")
    print(f"  Qdrant:   {QDRANT_URL}")
    print(f"  Tenant:   {TENANT_ID}  insurer: {INSURER_ID}")
    print()

    # Drop any prior PDF chunks first (idempotent re-ingest).
    if delete_existing_pdf_chunks():
        print("  (cleared prior pdf_insurer_001_* chunks before re-ingest)")
    print()

    points_payload: list[dict] = []
    total_chunks = 0
    total_chars = 0
    t0 = time.time()

    for pdf in pdfs:
        if pdf.name not in pdf_meta:
            print(f"  ⚠  skip (no metadata in PDF_META_PATH): {pdf.name}")
            continue
        raw = extract_pdf_text(pdf)
        norm = normalize(raw)
        chunks = chunk_text(norm)
        n = len(chunks)
        total_chunks += n
        total_chars += sum(len(c) for c in chunks)
        print(f"  📄 {pdf.name:<50s}  {len(norm):>6d} chars  →  {n} chunks")

        for i, chunk in enumerate(chunks):
            vec = embed(chunk)
            payload = build_payload(pdf.name, i, n, chunk, pdf_meta)
            point_id = str(uuid.uuid4())
            points_payload.append({
                "id": point_id,
                "vector": vec,
                "payload": payload,
            })

    if not points_payload:
        print("ERR: no chunks produced", flush=True)
        return 1

    # Batch upsert. Qdrant `points` PUT replaces atomically per batch.
    BATCH = 64
    for i in range(0, len(points_payload), BATCH):
        batch = points_payload[i:i+BATCH]
        body = {"points": batch}
        http_put_json(f"{QDRANT_URL}/collections/{COLLECTION}/points?wait=true", body)
        print(f"  ✓ upserted {min(i+BATCH, len(points_payload))}/{len(points_payload)}")

    # Verify count
    info = json.loads(urllib.request.urlopen(
        f"{QDRANT_URL}/collections/{COLLECTION}", timeout=10
    ).read().decode("utf-8"))
    pc = info.get("result", {}).get("points_count", "?")

    elapsed = int(time.time() - t0)
    print()
    print("=" * 64)
    print(f"  Ingested:    {total_chunks} chunks, {total_chars:,} chars")
    print(f"  Collection now: {pc} total points")
    print(f"  Elapsed:     {elapsed}s")
    print("=" * 64)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
