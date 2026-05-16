"""Phase 1: Extract products from public web sources (24 products × 40 chunks)."""

import json
import logging
from typing import Optional
from pathlib import Path
from urllib.parse import urljoin

import requests
from bs4 import BeautifulSoup

from insurance_ingestion.core import (
    Phase, PipelineLogger, Chunk, PipelineConfig,
    PipelineError, format_progress,
)


class ExtractionConfig:
    """Configuration for extraction phase."""
    TIMEOUT_SECONDS = 30
    RETRIES = 3
    TARGET_TOKENS_PER_CHUNK = 300
    CHUNK_OVERLAP_TOKENS = 50
    MIN_CHUNK_TOKENS = 100


def extract_from_urls(
    urls: list[str],
    config: PipelineConfig,
    logger: Optional[PipelineLogger] = None,
) -> list[dict]:
    """Fetch HTML from configured URLs.

    Args:
        urls: List of product page URLs to extract
        config: Pipeline configuration
        logger: Optional logger for progress feedback

    Returns:
        List of dicts: {"source_id", "content", "metadata"}

    Raises:
        PipelineError: If too many URLs fail
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
            # Remove script/style tags
            for tag in soup(["script", "style"]):
                tag.decompose()

            text = soup.get_text(separator="\n", strip=True)

            doc_id = url.split("/")[-1].replace(".html", "")
            docs.append({
                "source_id": f"extracted_{doc_id}_{i}",
                "content": text,
                "metadata": {
                    "source_url": url,
                    "document_type": "web_page",
                    "language": "en",
                    "extraction_date": "2026-05-16",
                    "vendor": "VENDOR_ABSTRACTED",
                },
            })

        except requests.RequestException as e:
            logger.warning(f"Failed to fetch {url}: {e}")
            failed += 1
            if failed > len(urls) * 0.1:
                raise PipelineError(f"Too many extraction failures ({failed}/{len(urls)})")

    logger.success(f"Extracted {len(docs)}/{len(urls)} documents")
    return docs


def chunk_document(
    doc: dict,
    target_tokens: int = ExtractionConfig.TARGET_TOKENS_PER_CHUNK,
    overlap: int = ExtractionConfig.CHUNK_OVERLAP_TOKENS,
) -> list[Chunk]:
    """Split document into chunks of target token size.

    Uses simple paragraph-based chunking with overlap.

    Args:
        doc: Document dict with "source_id", "content", "metadata"
        target_tokens: Target tokens per chunk (~300)
        overlap: Overlap tokens between chunks

    Returns:
        List of Chunk objects with sequential indices
    """
    # Simple token estimate: ~1.3 tokens per word
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
            )
            chunks.append(chunk)

            # Overlap: keep last ~50 tokens for next chunk
            words = current_text.split()
            overlap_count = int(overlap / 1.3)
            current_text = " ".join(words[-overlap_count:]) + "\n\n"
            chunk_idx += 1

    # Final chunk if remainder
    if current_text.strip():
        token_count = int(len(current_text.split()) * 1.3)
        if token_count >= ExtractionConfig.MIN_CHUNK_TOKENS:
            chunk = Chunk(
                source_id=doc["source_id"],
                content=current_text.strip(),
                metadata=doc["metadata"].copy(),
                chunk_index=chunk_idx,
                tokens=token_count,
            )
            chunks.append(chunk)

    return chunks


def run_phase1(config: PipelineConfig) -> Path:
    """Execute Phase 1: extract and chunk documents.

    Success criteria:
    - 950+ chunks extracted from 24 products
    - Each chunk 250-350 tokens
    - JSONL output with all 21 metadata fields

    Returns:
        Path to output JSONL file
    """
    logger = PipelineLogger(Phase.EXTRACTION)

    # In production, this would read from PRUDENTIAL_DATA_INGESTION_SUMMARY.md
    urls = [
        "https://example.com/products/health-plan-a",
        "https://example.com/products/critical-illness",
        # ... 22 more
    ]

    logger.info(f"Starting Phase 1: Extract {len(urls)} products")

    try:
        docs = extract_from_urls(urls, config, logger)
        logger.info(f"Fetched {len(docs)} documents")

        all_chunks = []
        for i, doc in enumerate(docs):
            chunks = chunk_document(doc)
            all_chunks.extend(chunks)
            logger.info(format_progress(i + 1, len(docs), "Chunking"))

        logger.success(f"Created {len(all_chunks)} chunks ({sum(c.tokens for c in all_chunks)} tokens)")

        # Write JSONL output
        output_file = config.output_dir / "phase1_chunks.jsonl"
        config.output_dir.mkdir(parents=True, exist_ok=True)

        with open(output_file, "w") as f:
            for chunk in all_chunks:
                f.write(chunk.to_jsonl() + "\n")

        logger.success(f"Wrote {len(all_chunks)} chunks to {output_file}")
        return output_file

    except PipelineError as e:
        logger.error(f"Phase 1 failed: {e}")
        raise


if __name__ == "__main__":
    config = PipelineConfig()
    run_phase1(config)
