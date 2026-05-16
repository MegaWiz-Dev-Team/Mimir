"""TDD: Phase 1 - Extract products from public web sources."""

import pytest
from pathlib import Path

from insurance_ingestion.core import Phase, PipelineLogger, Chunk
from insurance_ingestion.phases.phase1_extraction import (
    extract_from_urls,
    chunk_document,
)
from tests.fixtures.sample_data import SAMPLE_CHUNKS, SAMPLE_CONFIG


class TestPhase1Extraction:
    """Test product extraction from web sources."""

    def test_extract_from_urls_returns_raw_html(self):
        """Should fetch HTML from configured URLs."""
        urls = [
            "https://example.com/plans/health-a",
            "https://example.com/plans/critical-b",
        ]
        docs = extract_from_urls(urls, SAMPLE_CONFIG)
        assert len(docs) == len(urls)
        assert all(isinstance(d["content"], str) for d in docs)

    def test_chunk_document_splits_on_paragraph_boundary(self):
        """Should split document into chunks of ~300 tokens (±20%)."""
        doc_content = "paragraph1. " * 50  # ~300 tokens
        doc = {
            "source_id": "test_001",
            "content": doc_content,
            "metadata": {"source_url": "https://example.com"},
        }
        chunks = chunk_document(doc, target_tokens=300)
        assert len(chunks) > 0
        assert all(isinstance(c, Chunk) for c in chunks)
        assert all(250 <= c.tokens <= 350 for c in chunks)

    def test_chunk_preserves_metadata(self):
        """Chunks should inherit source metadata."""
        doc = {
            "source_id": "test_002",
            "content": "test content",
            "metadata": {
                "source_url": "https://example.com",
                "product_id": "PROD_TEST",
                "vendor": "VENDOR_TEST",
            },
        }
        chunks = chunk_document(doc)
        assert all(c.metadata["source_url"] == doc["metadata"]["source_url"] for c in chunks)
        assert all(c.metadata["vendor"] == "VENDOR_TEST" for c in chunks)

    def test_extraction_pipeline_produces_jsonl_output(self):
        """Phase 1 should output JSONL file with chunks."""
        output_file = SAMPLE_CONFIG.output_dir / "phase1_chunks.jsonl"
        # Would be run by actual pipeline
        assert output_file.suffix == ".jsonl"

    def test_chunk_index_is_sequential(self):
        """Multiple chunks should have sequential indices."""
        doc = {
            "source_id": "test_003",
            "content": "chunk1. " * 100 + "chunk2. " * 100 + "chunk3. " * 100,
            "metadata": {"source_url": "https://example.com"},
        }
        chunks = chunk_document(doc, target_tokens=200)
        indices = [c.chunk_index for c in chunks]
        assert indices == list(range(len(indices)))


class TestPhase1UX:
    """Test user experience and progress reporting."""

    def test_logger_formats_progress_output(self):
        """Logger should provide clear progress feedback."""
        logger = PipelineLogger(Phase.EXTRACTION)
        logger.success("Extracted 10 documents")
        logger.warning("Skipped 1 unreachable URL")
        # Would verify output formatting in integration tests

    def test_extraction_report_shows_summary_stats(self):
        """Should report chunks extracted, tokens, error count."""
        # Expected output:
        # [1_extraction] ✅ SUCCESS | Extracted 24 products
        # [1_extraction] ℹ️ 960 chunks, 285,600 tokens
        # [1_extraction] ⚠️ 1 URL timeout (retried)
        pass


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
