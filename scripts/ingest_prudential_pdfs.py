#!/usr/bin/env python3
"""Ingest Prudential POC PDFs into Qdrant insurance_products_001.

Reads the 5 Prudential PDFs in `data/insurance/Prudential/`, chunks each
into ~2400-char segments (per sprint48_c3 asgard_insurance config), embeds
via Heimdall BGE-M3, and inserts as new points alongside the existing
web-scraped entries.

Payload schema matches the existing 2 points in the collection (so the
Underwriter app's downstream queries don't need to special-case PDF vs
web sources). insurer_id = `insurer_001` (the vendor-abstracted Prudential
ID already used in the collection).

Usage:
    export HEIMDALL_API_KEY=hml-...
    python3 scripts/ingest_prudential_pdfs.py
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
PDF_DIR      = HERE.parent / "data/insurance/Prudential"
HEIMDALL_URL = os.environ.get("HEIMDALL_API_URL", "http://localhost:8080/v1").rstrip("/")
HEIMDALL_KEY = os.environ.get("HEIMDALL_API_KEY", "")
QDRANT_URL   = os.environ.get("QDRANT_URL", "http://localhost:6334").rstrip("/")
COLLECTION   = "insurance_products_001"
TENANT_ID    = "asgard_insurance"
INSURER_ID   = "insurer_001"  # abstracted Prudential per existing collection
CHUNK_CHARS  = 2400  # sprint48_c3 setting for asgard_insurance
CHUNK_OVERLAP = 200

# Map filename → product metadata. Filenames are Thai; this table is
# the curator's record of which doc covers which product surface.
PDF_META = {
    "PRUMhaoMhaoDoubleSure.pdf": {
        "product_name": "PRUMhaoMhaoDoubleSure",
        "product_type": "life",  # life insurance product brochure
        "language": "en",
        "document_kind": "product_brochure",
    },
    "ข้อยกเว้นทั่วไปของกรมธรรม์.pdf": {
        "product_name": "General Policy Exclusions",
        "product_type": "policy_terms",
        "language": "th",
        "document_kind": "policy_exclusions",
    },
    "รายละเอียดสัญญากรมธรรม์.pdf": {
        "product_name": "Policy Contract Details",
        "product_type": "policy_terms",
        "language": "th",
        "document_kind": "policy_contract",
    },
    "เงื่อนไขการรับประกัน.pdf": {
        "product_name": "Underwriting Conditions",
        "product_type": "underwriting",
        "language": "th",
        "document_kind": "underwriting_terms",
    },
    "เงื่อนไขทั่วไปแห่งกรมธรรม์ประกันชีวิต.pdf": {
        "product_name": "General Life Insurance Policy Conditions",
        "product_type": "life",
        "language": "th",
        "document_kind": "policy_conditions",
    },
}


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
    """Use `pdftotext -layout` for table-aware extraction. -enc UTF-8
    ensures Thai chars survive cleanly."""
    r = subprocess.run(
        ["pdftotext", "-layout", "-enc", "UTF-8", str(pdf), "-"],
        capture_output=True, timeout=60, check=True,
    )
    return r.stdout.decode("utf-8")


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
                  chunk_text_: str) -> dict:
    meta = PDF_META[pdf_name]
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
    pdfs = sorted(PDF_DIR.glob("*.pdf"))
    if not pdfs:
        print(f"ERR: no PDFs in {PDF_DIR}", flush=True)
        return 1
    print(f"=== Ingesting {len(pdfs)} Prudential PDFs into {COLLECTION} ===")
    print(f"  Heimdall: {HEIMDALL_URL}")
    print(f"  Qdrant:   {QDRANT_URL}")
    print(f"  Tenant:   {TENANT_ID}  insurer: {INSURER_ID}")
    print()

    points_payload: list[dict] = []
    total_chunks = 0
    total_chars = 0
    t0 = time.time()

    for pdf in pdfs:
        if pdf.name not in PDF_META:
            print(f"  ⚠  skip (no metadata): {pdf.name}")
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
            payload = build_payload(pdf.name, i, n, chunk)
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
