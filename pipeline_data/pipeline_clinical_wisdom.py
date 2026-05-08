#!/usr/bin/env python3
"""B-S1: Populate clinical-wisdom Qdrant collection.

Sources:
  medical_data/sleep/sleep_disorders_comprehensive_guide.md
  medical_data/ent/ent_clinical_guidelines.md
  medical_data/neurology/neurology_sleep_disorders_guide.md
  medical_data/drugs/drug_reference_sleep_ent.md
  medical_data/cpap_device/Airsense11ClinicalGuide.pdf

Collection config:
  dense:  BGE-M3 1024d, Cosine
  sparse: BM25, idf modifier
  payload: {content, title, source, category, guideline_id, version, section_order}
  scope: global — no tenant_id (public medical knowledge)

Prerequisites:
  kubectl port-forward -n asgard svc/qdrant 16333:6333 &
  pip install pdfminer.six
"""

import base64
import hashlib
import json
import os
import time
import re
import urllib.request
import urllib.error
from pathlib import Path

HEIMDALL_API   = "http://localhost:8080"
HEIMDALL_KEY   = os.environ["HEIMDALL_API_KEY"]
QDRANT_API     = "http://localhost:16333"
GEMINI_API     = "https://generativelanguage.googleapis.com/v1beta"
GEMINI_MODEL   = "gemini-3-flash-preview"
GEMINI_KEY     = os.environ.get("GEMINI_API_KEY", "")
COLLECTION     = "clinical-wisdom"
DIM            = 1024
BATCH          = 6
CHUNK_MAX      = 1800  # chars — BGE-M3 works well up to ~512 tokens
CHUNK_MIN      = 200   # merge small sections

MEDICAL_DATA = Path(__file__).parent.parent / "medical_data"

SOURCES = [
    {
        "path": MEDICAL_DATA / "sleep" / "sleep_disorders_comprehensive_guide.md",
        "source": "IOM Committee on Sleep Medicine and Research (NCBI Bookshelf)",
        "category": "sleep_medicine",
        "version": "2006",
        "effective_date": "2006-01-01",
    },
    {
        "path": MEDICAL_DATA / "ent" / "ent_clinical_guidelines.md",
        "source": "AAO-HNS / RCOT / WHO ENT Clinical Guidelines",
        "category": "ent",
        "version": "2024",
        "effective_date": "2024-01-01",
    },
    {
        "path": MEDICAL_DATA / "neurology" / "neurology_sleep_disorders_guide.md",
        "source": "Neurology Sleep Disorders Reference",
        "category": "neurology",
        "version": "2024",
        "effective_date": "2024-01-01",
    },
    {
        "path": MEDICAL_DATA / "drugs" / "drug_reference_sleep_ent.md",
        "source": "DailyMed / DrugBank / FDA Drug Labels",
        "category": "pharmacology",
        "version": "2024",
        "effective_date": "2024-01-01",
    },
]

PDF_SOURCES = [
    {
        "path": MEDICAL_DATA / "cpap_device" / "Airsense11ClinicalGuide.pdf",
        "source": "ResMed AirSense 11 Clinical Guide",
        "category": "cpap_device",
        "version": "2021",
        "effective_date": "2021-01-01",
    },
]


# ── HTTP helper ───────────────────────────────────────────────────────────────

def api(url, data=None, headers=None, method=None):
    headers = headers or {}
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    else:
        body = None
    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=90) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        err = e.read().decode()[:300]
        raise RuntimeError(f"HTTP {e.code}: {err}")


# ── BM25 sparse vector ────────────────────────────────────────────────────────

_STOPWORDS = {
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "is", "are", "was", "were", "be", "been",
    "being", "have", "has", "had", "do", "does", "did", "will", "would",
    "could", "should", "may", "might", "that", "this", "it", "its", "as",
    "not", "no", "nor", "so", "yet", "both", "either", "whether", "which",
}

def _fnv32(s: str) -> int:
    h = 2166136261
    for c in s.encode():
        h = ((h ^ c) * 16777619) & 0xFFFFFFFF
    return h

def compute_sparse_bm25(text: str):
    tokens = re.findall(r"[a-z][a-z0-9\-]{1,}", text.lower())
    tokens = [t for t in tokens if t not in _STOPWORDS]
    if not tokens:
        return {"indices": [], "values": []}
    freq: dict[int, float] = {}
    for tok in tokens:
        tid = _fnv32(tok)
        freq[tid] = freq.get(tid, 0) + 1.0
    total = sum(freq.values())
    indices = list(freq.keys())
    values  = [v / total for v in freq.values()]
    return {"indices": indices, "values": values}


# ── Chunking ──────────────────────────────────────────────────────────────────

def _split_markdown(text: str, meta: dict) -> list[dict]:
    """Split markdown into sections at H2/H3 boundaries."""
    lines = text.splitlines()
    sections: list[tuple[str, list[str]]] = []
    cur_title, cur_lines = "", []

    for line in lines:
        if line.startswith("## ") or line.startswith("### "):
            if cur_lines:
                sections.append((cur_title, cur_lines))
            cur_title = line.lstrip("# ").strip()
            cur_lines = []
        elif line.startswith("# "):
            pass  # skip H1 document title
        else:
            cur_lines.append(line)
    if cur_lines:
        sections.append((cur_title, cur_lines))

    # Merge short sections into previous
    merged: list[tuple[str, str]] = []
    for title, body_lines in sections:
        body = "\n".join(body_lines).strip()
        if not body or len(body) < CHUNK_MIN:
            if merged:
                merged[-1] = (merged[-1][0], merged[-1][1] + "\n\n" + body)
            continue
        if merged and len(merged[-1][1]) < CHUNK_MIN:
            merged[-1] = (merged[-1][0], merged[-1][1] + "\n\n" + body)
        else:
            merged.append((title, body))

    chunks = []
    for i, (title, body) in enumerate(merged):
        # Split oversized sections at paragraph boundaries
        sub_bodies = _split_by_paragraphs(body, CHUNK_MAX)
        for j, sub in enumerate(sub_bodies):
            content = f"{title}\n\n{sub}".strip() if title else sub
            gid = _guideline_id(meta["path"].stem, i, j)
            chunks.append({
                "id": _chunk_id(meta["path"].stem, i, j),
                "content": content,
                "title": title,
                "source": meta["source"],
                "category": meta["category"],
                "version": meta["version"],
                "effective_date": meta["effective_date"],
                "guideline_id": gid,
                "section_order": i * 100 + j,
            })
    return chunks


def _split_by_paragraphs(text: str, max_len: int) -> list[str]:
    if len(text) <= max_len:
        return [text]
    paragraphs = re.split(r"\n{2,}", text)
    parts, buf = [], ""
    for para in paragraphs:
        if buf and len(buf) + len(para) + 2 > max_len:
            parts.append(buf.strip())
            buf = para
        else:
            buf = (buf + "\n\n" + para).strip() if buf else para
    if buf:
        parts.append(buf.strip())
    return parts or [text[:max_len]]


def _guideline_id(stem: str, section: int, sub: int) -> str:
    raw = f"{stem}:{section}:{sub}"
    return "cw" + raw.encode().hex()[:14]


def _chunk_id(stem: str, section: int, sub: int) -> int:
    raw = f"{stem}:{section}:{sub}"
    digest = hashlib.md5(raw.encode()).digest()
    return int.from_bytes(digest[:8], "little") & 0x7FFFFFFFFFFFFFFF


def _extract_pdf_text(path: Path) -> str:
    """Try pdfminer first (fast). Returns empty string if PDF is image-based."""
    try:
        from pdfminer.high_level import extract_text
        txt = extract_text(str(path))
        # Form-feed only = scanned/image PDF with no text layer
        cleaned = re.sub(r"[\x0c\s]+", "", txt)
        return txt if len(cleaned) > 200 else ""
    except Exception as e:
        print(f"  ⚠️  pdfminer error: {e}")
        return ""


def _ocr_pdf_chunk_with_gemini(pdf_chunk_bytes: bytes, page_label: str) -> str:
    """Send a small PDF chunk (1-N pages) to Gemini and return paraphrased markdown.

    We ask for a *paraphrased clinical summary* rather than verbatim extraction
    so the response doesn't trip Gemini's RECITATION filter on copyrighted PDFs.
    """
    pdf_b64 = base64.b64encode(pdf_chunk_bytes).decode()
    prompt = (
        "Read the following PDF pages and produce concise clinical knowledge notes "
        "in well-structured Markdown for use in a medical reference database.\n\n"
        "Requirements:\n"
        "- Paraphrase in your own words — do NOT copy sentences verbatim.\n"
        "- Use `## ` for topics, `### ` for subtopics.\n"
        "- Capture all clinical facts, parameter ranges, dosages, settings, alarms, contraindications, "
        "  and troubleshooting steps precisely (preserve numbers and units).\n"
        "- Use bullet lists and tables where appropriate.\n"
        "- Skip navigation icons, page numbers, copyright lines, marketing text, and section TOCs.\n"
        "- If a page has no clinical content, output exactly: `(no clinical content)`\n"
        "- Output Markdown only, no preface."
    )
    body = {
        "contents": [{"parts": [
            {"inline_data": {"mime_type": "application/pdf", "data": pdf_b64}},
            {"text": prompt},
        ]}],
        "generationConfig": {"temperature": 0.1, "maxOutputTokens": 8192},
    }
    url = f"{GEMINI_API}/models/{GEMINI_MODEL}:generateContent?key={GEMINI_KEY}"
    req = urllib.request.Request(url, data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"},
                                 method="POST")
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            data = json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"     ⚠️  {page_label}: HTTP {e.code} {e.read().decode()[:200]}")
        return ""

    candidates = data.get("candidates", [])
    if not candidates:
        return ""
    finish = candidates[0].get("finishReason", "")
    parts = candidates[0].get("content", {}).get("parts", [])
    text = "".join(p.get("text", "") for p in parts)
    if finish == "RECITATION" and not text:
        print(f"     ⚠️  {page_label}: blocked by RECITATION filter")
        return ""
    return text


def _ocr_pdf_with_gemini(path: Path) -> str:
    """OCR an image-based PDF via Gemini 3 Flash Preview, page-by-page."""
    if not GEMINI_KEY:
        print("  ⚠️  GEMINI_API_KEY not set — skipping OCR")
        return ""
    try:
        from pypdf import PdfReader, PdfWriter
    except ImportError:
        print("  ⚠️  pypdf not installed — Run: pip install pypdf")
        return ""

    reader = PdfReader(str(path))
    n_pages = len(reader.pages)
    print(f"     → {n_pages} pages, OCR via Gemini (paraphrase mode)...")
    t0 = time.time()

    # Process in chunks of 4 pages — small enough to avoid RECITATION,
    # large enough to keep total request count manageable.
    PAGES_PER_REQUEST = 4
    all_text: list[str] = []
    blocked = 0

    for start in range(0, n_pages, PAGES_PER_REQUEST):
        end = min(start + PAGES_PER_REQUEST, n_pages)
        writer = PdfWriter()
        for i in range(start, end):
            writer.add_page(reader.pages[i])
        import io
        buf = io.BytesIO()
        writer.write(buf)
        chunk_bytes = buf.getvalue()
        label = f"pp.{start+1}-{end}"
        text = _ocr_pdf_chunk_with_gemini(chunk_bytes, label)
        if text and "(no clinical content)" not in text[:50]:
            all_text.append(f"<!-- {label} -->\n{text.strip()}")
        elif not text:
            blocked += 1
        elapsed = time.time() - t0
        print(f"     [{end:3d}/{n_pages}] {elapsed:5.1f}s  {len(text):5d} chars  {label}")

    combined = "\n\n".join(all_text)
    print(f"     → Total {len(combined):,} chars from {n_pages} pages "
          f"({blocked} chunks blocked) in {time.time()-t0:.1f}s")
    return combined


def _split_pdf(path: Path, meta: dict) -> list[dict]:
    """Parse PDF via pdfminer first, fall back to Gemini OCR for scanned PDFs."""
    text = _extract_pdf_text(path)
    if not text:
        text = _ocr_pdf_with_gemini(path)
    if not text:
        return []
    return _split_markdown(text, meta)


# ── Qdrant helpers ────────────────────────────────────────────────────────────

def ensure_collection():
    """Delete (if wrong dim) and recreate clinical-wisdom with 1024d + BM25."""
    try:
        info = api(f"{QDRANT_API}/collections/{COLLECTION}")
        params = info.get("result", {}).get("config", {}).get("params", {})
        vectors = params.get("vectors", {})
        current_dim = vectors.get("size") if isinstance(vectors, dict) and "size" in vectors \
                      else vectors.get("dense", {}).get("size") if isinstance(vectors, dict) else None
        sparse = params.get("sparse_vectors", {})
        if current_dim == DIM and "bm25" in sparse:
            pts = info.get("result", {}).get("points_count", 0)
            print(f"  ✅ Exists — {pts} pts (1024d + BM25)")
            return
        print(f"  ⚠️  Wrong config (dim={current_dim}, sparse={list(sparse.keys())}) — recreating")
        api(f"{QDRANT_API}/collections/{COLLECTION}", method="DELETE")
        time.sleep(1)
    except RuntimeError:
        pass  # collection doesn't exist yet

    api(f"{QDRANT_API}/collections/{COLLECTION}", {
        "vectors": {
            "dense": {"size": DIM, "distance": "Cosine", "on_disk": False}
        },
        "sparse_vectors": {
            "bm25": {"modifier": "idf"}
        },
        "on_disk_payload": True,
    }, method="PUT")
    print("  ✅ Created (1024d dense + BM25 sparse)")


def embed_batch(texts: list[str]) -> list[list[float]]:
    resp = api(f"{HEIMDALL_API}/v1/embeddings",
               {"model": "BAAI/bge-m3", "input": texts},
               headers={"Authorization": f"Bearer {HEIMDALL_KEY}"})
    return [x["embedding"] for x in resp["data"]]


def upsert_points(chunks_batch: list[dict], vectors: list[list[float]]):
    points = []
    for chunk, vec in zip(chunks_batch, vectors):
        sparse = compute_sparse_bm25(chunk["content"])
        points.append({
            "id": chunk["id"],
            "vector": {
                "dense": vec,
                "bm25": sparse,
            },
            "payload": {
                "content":        chunk["content"],
                "title":          chunk["title"],
                "source":         chunk["source"],
                "category":       chunk["category"],
                "version":        chunk["version"],
                "effective_date": chunk["effective_date"],
                "guideline_id":   chunk["guideline_id"],
                "section_order":  chunk["section_order"],
            },
        })
    api(f"{QDRANT_API}/collections/{COLLECTION}/points", {"points": points}, method="PUT")


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    t0 = time.time()
    print("═══ B-S1: clinical-wisdom Pipeline ═══\n")

    # 1. Collect all chunks
    print("1️⃣  Parsing source files...")
    all_chunks: list[dict] = []

    for meta in SOURCES:
        path = meta["path"]
        if not path.exists():
            print(f"  ⚠️  Not found: {path}")
            continue
        text = path.read_text(encoding="utf-8")
        chunks = _split_markdown(text, meta)
        print(f"  📄 {path.name}: {len(chunks)} chunks")
        all_chunks.extend(chunks)

    for meta in PDF_SOURCES:
        path = meta["path"]
        if not path.exists():
            print(f"  ⚠️  Not found: {path}")
            continue
        print(f"  📋 {path.name}: parsing PDF...")
        chunks = _split_pdf(path, meta)
        print(f"     → {len(chunks)} chunks")
        all_chunks.extend(chunks)

    print(f"\n  Total: {len(all_chunks)} chunks across {len(SOURCES) + len(PDF_SOURCES)} sources")

    # 2. Ensure collection
    print("\n2️⃣  Qdrant collection...")
    ensure_collection()

    # 3. Embed & upsert
    print(f"\n3️⃣  Embedding & upserting... (batch={BATCH})")
    n = len(all_chunks)
    nb = (n + BATCH - 1) // BATCH
    done = 0

    for i in range(0, n, BATCH):
        batch = all_chunks[i:i + BATCH]
        texts = [c["content"][:4096] for c in batch]
        try:
            vecs = embed_batch(texts)
            upsert_points(batch, vecs)
            done += len(batch)
            pct = done * 100 // n
            bn = i // BATCH + 1
            cats = {c["category"] for c in batch}
            print(f"  [{bn:3d}/{nb}] {done:4d}/{n} ({pct:3d}%)  {', '.join(sorted(cats))}")
        except Exception as e:
            print(f"  ✗ Batch {i//BATCH+1} failed: {e}")
            continue

    elapsed = time.time() - t0

    # 4. Verify
    print("\n4️⃣  Verifying...")
    info = api(f"{QDRANT_API}/collections/{COLLECTION}")
    pts = info.get("result", {}).get("points_count", 0)
    indexed = info.get("result", {}).get("indexed_vectors_count", 0)
    print(f"  Points:  {pts}")
    print(f"  Indexed: {indexed}")

    # Category distribution
    print("\n5️⃣  Test search: 'CPAP therapy OSA apnea hypopnea index'")
    tv = embed_batch(["CPAP therapy OSA apnea hypopnea index"])
    results = api(f"{QDRANT_API}/collections/{COLLECTION}/points/search", {
        "vector": {"name": "dense", "vector": tv[0]},
        "limit": 3,
        "with_payload": True,
    })
    for j, r in enumerate(results.get("result", [])):
        score = r.get("score", 0)
        p = r.get("payload", {})
        print(f"  {j+1}. [{score:.4f}] [{p.get('category','?')}] {p.get('title','?')}")
        print(f"        {p.get('content','')[:100].replace(chr(10),' ')}...")

    print(f"\n═══ Done: {done}/{n} chunks in {elapsed:.1f}s ═══")


if __name__ == "__main__":
    main()
