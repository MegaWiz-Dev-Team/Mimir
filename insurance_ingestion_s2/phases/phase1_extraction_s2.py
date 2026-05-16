"""Phase 1 (S2): Extract from URLs + File uploads + OCR images (multi-insurer, Thai support)."""

from typing import Optional
from pathlib import Path
import json
import hashlib
from datetime import datetime

import requests
from bs4 import BeautifulSoup

from insurance_ingestion_s2.core import (
    Phase, PipelineLogger, Chunk, PipelineConfig,
    PipelineError, format_progress, ProductStatus,
)


class ExtractionConfig:
    """Configuration for extraction phase (S1 + S2)."""
    TIMEOUT_SECONDS = 30
    RETRIES = 3
    TARGET_TOKENS_PER_CHUNK = 300
    CHUNK_OVERLAP_TOKENS = 50
    MIN_CHUNK_TOKENS = 100
    OCR_CONFIDENCE_THRESHOLD = 0.8

    # Product type patterns (URL path based)
    PRODUCT_TYPE_PATTERNS = {
        "health": [r"/health/", r"/medical/", r"/hospitalization/", r"/care/"],
        "life": [r"/life/", r"/protection/", r"/term/", r"/endowment/"],
        "savings": [r"/savings/", r"/retirement/", r"/pension/", r"/retirement-plan/"],
        "investment": [r"/investment/", r"/unit-linked/", r"/wealth/", r"/ulip/"],
    }

    # Channel patterns (based on domain/partner)
    CHANNEL_PATTERNS = {
        "uob": [r"uob\.co\.th", r"uob", r"united-overseas"],
        "ttb": [r"ttb\.co\.th", r"ttb", r"krungthai-axa"],
        "cimb": [r"cimbthai\.com", r"cimb", r"thai-cover"],
        "krungthai": [r"krungthai\.co\.th", r"krungthai"],
        "agent": [r"/agent/", r"/distributor/", r"/broker/"],
        "broker": [r"broker", r"intermediary", r"aggregator"],
    }


def classify_product_type(url: str) -> str:
    """Classify product type from URL path (health, life, savings, investment)."""
    import re
    url_lower = url.lower()

    for product_type, patterns in ExtractionConfig.PRODUCT_TYPE_PATTERNS.items():
        for pattern in patterns:
            if re.search(pattern, url_lower):
                return product_type

    return "health"  # Default to health


def classify_channel(url: str) -> str:
    """Classify distribution channel from URL (direct, uob, ttb, cimb, broker, agent)."""
    import re
    url_lower = url.lower()

    for channel, patterns in ExtractionConfig.CHANNEL_PATTERNS.items():
        for pattern in patterns:
            if re.search(pattern, url_lower):
                return channel

    return "direct"  # Default to direct


def extract_product_name(content: str, url: str = "") -> str:
    """Extract product name from content (first line, title, or product mention)."""
    lines = content.split("\n")

    # Try first non-empty line with capital letters (likely a title)
    for line in lines[:10]:
        stripped = line.strip()
        if stripped and len(stripped) > 5 and len(stripped) < 100:
            # Check if it looks like a title (has caps)
            if sum(1 for c in stripped if c.isupper()) > 2:
                return stripped

    # Fallback: extract from URL
    if url:
        parts = url.rstrip("/").split("/")
        for part in reversed(parts):
            if part and not part.endswith(".html"):
                return part.replace("-", " ").title()

    return ""


def extract_launch_date(content: str) -> str:
    """Extract product launch date from content if available."""
    import re

    # Pattern: "launched on", "effective from", "started on", etc.
    patterns = [
        r"(?:launched|started|effective)\s+(?:on|from|date)?\s+(\d{1,2}[-/]\d{1,2}[-/]\d{4})",
        r"(\d{4}[-/]\d{1,2}[-/]\d{1,2})",
        r"(?:January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2},?\s+(\d{4})",
    ]

    for pattern in patterns:
        match = re.search(pattern, content[:1000])  # Check first 1000 chars
        if match:
            return match.group(1)

    return ""


def extract_from_urls(
    urls: list[str],
    config: PipelineConfig,
    insurer_id: str = "insurer_001",
    language: str = "en",
    logger: Optional[PipelineLogger] = None,
) -> list[dict]:
    """Fetch HTML from configured URLs (S1 compatible + S2 insurer/language support).

    Args:
        urls: List of product page URLs to extract
        config: Pipeline configuration
        insurer_id: S2: Insurer identifier for multi-insurer support
        language: S2: Language code (en, th, bi)
        logger: Optional logger for progress feedback

    Returns:
        List of dicts: {"source_id", "content", "metadata", "insurer_id", "language"}
    """
    if logger is None:
        logger = PipelineLogger(Phase.EXTRACTION)

    docs = []
    failed = 0

    for i, url in enumerate(urls):
        logger.info(format_progress(i, len(urls), "Fetching"))

        try:
            resp = requests.get(url, timeout=ExtractionConfig.TIMEOUT_SECONDS)
            resp.raise_for_status()

            soup = BeautifulSoup(resp.content, "html.parser")
            for tag in soup(["script", "style"]):
                tag.decompose()

            text = soup.get_text(separator="\n", strip=True)

            doc_id = url.split("/")[-1].replace(".html", "")

            # S2: Classify product type, channel, and extract product metadata
            product_type = classify_product_type(url)
            channel = classify_channel(url)
            product_name = extract_product_name(text, url)
            product_launch_date = extract_launch_date(text)

            docs.append({
                "source_id": f"url_{insurer_id}_{doc_id}_{i}",
                "content": text,
                "metadata": {
                    "source_url": url,
                    "document_type": "web_page",
                    "language": language,
                    "extraction_date": datetime.now().isoformat(),
                    "vendor": "VENDOR_ABSTRACTED",
                    "source_type": "url",
                    "product_type": product_type,
                    "channel": channel,
                    "product_name": product_name,
                    "product_launch_date": product_launch_date,
                },
                "insurer_id": insurer_id,
                "language": language,
                "product_type": product_type,
                "channel": channel,
                "product_name": product_name,
                "product_launch_date": product_launch_date,
            })

        except requests.RequestException as e:
            logger.warning(f"Failed to fetch {url}: {e}")
            failed += 1
            if failed > len(urls) * 0.1:
                raise PipelineError(f"Too many extraction failures ({failed}/{len(urls)})")

    logger.success(f"Extracted {len(docs)}/{len(urls)} documents from URLs")
    return docs


def extract_from_files(
    file_paths: list[Path],
    config: PipelineConfig,
    insurer_id: str = "insurer_001",
    language: str = "en",
    logger: Optional[PipelineLogger] = None,
) -> list[dict]:
    """Extract text from uploaded files (PDF, DOCX, TXT, images with OCR).

    Args:
        file_paths: List of file paths to process
        config: Pipeline configuration
        insurer_id: S2: Insurer identifier
        language: S2: Language code
        logger: Optional logger

    Returns:
        List of extracted documents with content
    """
    if logger is None:
        logger = PipelineLogger(Phase.EXTRACTION)

    docs = []
    failed = 0

    for i, file_path in enumerate(file_paths):
        logger.info(format_progress(i, len(file_paths), "Processing"))

        try:
            file_path = Path(file_path)
            if not file_path.exists():
                raise FileNotFoundError(f"File not found: {file_path}")

            ext = file_path.suffix.lower()

            if ext == ".pdf":
                content = _extract_pdf(file_path, logger)
                doc_type = "pdf"

            elif ext in [".docx", ".doc"]:
                content = _extract_docx(file_path, logger)
                doc_type = "docx"

            elif ext == ".txt":
                content = file_path.read_text(encoding="utf-8")
                doc_type = "text"

            elif ext in [".jpg", ".jpeg", ".png"]:
                content = _extract_image_ocr(file_path, config, logger)
                doc_type = "image_ocr"

            else:
                logger.warning(f"Unsupported file type: {ext}")
                continue

            file_hash = hashlib.md5(file_path.read_bytes()).hexdigest()[:8]

            # S2: Extract product metadata from file content
            product_name = extract_product_name(content)
            product_launch_date = extract_launch_date(content)
            # For files, classify as health by default (can be overridden by filename pattern)
            product_type = "health"
            if "life" in file_path.name.lower():
                product_type = "life"
            elif "savings" in file_path.name.lower():
                product_type = "savings"
            elif "investment" in file_path.name.lower():
                product_type = "investment"

            docs.append({
                "source_id": f"file_{insurer_id}_{file_path.stem}_{file_hash}",
                "content": content,
                "metadata": {
                    "file_path": str(file_path),
                    "file_name": file_path.name,
                    "file_size_bytes": file_path.stat().st_size,
                    "document_type": doc_type,
                    "language": language,
                    "extraction_date": datetime.now().isoformat(),
                    "source_type": "upload",
                    "product_type": product_type,
                    "product_name": product_name,
                    "product_launch_date": product_launch_date,
                },
                "insurer_id": insurer_id,
                "language": language,
                "product_type": product_type,
                "channel": "direct",  # Files are typically direct uploads
                "product_name": product_name,
                "product_launch_date": product_launch_date,
            })

        except Exception as e:
            logger.warning(f"Failed to extract {file_path}: {e}")
            failed += 1

    if docs:
        logger.success(f"Extracted {len(docs)}/{len(file_paths)} documents from files")
    return docs


def _extract_pdf(file_path: Path, logger: PipelineLogger) -> str:
    """Extract text from PDF file."""
    try:
        import PyPDF2
    except ImportError:
        logger.warning("PyPDF2 not installed, skipping PDF")
        return ""

    text = []
    try:
        with open(file_path, "rb") as f:
            reader = PyPDF2.PdfReader(f)
            for page in reader.pages:
                text.append(page.extract_text())
    except Exception as e:
        logger.warning(f"PDF extraction failed: {e}")

    return "\n".join(text)


def _extract_docx(file_path: Path, logger: PipelineLogger) -> str:
    """Extract text from DOCX file."""
    try:
        from docx import Document
    except ImportError:
        logger.warning("python-docx not installed, skipping DOCX")
        return ""

    text = []
    try:
        doc = Document(file_path)
        for para in doc.paragraphs:
            if para.text.strip():
                text.append(para.text)
    except Exception as e:
        logger.warning(f"DOCX extraction failed: {e}")

    return "\n".join(text)


def _extract_image_ocr(file_path: Path, config: PipelineConfig, logger: PipelineLogger) -> str:
    """Extract text from image using Syn OCR endpoint or local OCR."""
    if config.ocr_enabled:
        try:
            return _extract_via_syn(file_path, config, logger)
        except Exception as e:
            logger.warning(f"Syn OCR failed, fallback to local: {e}")

    try:
        return _extract_via_pytesseract(file_path, logger)
    except Exception as e:
        logger.warning(f"OCR extraction failed: {e}")
        return ""


def _extract_via_syn(file_path: Path, config: PipelineConfig, logger: PipelineLogger) -> str:
    """Send image to Syn OCR service for extraction."""
    try:
        with open(file_path, "rb") as f:
            files = {"image": f}
            resp = requests.post(
                config.syn_endpoint,
                files=files,
                timeout=30,
            )
            resp.raise_for_status()
            result = resp.json()
            return result.get("text", "")
    except Exception as e:
        logger.warning(f"Syn endpoint error: {e}")
        raise


def _extract_via_pytesseract(file_path: Path, logger: PipelineLogger) -> str:
    """Fallback: Use pytesseract for local OCR."""
    try:
        import pytesseract
        from PIL import Image
    except ImportError:
        logger.warning("pytesseract/Pillow not installed")
        return ""

    try:
        image = Image.open(file_path)
        text = pytesseract.image_to_string(image)
        return text
    except Exception as e:
        logger.warning(f"Pytesseract OCR failed: {e}")
        return ""


def chunk_document(
    doc: dict,
    target_tokens: int = ExtractionConfig.TARGET_TOKENS_PER_CHUNK,
    overlap: int = ExtractionConfig.CHUNK_OVERLAP_TOKENS,
) -> list[Chunk]:
    """Split document into chunks of target token size (S1 + S2 metadata).

    Args:
        doc: Document dict with source_id, content, metadata, insurer_id, language, product metadata
        target_tokens: Target tokens per chunk (~300)
        overlap: Overlap tokens between chunks

    Returns:
        List of Chunk objects with S2 fields (hierarchical + temporal)
    """
    paragraphs = doc["content"].split("\n\n")
    chunks = []
    current_text = ""
    chunk_idx = 0

    for para in paragraphs:
        current_text += para + "\n\n"
        token_count = int(len(current_text.split()) * 1.3)

        if token_count >= target_tokens:
            chunk = Chunk(
                source_id=doc["source_id"],
                content=current_text.strip(),
                metadata=doc["metadata"].copy(),
                chunk_index=chunk_idx,
                tokens=token_count,
                insurer_id=doc.get("insurer_id", "insurer_001"),
                language=doc.get("language", "en"),
                source_type=doc["metadata"].get("source_type", "url"),
                product_type=doc.get("product_type", "health"),
                channel=doc.get("channel", "direct"),
                product_name=doc.get("product_name", ""),
                product_version="1.0",  # Default version during extraction
                product_launch_date=doc.get("product_launch_date", ""),
                product_end_date=None,  # null = still active
                is_active=True,  # Assume active unless specified
                status="active",  # Default to active
            )
            chunks.append(chunk)

            words = current_text.split()
            overlap_count = int(overlap / 1.3)
            current_text = " ".join(words[-overlap_count:]) + "\n\n"
            chunk_idx += 1

    if current_text.strip():
        token_count = int(len(current_text.split()) * 1.3)
        if token_count >= ExtractionConfig.MIN_CHUNK_TOKENS:
            chunk = Chunk(
                source_id=doc["source_id"],
                content=current_text.strip(),
                metadata=doc["metadata"].copy(),
                chunk_index=chunk_idx,
                tokens=token_count,
                insurer_id=doc.get("insurer_id", "insurer_001"),
                language=doc.get("language", "en"),
                source_type=doc["metadata"].get("source_type", "url"),
                product_type=doc.get("product_type", "health"),
                channel=doc.get("channel", "direct"),
                product_name=doc.get("product_name", ""),
                product_version="1.0",  # Default version during extraction
                product_launch_date=doc.get("product_launch_date", ""),
                product_end_date=None,  # null = still active
                is_active=True,  # Assume active unless specified
                status="active",  # Default to active
            )
            chunks.append(chunk)

    return chunks


def run_phase1_s2(
    config: PipelineConfig,
    urls: Optional[list[str]] = None,
    file_paths: Optional[list[Path]] = None,
    insurers: dict = None,
) -> Path:
    """Execute Phase 1 (S2): Extract from URLs + Files + OCR + Multi-insurer support.

    Args:
        config: Pipeline configuration
        urls: List of URLs to extract (flattened from insurer config)
        file_paths: List of file paths to process (S2 new)
        insurers: Dict mapping insurer_id to {name, urls, language} (S2 new)
            If provided, extract per-insurer URLs; otherwise use urls parameter

    Returns:
        Path to output JSONL file
    """
    logger = PipelineLogger(Phase.EXTRACTION, quiet=config.test_mode)
    logger.info(f"Phase 1 (S2): Extract URLs + Files + OCR (Multi-Insurer)")

    insurers = insurers or {"insurer_001": {"name": "Prudential", "language": "en"}}
    all_chunks = []

    config.output_dir.mkdir(parents=True, exist_ok=True)

    # S1 URL extraction (compat) — map URLs to insurers if provided
    if insurers and any(insurer.get("urls") for insurer in insurers.values()):
        # Per-insurer URLs: each insurer extracts their own URLs
        total_urls = 0
        for insurer_id, insurer_meta in insurers.items():
            insurer_urls = insurer_meta.get("urls", [])
            if not insurer_urls:
                continue

            insurer_name = insurer_meta.get("name", insurer_id)
            logger.info(f"Extracting {len(insurer_urls)} URLs for {insurer_name}")
            total_urls += len(insurer_urls)

            docs = extract_from_urls(
                insurer_urls,
                config,
                insurer_id=insurer_id,
                language=insurer_meta.get("language", "en"),
                logger=logger,
            )
            for doc in docs:
                chunks = chunk_document(doc)
                all_chunks.extend(chunks)

        logger.success(f"Extracted from {total_urls} URLs across {len(insurers)} insurers")

    elif urls:
        # Flat URL list: extract all URLs (assume all belong to default insurer)
        logger.info(f"Extracting {len(urls)} URLs (single insurer mode)")
        default_insurer = next(iter(insurers.items())) if insurers else ("insurer_001", {"name": "Prudential", "language": "en"})
        insurer_id, insurer_meta = default_insurer

        docs = extract_from_urls(
            urls,
            config,
            insurer_id=insurer_id,
            language=insurer_meta.get("language", "en"),
            logger=logger,
        )
        for doc in docs:
            chunks = chunk_document(doc)
            all_chunks.extend(chunks)

    # S2 File upload extraction
    if file_paths:
        config.upload_dir.mkdir(parents=True, exist_ok=True)
        for insurer_id, insurer_meta in insurers.items():
            logger.info(f"Processing files for {insurer_meta.get('name', insurer_id)}")
            docs = extract_from_files(
                file_paths,
                config,
                insurer_id=insurer_id,
                language=insurer_meta.get("language", "en"),
                logger=logger,
            )
            for doc in docs:
                chunks = chunk_document(doc)
                all_chunks.extend(chunks)

    if all_chunks:
        logger.success(f"Extracted {len(all_chunks)} total chunks")
        logger.info(f"Total tokens: {sum(c.tokens for c in all_chunks)}")
    else:
        logger.warning("No chunks extracted (no URLs or files provided)")

    # Write JSONL output
    output_file = config.output_dir / "phase1_chunks.jsonl"
    with open(output_file, "w") as f:
        for chunk in all_chunks:
            f.write(chunk.to_jsonl() + "\n")

    logger.success(f"Phase 1 (S2) Complete: {len(all_chunks)} chunks → {output_file}")
    return output_file


if __name__ == "__main__":
    config = PipelineConfig()
    run_phase1_s2(config)
